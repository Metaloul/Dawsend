//! Lecture d'une source : ring → rééchantillonnage → mixage dans le buffer de sortie.
//!
//! Étape 1 : rééchantillonnage linéaire à rapport **fixe** (fréquence source → sortie),
//! sans compensation de dérive. Si les horloges divergent, le remplissage dérive
//! lentement jusqu'à un underrun ou un overrun, rattrapé proprement (fondu, silence).
//! L'étape 2 remplacera ce module par rubato + régulateur de dérive.
//!
//! Tout ce qui est appelé depuis `render` est temps réel : pas d'allocation, pas de verrou.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use dawsend_core::ring::READABLE_FRAMES;
use dawsend_core::{Consumer, ReadError};

/// Taille max d'un morceau de sortie traité d'un coup (le callback est découpé au-delà).
const CHUNK_FRAMES: usize = 1024;
/// Rapport source/sortie max accepté (768 kHz → 8 kHz serait 96 ; on reste large).
const MAX_STEP: f64 = 128.0;

/// Durée des fondus d'entrée et de sortie.
const FADE_MS: f64 = 5.0;
/// Au-delà de cible + ça, on considère qu'on a pris trop de retard et on resynchronise.
const MAX_EXCESS_MS: f64 = 40.0;

/// Statistiques lisibles depuis le thread de contrôle.
#[derive(Default)]
pub struct SourceStats {
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
    /// Trames source restant dans le ring après la dernière lecture.
    pub fill_frames: AtomicU64,
    /// 0 = en attente de données, 1 = lecture.
    pub playing: AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceKey {
    pub index: usize,
    pub generation: u32,
}

pub struct SourcePlayer {
    consumer: Consumer,
    key: SourceKey,
    stats: Arc<SourceStats>,
    /// Trames source consommées par trame de sortie.
    step: f64,
    /// Position fractionnaire entre `prev` et `cur`.
    t: f64,
    prev: [f32; 2],
    cur: [f32; 2],
    scratch: Vec<f32>,
    /// Marge visée dans le ring après chaque lecture, en trames source.
    target_frames: u64,
    max_excess_frames: u64,
    playing: bool,
    stale: bool,
    gain: f32,
    fade_inc: f32,
    /// Dernier échantillon émis, prolongé en fondu vers zéro quand les données manquent.
    tail: [f32; 2],
}

impl SourcePlayer {
    /// Crée un lecteur (hors thread audio : alloue).
    pub fn new(consumer: Consumer, output_rate: u32, latency_ms: f64) -> Self {
        let src_rate = f64::from(consumer.sample_rate());
        let step = (src_rate / f64::from(output_rate.max(1))).min(MAX_STEP);
        let key = SourceKey {
            index: consumer.index(),
            generation: consumer.generation(),
        };
        let scratch_frames = (CHUNK_FRAMES as f64 * step).ceil() as usize + 2;
        let fade_len = (f64::from(output_rate) * FADE_MS / 1000.0).max(1.0);
        Self {
            consumer,
            key,
            stats: Arc::new(SourceStats::default()),
            step,
            t: 0.0,
            prev: [0.0; 2],
            cur: [0.0; 2],
            scratch: vec![0.0; scratch_frames * 2],
            target_frames: (src_rate * latency_ms / 1000.0) as u64,
            max_excess_frames: (src_rate * MAX_EXCESS_MS / 1000.0) as u64,
            playing: false,
            stale: false,
            gain: 0.0,
            fade_inc: (1.0 / fade_len) as f32,
            tail: [0.0; 2],
        }
    }

    pub fn key(&self) -> SourceKey {
        self.key
    }

    pub fn stats(&self) -> Arc<SourceStats> {
        self.stats.clone()
    }

    pub fn source_rate(&self) -> u32 {
        self.consumer.sample_rate()
    }

    /// Le slot a été libéré ou reconfiguré : le lecteur doit être retiré.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Ajoute la source à `out` (stéréo entrelacée, `frames` trames). Temps réel.
    pub fn render_add(&mut self, out: &mut [f32], frames: usize) {
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(CHUNK_FRAMES);
            self.render_chunk(&mut out[done * 2..(done + n) * 2], n);
            done += n;
        }
    }

    fn render_chunk(&mut self, out: &mut [f32], frames: usize) {
        if self.stale || self.consumer.is_stale() {
            self.stale = true;
            self.render_tail(out);
            return;
        }
        // Trames source nécessaires pour produire `frames` trames de sortie.
        let need = (self.t + (frames - 1) as f64 * self.step).floor() as u64;
        let available = self.consumer.available();

        if !self.playing {
            // Amorçage : attendre d'avoir la marge visée en plus de ce qu'on va lire.
            if available >= self.target_frames + need && available != u64::MAX {
                self.consumer.seek_behind_writer(self.target_frames + need);
                self.playing = true;
                self.gain = 0.0;
                self.t = 0.0;
                self.prev = [0.0; 2];
                self.cur = [0.0; 2];
                self.stats.playing.store(1, Ordering::Relaxed);
            } else {
                if available > READABLE_FRAMES {
                    self.consumer.seek_behind_writer(0);
                }
                self.render_tail(out);
                return;
            }
        } else if available != u64::MAX
            && available > self.target_frames + need + self.max_excess_frames
        {
            // Trop de retard accumulé (dérive d'horloge) : on resynchronise.
            self.stats.overruns.fetch_add(1, Ordering::Relaxed);
            self.stop();
            self.render_tail(out);
            return;
        }
        // `need` repart du `t` courant : il faut le recalculer si l'amorçage l'a remis à 0.
        let need = (self.t + (frames - 1) as f64 * self.step).floor() as usize;

        match self.consumer.read_stereo(&mut self.scratch, need) {
            Ok(()) => {}
            Err(ReadError::Underrun { .. }) => {
                self.stats.underruns.fetch_add(1, Ordering::Relaxed);
                self.stop();
                self.render_tail(out);
                return;
            }
            Err(ReadError::Overrun) => {
                self.stats.overruns.fetch_add(1, Ordering::Relaxed);
                self.stop();
                self.render_tail(out);
                return;
            }
            Err(ReadError::Stale) => {
                self.stale = true;
                self.render_tail(out);
                return;
            }
        }
        self.stats.fill_frames.store(
            self.consumer.available().min(READABLE_FRAMES),
            Ordering::Relaxed,
        );

        let mut idx = 0;
        for o in out.chunks_exact_mut(2) {
            while self.t >= 1.0 {
                self.prev = self.cur;
                self.cur = [self.scratch[idx * 2], self.scratch[idx * 2 + 1]];
                idx += 1;
                self.t -= 1.0;
            }
            let t = self.t as f32;
            let l = self.prev[0] + (self.cur[0] - self.prev[0]) * t;
            let r = self.prev[1] + (self.cur[1] - self.prev[1]) * t;
            if self.gain < 1.0 {
                self.gain = (self.gain + self.fade_inc).min(1.0);
            }
            let s = [l * self.gain, r * self.gain];
            o[0] += s[0];
            o[1] += s[1];
            self.tail = s;
            self.t += self.step;
        }
        debug_assert_eq!(idx, need);
    }

    fn stop(&mut self) {
        self.playing = false;
        self.stats.playing.store(0, Ordering::Relaxed);
    }

    /// Prolonge le dernier échantillon en fondu linéaire vers zéro (évite le clic).
    fn render_tail(&mut self, out: &mut [f32]) {
        if self.tail == [0.0; 2] {
            return;
        }
        let mut g = 1.0f32;
        for o in out.chunks_exact_mut(2) {
            g = (g - self.fade_inc).max(0.0);
            o[0] += self.tail[0] * g;
            o[1] += self.tail[1] * g;
        }
        self.tail = if g > 0.0 {
            [self.tail[0] * g, self.tail[1] * g]
        } else {
            [0.0; 2]
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dawsend_core::registry::read_meta;
    use dawsend_core::{Region, SlotClaim};

    struct Shm(String);
    impl Drop for Shm {
        fn drop(&mut self) {
            dawsend_core::shm::unlink(&self.0);
        }
    }

    fn setup(name: &str, src_rate: u32) -> (Shm, Arc<Region>, SlotClaim) {
        #[cfg(unix)]
        let n = format!("/dwa.{}.{}", std::process::id(), name);
        #[cfg(windows)]
        let n = format!("Local\\DawsendAppTest.{}.{}", std::process::id(), name);
        let region = Region::open_named(&n).unwrap();
        let claim = SlotClaim::claim(region.clone(), src_rate, 2, "t").unwrap();
        (Shm(n), region, claim)
    }

    fn sine(start: usize, n: usize, rate: f32) -> Vec<f32> {
        (start..start + n)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate).sin() * 0.5)
            .collect()
    }

    #[test]
    fn primes_then_plays_without_underrun() {
        let (_s, region, claim) = setup("a", 48_000);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0);
        let mut out = vec![0.0; 480 * 2];
        let mut written = 0;
        // Simule DAW (blocs de 128) et sortie (périodes de 480) à la même horloge.
        for step in 0..2000 {
            while written < (step + 1) * 480 + 1000 {
                let b = sine(written, 128, 48_000.0);
                prod.write_planar(&[b.clone(), b]);
                written += 128;
            }
            out.fill(0.0);
            p.render_add(&mut out, 480);
        }
        let st = p.stats();
        assert_eq!(st.playing.load(Ordering::Relaxed), 1);
        assert_eq!(st.underruns.load(Ordering::Relaxed), 0);
        assert_eq!(st.overruns.load(Ordering::Relaxed), 0);
        assert!(out.iter().any(|v| v.abs() > 0.1));
        assert!(out.iter().all(|v| v.abs() <= 0.5001));
    }

    #[test]
    fn resamples_44100_to_48000() {
        let (_s, region, claim) = setup("b", 44_100);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0);
        let mut written = 0usize;
        let mut consumed_out = 0usize;
        let mut out = vec![0.0; 480 * 2];
        for _ in 0..1000 {
            consumed_out += 480;
            let target_src = consumed_out * 44_100 / 48_000 + 2000;
            while written < target_src {
                let b = sine(written, 64, 44_100.0);
                prod.write_planar(&[b.clone(), b]);
                written += 64;
            }
            out.fill(0.0);
            p.render_add(&mut out, 480);
        }
        assert_eq!(p.stats().underruns.load(Ordering::Relaxed), 0);
        // Une sinusoïde de 440 Hz ré-échantillonnée : pas de saut brutal entre trames.
        let max_jump = out
            .chunks_exact(2)
            .zip(out.chunks_exact(2).skip(1))
            .map(|(a, b)| (a[0] - b[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_jump < 0.05, "saut {max_jump}");
    }

    #[test]
    fn underrun_fades_to_silence() {
        let (_s, region, claim) = setup("c", 48_000);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 5.0);
        let mut out = vec![0.0; 256 * 2];
        let dc = vec![0.5f32; 4096];
        prod.write_planar(&[dc.clone(), dc]);
        for _ in 0..10 {
            out.fill(0.0);
            p.render_add(&mut out, 256);
        }
        // Plus de données : un underrun, puis du silence (sans saut brutal).
        out.fill(0.0);
        p.render_add(&mut out, 256);
        assert!(p.stats().underruns.load(Ordering::Relaxed) >= 1);
        out.fill(0.0);
        p.render_add(&mut out, 256);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn released_slot_becomes_stale() {
        let (_s, region, claim) = setup("d", 48_000);
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0);
        drop(claim);
        let mut out = vec![0.0; 64];
        p.render_add(&mut out, 32);
        assert!(p.is_stale());
    }
}
