//! Lecture d'une source : ring → rééchantillonnage adaptatif → mixage dans la sortie.
//!
//! - **Rééchantillonnage** : rubato (sinc asynchrone, taille fixe en sortie). Le rapport
//!   nominal `fs_sortie / fs_source` couvre les fréquences différentes (44,1 → 48 kHz,
//!   192 → 48 kHz…). Quand la source est plus rapide que la sortie, rubato abaisse la
//!   coupure du filtre sous la moitié de la fréquence de sortie (anti-repliement), et on
//!   allonge le filtre d'autant pour garder sa qualité.
//! - **Dérive d'horloge** : le régulateur PI de [`super::drift`] ajuste ce rapport
//!   (± 1 000 ppm, en rampe) pour garder le remplissage du ring sur la cible.
//! - **Marge adaptative** : la cible ne descend jamais sous la profondeur mesurée de la
//!   dent de scie du remplissage (gros blocs du DAW) plus une sécurité.
//! - **Underrun** : fondu vers le silence puis réamorçage, avec fondu d'entrée.
//! - **Retard excessif** : saut vers les données récentes avec un fondu croisé, tout de
//!   suite pour une grosse rafale, après 0,3 s pour un retard modéré qui dure (DAW qui
//!   rattrape après un pic, marge qui redescend). Sans ça, le régulateur résorberait le
//!   retard à 1 ms par seconde au plus : la latence resterait haute de longues secondes.
//!
//! Tout ce qui est appelé depuis `render_add` est temps réel : pas d'allocation, pas de
//! verrou. Les buffers sont alloués dans [`SourcePlayer::new`], hors du callback.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use audioadapter_buffers::direct::InterleavedSlice;
use dawsend_core::ring::READABLE_FRAMES;
use dawsend_core::{Consumer, ReadError, clock};
use rubato::{
    Adjustable, Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use super::drift::{DriftController, MAX_CORRECTION};

/// Trames de sortie produites par appel au rééchantillonneur.
const OUT_CHUNK: usize = 128;
/// Longueur de base du filtre sinc (en trames source), multipliée par le rapport de
/// sous-échantillonnage.
const SINC_LEN: usize = 128;
const MAX_SINC_LEN: usize = 2048;
/// Durée des fondus d'entrée et de sortie.
const FADE_MS: f64 = 5.0;
/// Durée du fondu croisé lors d'une resynchronisation.
const XFADE_MS: f64 = 10.0;
/// Retard toléré au-delà de la cible avant de resynchroniser (au moins).
const MIN_EXCESS_MS: f64 = 40.0;
/// Sécurité ajoutée sous la dent de scie du remplissage.
const SAFETY_MS: f64 = 2.0;
/// Lissage des changements de volume.
const GAIN_SMOOTHING_MS: f64 = 20.0;
/// Si le plugin n'a rien écrit depuis ce délai, un ring vide est une pause (DAW arrêté),
/// pas un underrun.
const PAUSE_NS: u64 = 60 * clock::MS;
/// Retard moyen au-delà de la cible qui déclenche un retour rapide, s'il dure.
const RETURN_EXCESS_S: f64 = 0.006;
const RETURN_AFTER_S: f64 = 0.3;

/// État partagé entre le lecteur (callback audio) et le thread de contrôle : réglages
/// écrits par le contrôle, statistiques et vumètres écrits par le callback.
pub struct SourceStats {
    /// Volume linéaire voulu (bits d'un `f32`), appliqué avec un lissage de 20 ms.
    pub gain_bits: AtomicU32,
    pub muted: AtomicBool,
    /// Crête de sortie par canal depuis la dernière lecture (bits d'un `f32`).
    pub peak_bits: [AtomicU32; 2],
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
    /// Trames source en attente (ring + rééchantillonneur), à la dernière mesure.
    pub fill_frames: AtomicU64,
    /// Remplissage lissé, en trames source.
    pub smoothed_fill_frames: AtomicU64,
    /// Marge visée effective, en trames source.
    pub target_frames: AtomicU64,
    /// Correction de dérive appliquée, en ppm (bits d'un `f32`).
    pub correction_ppm_bits: AtomicU32,
    /// 0 = en attente de données, 1 = lecture.
    pub playing: AtomicU32,
}

impl Default for SourceStats {
    fn default() -> Self {
        Self {
            gain_bits: AtomicU32::new(1.0f32.to_bits()),
            muted: AtomicBool::new(false),
            peak_bits: [AtomicU32::new(0), AtomicU32::new(0)],
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            fill_frames: AtomicU64::new(0),
            smoothed_fill_frames: AtomicU64::new(0),
            target_frames: AtomicU64::new(0),
            correction_ppm_bits: AtomicU32::new(0),
            playing: AtomicU32::new(0),
        }
    }
}

impl SourceStats {
    pub fn set_gain(&self, gain: f32) {
        let g = if gain.is_finite() {
            gain.clamp(0.0, 4.0)
        } else {
            1.0
        };
        self.gain_bits.store(g.to_bits(), Ordering::Relaxed);
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain_bits.load(Ordering::Relaxed))
    }

    /// Crêtes (gauche, droite) depuis le dernier appel, puis remise à zéro.
    pub fn take_peaks(&self) -> [f32; 2] {
        [0, 1].map(|c| f32::from_bits(self.peak_bits[c].swap(0, Ordering::Relaxed)))
    }

    pub fn correction_ppm(&self) -> f32 {
        f32::from_bits(self.correction_ppm_bits.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceKey {
    pub index: usize,
    pub generation: u32,
}

/// Pourquoi le rééchantillonneur n'a pas pu fournir de trame.
enum PullError {
    Underrun,
    Overrun,
    Stale,
}

pub struct SourcePlayer {
    consumer: Consumer,
    key: SourceKey,
    stats: Arc<SourceStats>,
    src_rate: f64,
    out_rate: f64,
    resampler: Async<f32>,
    in_buf: Vec<f32>,
    out_buf: Vec<f32>,
    out_pos: usize,
    out_len: usize,
    /// Marge demandée et marge max, en trames source.
    user_target: f64,
    max_target: f64,
    min_excess: f64,
    drift: DriftController,
    playing: bool,
    stale: bool,
    gain: f32,
    fade_inc: f32,
    /// Volume courant (lissé) et coefficient de lissage par trame.
    level: f32,
    level_coef: f32,
    /// Dernier échantillon émis, prolongé en fondu vers zéro quand les données manquent.
    tail: [f32; 2],
    /// Fin de l'ancien flux pendant un fondu croisé.
    xfade: Vec<f32>,
    xfade_len: usize,
    xfade_pos: usize,
    /// Depuis combien de temps le remplissage lissé dépasse la cible de plus de
    /// `RETURN_EXCESS_S`, et retour rapide demandé pour le prochain callback.
    late_for: f64,
    return_pending: bool,
}

impl SourcePlayer {
    /// Crée un lecteur (hors thread audio : alloue).
    pub fn new(consumer: Consumer, output_rate: u32, latency_ms: f64) -> Result<Self, String> {
        let src_rate = f64::from(consumer.sample_rate().max(1));
        let out_rate = f64::from(output_rate.max(1));
        let key = SourceKey {
            index: consumer.index(),
            generation: consumer.generation(),
        };
        let down = (src_rate / out_rate).ceil().max(1.0) as usize;
        let params = SincInterpolationParameters::new(
            (SINC_LEN * down).min(MAX_SINC_LEN),
            WindowFunction::BlackmanHarris2,
        )
        .oversampling_factor(256)
        .interpolation(SincInterpolationType::Linear);
        let resampler = Async::<f32>::new_sinc(
            out_rate / src_rate,
            1.0 + MAX_CORRECTION * 1.5,
            &params,
            OUT_CHUNK,
            2,
            FixedAsync::Output,
        )
        .map_err(|e| format!("rééchantillonneur : {e}"))?;
        let in_max = resampler.input_frames_max();

        let min_excess = src_rate * MIN_EXCESS_MS / 1000.0;
        // La cible doit tenir dans la fenêtre lisible du ring, avec la lecture d'un morceau
        // et la tolérance de retard (ne compte qu'aux fréquences extrêmes).
        let max_target = (READABLE_FRAMES as f64 - in_max as f64 - min_excess)
            .min(READABLE_FRAMES as f64 * 7.0 / 8.0 - in_max as f64 - min_excess)
            .max(0.0);
        let user_target = (src_rate * latency_ms / 1000.0).min(max_target);
        let fade_len = (out_rate * FADE_MS / 1000.0).max(1.0);
        let xfade_frames = (out_rate * XFADE_MS / 1000.0).max(1.0) as usize;

        let stats = Arc::new(SourceStats::default());
        stats
            .target_frames
            .store(user_target as u64, Ordering::Relaxed);
        Ok(Self {
            consumer,
            key,
            stats,
            src_rate,
            out_rate,
            resampler,
            in_buf: vec![0.0; in_max * 2],
            out_buf: vec![0.0; OUT_CHUNK * 2],
            out_pos: 0,
            out_len: 0,
            user_target,
            max_target,
            min_excess,
            drift: DriftController::new(),
            playing: false,
            stale: false,
            gain: 0.0,
            fade_inc: (1.0 / fade_len) as f32,
            level: 1.0,
            level_coef: (1.0 - (-1000.0 / (GAIN_SMOOTHING_MS * out_rate)).exp()) as f32,
            tail: [0.0; 2],
            xfade: vec![0.0; xfade_frames * 2],
            xfade_len: 0,
            xfade_pos: 0,
            late_for: 0.0,
            return_pending: false,
        })
    }

    /// Marge demandée, en ms (peut être inférieure à celle demandée aux fréquences
    /// extrêmes, pour tenir dans le ring).
    pub fn effective_latency_ms(&self) -> f64 {
        self.user_target * 1000.0 / self.src_rate
    }

    /// Délai propre au rééchantillonneur, en ms.
    pub fn resampler_delay_ms(&self) -> f64 {
        self.resampler.output_delay() as f64 * 1000.0 / self.out_rate
    }

    /// `true` si un filtre anti-repliement est appliqué (source plus rapide que la sortie).
    pub fn has_anti_alias(&self) -> bool {
        self.src_rate > self.out_rate
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

    /// Marge visée en ce moment, en trames source : la marge demandée, relevée si la dent
    /// de scie du remplissage (gros blocs du DAW) l'exige.
    fn target(&self) -> f64 {
        // Le ring peut aussi être plus bas que le remplissage mesuré d'un morceau du
        // rééchantillonneur (trames déjà rééchantillonnées, comptées dans la mesure).
        let needed = (self.drift.spread() * 1000.0 + SAFETY_MS) * self.src_rate / 1000.0
            + self.resampler.input_frames_next() as f64;
        self.user_target.max(needed).min(self.max_target)
    }

    /// Retard toléré au-delà de la cible avant de resynchroniser, en trames source.
    fn excess(&self) -> f64 {
        self.min_excess
            .max(3.0 * self.drift.spread() * self.src_rate)
    }

    /// Ajoute la source à `out` (stéréo entrelacée, `frames` trames). Temps réel.
    pub fn render_add(&mut self, out: &mut [f32], frames: usize) {
        let out = &mut out[..frames * 2];
        if self.stale || self.consumer.is_stale() {
            self.stale = true;
            self.render_tail(out);
            return;
        }
        let available = self.consumer.available();
        let target = self.target();
        // Pour démarrer, il faut la cible plus ce que lit ce callback (arrondi au morceau
        // du rééchantillonneur, plus un morceau de sécurité).
        let chunk = self.resampler.input_frames_next() as f64;
        let read = frames as f64 * self.src_rate / self.out_rate;
        let need = read + 2.0 * chunk;
        // Position de départ : après la lecture de ce callback (un morceau d'avance au
        // plus), il reste la cible. Une marge plus large ferait démarrer en retard, que le
        // régulateur mettrait une dizaine de secondes à résorber.
        let place = target + read + chunk;

        if !self.playing {
            if available != u64::MAX
                && available <= READABLE_FRAMES
                && available as f64 >= target + need
            {
                self.start(place);
            } else {
                if available > READABLE_FRAMES {
                    self.consumer.seek_behind_writer(0);
                }
                self.render_tail(out);
                return;
            }
        } else if available == u64::MAX {
            // write_pos remis à zéro : on repart de zéro.
            self.stop();
            self.render_tail(out);
            return;
        } else if self.return_pending || available as f64 > target + need + self.excess() {
            // Trop de retard (DAW qui a envoyé une rafale, sortie bloquée un moment…) :
            // on saute vers les données récentes, en fondu croisé.
            self.stats.overruns.fetch_add(1, Ordering::Relaxed);
            self.resync_with_crossfade(place);
        }

        let wanted = if self.stats.muted.load(Ordering::Relaxed) {
            0.0
        } else {
            self.stats.gain()
        };
        let mut peak = [0.0f32; 2];
        for i in 0..frames {
            let f = match self.pull() {
                Ok(f) => f,
                Err(e) => {
                    match e {
                        PullError::Underrun => {
                            let age = self.consumer.producer_age_ns(clock::now_ns());
                            if age < PAUSE_NS {
                                self.stats.underruns.fetch_add(1, Ordering::Relaxed);
                                // Les blocs du DAW arrivent plus espacés que prévu : la
                                // marge adaptative doit couvrir au moins ce qui a manqué.
                                self.drift
                                    .note_shortfall((frames - i) as f64 / self.out_rate);
                            }
                        }
                        PullError::Overrun => {
                            self.stats.overruns.fetch_add(1, Ordering::Relaxed);
                        }
                        PullError::Stale => self.stale = true,
                    }
                    self.stop();
                    self.render_tail(&mut out[i * 2..]);
                    self.store_peaks(peak);
                    return;
                }
            };
            if self.gain < 1.0 {
                self.gain = (self.gain + self.fade_inc).min(1.0);
            }
            self.level += (wanted - self.level) * self.level_coef;
            let g = self.gain * self.level;
            let mut s = [f[0] * g, f[1] * g];
            if self.xfade_pos < self.xfade_len {
                let g = (self.xfade_pos + 1) as f32 / (self.xfade_len + 1) as f32;
                let p = self.xfade_pos * 2;
                s[0] = s[0] * g + self.xfade[p] * (1.0 - g);
                s[1] = s[1] * g + self.xfade[p + 1] * (1.0 - g);
                self.xfade_pos += 1;
            }
            out[i * 2] += s[0];
            out[i * 2 + 1] += s[1];
            peak[0] = peak[0].max(s[0].abs());
            peak[1] = peak[1].max(s[1].abs());
            self.tail = s;
        }
        self.store_peaks(peak);

        // Régulation de la dérive : remplissage = ring + trames déjà rééchantillonnées
        // pas encore émises (ramenées en trames source).
        let pending_out = (self.out_len - self.out_pos) as f64;
        let fill = self.consumer.available().min(READABLE_FRAMES) as f64
            + pending_out * self.src_rate / self.out_rate;
        let dt = frames as f64 / self.out_rate;
        let corr = self
            .drift
            .update(fill / self.src_rate, target / self.src_rate, dt);
        let late = self.drift.smoothed() - target / self.src_rate;
        if late > RETURN_EXCESS_S {
            self.late_for += dt;
            if self.late_for >= RETURN_AFTER_S {
                self.return_pending = true;
            }
        } else {
            self.late_for = 0.0;
        }
        // corr > 0 : consommer la source plus vite, donc moins de sortie par trame source.
        let _ = self
            .resampler
            .set_resample_ratio_relative(1.0 / (1.0 + corr), true);

        let s = &self.stats;
        s.fill_frames.store(fill as u64, Ordering::Relaxed);
        s.smoothed_fill_frames.store(
            (self.drift.smoothed() * self.src_rate) as u64,
            Ordering::Relaxed,
        );
        s.target_frames.store(target as u64, Ordering::Relaxed);
        s.correction_ppm_bits
            .store(((corr * 1e6) as f32).to_bits(), Ordering::Relaxed);
    }

    fn store_peaks(&self, peak: [f32; 2]) {
        for (c, p) in peak.into_iter().enumerate() {
            let slot = &self.stats.peak_bits[c];
            if p > f32::from_bits(slot.load(Ordering::Relaxed)) {
                slot.store(p.to_bits(), Ordering::Relaxed);
            }
        }
    }

    /// Démarre la lecture `behind` trames derrière l'écrivain.
    fn start(&mut self, behind: f64) {
        self.consumer.seek_behind_writer(behind as u64);
        self.resampler.reset();
        let _ = self
            .resampler
            .set_resample_ratio_relative(1.0 / (1.0 + self.drift.correction()), false);
        self.out_pos = 0;
        self.out_len = 0;
        self.xfade_len = 0;
        self.xfade_pos = 0;
        self.gain = 0.0;
        self.late_for = 0.0;
        self.return_pending = false;
        self.playing = true;
        self.drift.restart();
        self.stats.playing.store(1, Ordering::Relaxed);
    }

    fn stop(&mut self) {
        self.playing = false;
        self.xfade_len = 0;
        self.stats.playing.store(0, Ordering::Relaxed);
    }

    /// Garde la suite de l'ancien flux pour le fondu croisé, puis saute vers les données
    /// récentes. Le rééchantillonneur n'est pas réinitialisé : son historique assure une
    /// transition filtrée, masquée par le fondu croisé.
    fn resync_with_crossfade(&mut self, behind: f64) {
        let n = self.xfade.len() / 2;
        let mut len = 0;
        while len < n {
            match self.pull() {
                Ok(f) => {
                    self.xfade[len * 2] = f[0] * self.gain;
                    self.xfade[len * 2 + 1] = f[1] * self.gain;
                    len += 1;
                }
                Err(_) => break,
            }
        }
        self.xfade_len = len;
        self.xfade_pos = 0;
        self.late_for = 0.0;
        self.return_pending = false;
        self.consumer.seek_behind_writer(behind as u64);
        self.out_pos = 0;
        self.out_len = 0;
        self.drift.restart();
    }

    /// Trame suivante du flux rééchantillonné.
    #[inline]
    fn pull(&mut self) -> Result<[f32; 2], PullError> {
        if self.out_pos >= self.out_len {
            self.refill()?;
        }
        let p = self.out_pos * 2;
        self.out_pos += 1;
        Ok([self.out_buf[p], self.out_buf[p + 1]])
    }

    fn refill(&mut self) -> Result<(), PullError> {
        let n = self.resampler.input_frames_next();
        match self.consumer.read_stereo(&mut self.in_buf, n) {
            Ok(()) => {}
            Err(ReadError::Underrun { .. }) => return Err(PullError::Underrun),
            Err(ReadError::Overrun) => return Err(PullError::Overrun),
            Err(ReadError::Stale) => return Err(PullError::Stale),
        }
        let Ok(input) = InterleavedSlice::new(&self.in_buf[..n * 2], 2, n) else {
            return Err(PullError::Stale);
        };
        let Ok(mut output) = InterleavedSlice::new_mut(&mut self.out_buf, 2, OUT_CHUNK) else {
            return Err(PullError::Stale);
        };
        match self
            .resampler
            .process_into_buffer(&input, &mut output, None)
        {
            Ok((_, produced)) if produced > 0 => {
                self.out_len = produced;
                self.out_pos = 0;
                Ok(())
            }
            _ => Err(PullError::Stale),
        }
    }

    /// Prolonge le dernier échantillon en fondu linéaire vers zéro (évite le clic).
    fn render_tail(&mut self, out: &mut [f32]) {
        if self.tail == [0.0; 2] {
            return;
        }
        let mut g = 1.0f32;
        for o in out.as_chunks_mut::<2>().0 {
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
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0).unwrap();
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
        assert!(out.iter().all(|v| v.abs() <= 0.505));
    }

    #[test]
    fn resamples_44100_to_48000() {
        let (_s, region, claim) = setup("b", 44_100);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0).unwrap();
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
            .as_chunks::<2>()
            .0
            .windows(2)
            .map(|w| (w[0], w[1]))
            .map(|(a, b)| (a[0] - b[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_jump < 0.05, "saut {max_jump}");
    }

    /// Joue une sinusoïde de `freq` Hz publiée à `src_rate` sur une sortie à `out_rate`,
    /// horloges identiques. Renvoie (crête de sortie, underruns, resynchros).
    fn play_tone(name: &str, src_rate: u32, out_rate: u32, freq: f32) -> (f32, u64, u64) {
        let (_s, region, claim) = setup(name, src_rate);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p =
            SourcePlayer::new(Consumer::new(region.clone(), &meta), out_rate, 10.0).unwrap();
        let period = (out_rate / 100) as usize; // 10 ms
        let mut out = vec![0.0; period * 2];
        let mut written = 0usize;
        let mut peak = 0.0f32;
        for i in 0..300 {
            let target_src = (i + 1) * src_rate as usize / 100 + src_rate as usize / 50;
            while written < target_src {
                let b: Vec<f32> = (written..written + 256)
                    .map(|k| {
                        let cycles = (k as f64 * f64::from(freq) / f64::from(src_rate)).fract();
                        ((cycles * std::f64::consts::TAU).sin() * 0.5) as f32
                    })
                    .collect();
                prod.write_planar(&[b.clone(), b]);
                written += 256;
            }
            out.fill(0.0);
            p.render_add(&mut out, period);
            if i > 100 {
                peak = out.iter().fold(peak, |m, v| m.max(v.abs()));
            }
        }
        let st = p.stats();
        (
            peak,
            st.underruns.load(Ordering::Relaxed),
            st.overruns.load(Ordering::Relaxed),
        )
    }

    #[test]
    fn high_sample_rates() {
        for (i, (src, out)) in [
            (96_000, 48_000),
            (192_000, 48_000),
            (88_200, 44_100),
            (176_400, 48_000),
            (384_000, 48_000),
            (48_000, 96_000),
            (48_000, 192_000),
            (192_000, 192_000),
        ]
        .into_iter()
        .enumerate()
        {
            let (peak, under, over) = play_tone(&format!("hr{i}"), src, out, 1000.0);
            assert_eq!((under, over), (0, 0), "{src} → {out}");
            assert!(
                (0.45..=0.51).contains(&peak),
                "{src} → {out} : crête {peak}"
            );
        }
    }

    #[test]
    fn ultrasonic_content_is_not_folded_back() {
        // 40 kHz à 192 kHz se replierait sur 8 kHz à 48 kHz sans filtre.
        let (peak, under, over) = play_tone("alias", 192_000, 48_000, 40_000.0);
        assert_eq!((under, over), (0, 0));
        assert!(peak < 0.01, "repliement audible : crête {peak}");
    }

    #[test]
    fn latency_is_clamped_at_extreme_rates() {
        let (_s, region, claim) = setup("clamp", 768_000);
        let meta = read_meta(&region, claim.index()).unwrap();
        let p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 200.0).unwrap();
        assert!(p.effective_latency_ms() < 200.0);
        assert!(p.has_anti_alias());
    }

    #[test]
    fn underrun_fades_to_silence() {
        let (_s, region, claim) = setup("c", 48_000);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 5.0).unwrap();
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
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0).unwrap();
        drop(claim);
        let mut out = vec![0.0; 64];
        p.render_add(&mut out, 32);
        assert!(p.is_stale());
    }

    /// Simule un DAW dont l'horloge est décalée de `drift_ppm`, qui livre des blocs de
    /// `block` trames, et une sortie qui demande 10 ms à chaque callback. Renvoie le
    /// lecteur et (underruns, resynchros) comptés sur la seconde moitié.
    fn run_clocks(
        name: &str,
        src_rate: u32,
        out_rate: u32,
        block: usize,
        drift_ppm: f64,
        seconds: f64,
        latency_ms: f64,
    ) -> (SourcePlayer, u64, u64) {
        let (s, region, claim) = setup(name, src_rate);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p =
            SourcePlayer::new(Consumer::new(region.clone(), &meta), out_rate, latency_ms).unwrap();
        let period = (out_rate / 100) as usize;
        let periods = (seconds * 100.0) as usize;
        let mut out = vec![0.0; period * 2];
        let src_clock = f64::from(src_rate) * (1.0 + drift_ppm * 1e-6);
        let mut written = 0usize;
        let mut half = (0, 0);
        for i in 0..periods {
            // Le DAW livre un bloc quand il est complet (fin de bloc).
            let due = ((i + 1) as f64 / 100.0 * src_clock) as usize;
            while written + block <= due {
                let b: Vec<f32> = (written..written + block)
                    .map(|k| {
                        ((k as f64 * 440.0 / src_clock).fract() * std::f64::consts::TAU).sin()
                            as f32
                            * 0.5
                    })
                    .collect();
                prod.write_planar(&[b.clone(), b]);
                written += block;
            }
            out.fill(0.0);
            p.render_add(&mut out, period);
            if i == periods / 2 {
                let st = p.stats();
                half = (
                    st.underruns.load(Ordering::Relaxed),
                    st.overruns.load(Ordering::Relaxed),
                );
            }
        }
        let st = p.stats();
        let under = st.underruns.load(Ordering::Relaxed) - half.0;
        let over = st.overruns.load(Ordering::Relaxed) - half.1;
        drop(claim);
        drop(s);
        (p, under, over)
    }

    #[test]
    fn compensates_clock_drift() {
        for (i, (src, out, ppm)) in [
            (48_000, 48_000, 300.0),
            (48_000, 48_000, -250.0),
            (44_100, 48_000, 120.0),
            (96_000, 48_000, -80.0),
        ]
        .into_iter()
        .enumerate()
        {
            let (p, under, over) =
                run_clocks(&format!("drift{i}"), src, out, 128, ppm, 240.0, 10.0);
            let st = p.stats();
            assert_eq!((under, over), (0, 0), "{src} → {out}, {ppm} ppm");
            assert_eq!(
                st.underruns.load(Ordering::Relaxed),
                0,
                "{src} → {out}, {ppm} ppm"
            );
            let corr = f64::from(st.correction_ppm());
            assert!(
                (corr - ppm).abs() < 25.0,
                "{src} → {out}, {ppm} ppm : correction {corr}"
            );
            let fill_ms =
                st.smoothed_fill_frames.load(Ordering::Relaxed) as f64 * 1000.0 / f64::from(src);
            assert!(
                (fill_ms - 10.0).abs() < 1.5,
                "{src} → {out}, {ppm} ppm : remplissage {fill_ms} ms"
            );
        }
    }

    #[test]
    fn big_daw_blocks_raise_the_target() {
        // Buffer ASIO de 2 048 trames (43 ms) avec une marge demandée de 10 ms.
        let (p, under, over) = run_clocks("bigblock", 48_000, 48_000, 2048, 150.0, 120.0, 10.0);
        assert_eq!((under, over), (0, 0));
        let target = p.stats().target_frames.load(Ordering::Relaxed);
        assert!(target > 1024, "cible {target} trames");
    }

    #[test]
    fn resync_is_crossfaded() {
        let (_s, region, claim) = setup("xfade", 48_000);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0).unwrap();
        let mut out = vec![0.0; 480 * 2];
        let mut all = Vec::new();
        let mut written = 0usize;
        for i in 0..300 {
            let mut due = (i + 1) * 480 + 960;
            if i >= 150 {
                due += 48_000 / 10; // rafale de 100 ms : retard excessif
            }
            while written < due {
                let b = sine(written, 64, 48_000.0);
                prod.write_planar(&[b.clone(), b]);
                written += 64;
            }
            out.fill(0.0);
            p.render_add(&mut out, 480);
            if i > 50 {
                all.extend(out.as_chunks::<2>().0.iter().map(|f| f[0]));
            }
        }
        let st = p.stats();
        assert_eq!(st.overruns.load(Ordering::Relaxed), 1);
        assert_eq!(st.underruns.load(Ordering::Relaxed), 0);
        // Sinusoïde de 440 Hz, amplitude 0,5 : au plus 0,029 d'une trame à l'autre.
        let max_jump = all
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_jump < 0.06, "saut {max_jump}");
    }

    /// DAW régulier (blocs de 128), sortie de 10 ms, même horloge. À la période `at`, le
    /// DAW se fige `stall` périodes et perd ces blocs ; ou bien (`extra` > 0) la sortie
    /// s'est figée et trouve `extra` trames de plus dans le ring. Renvoie le lecteur et le
    /// remplissage (ms) à chaque période.
    fn run_spike(name: &str, at: usize, stall: usize, extra: usize) -> (SourcePlayer, Vec<f64>) {
        let (_s, region, claim) = setup(name, 48_000);
        let prod = claim.producer();
        let meta = read_meta(&region, claim.index()).unwrap();
        let mut p = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0).unwrap();
        let mut out = vec![0.0; 480 * 2];
        let mut written = 0usize;
        let mut offset = 0isize;
        let mut fills = Vec::new();
        for i in 0..1200 {
            let stalled = i >= at && i < at + stall;
            if stalled {
                offset -= 480;
            }
            if i == at {
                offset += extra as isize;
            }
            if !stalled {
                let due = ((i + 1) * 480) as isize + offset;
                while (written + 128) as isize <= due {
                    let b = sine(written, 128, 48_000.0);
                    prod.write_planar(&[b.clone(), b]);
                    written += 128;
                }
            }
            out.fill(0.0);
            p.render_add(&mut out, 480);
            fills.push(p.stats().fill_frames.load(Ordering::Relaxed) as f64 / 48.0);
        }
        (p, fills)
    }

    fn mean(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() / v.len() as f64
    }

    #[test]
    fn latency_recovers_quickly_after_an_output_stall() {
        // La sortie s'est figée 30 ms (pic de charge) : 30 ms de retard en trop, sous le
        // seuil de resynchro immédiate. En moins de 0,6 s, retour sur la cible (sans ça :
        // 30 s à 1 ms par seconde).
        let (p, fills) = run_spike("spike_out", 600, 0, 1440);
        let st = p.stats();
        assert_eq!(st.underruns.load(Ordering::Relaxed), 0);
        let before = mean(&fills[300..600]);
        let after = mean(&fills[660..760]);
        assert!(after - before < 1.0, "avant {before} ms, après {after} ms");
        assert_eq!(st.overruns.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn isolated_dropout_does_not_raise_the_latency() {
        // Pic de 40 ms où le DAW perd ses blocs : une coupure, puis la lecture reprend sur
        // la même marge, sans latence durablement relevée.
        let (p, fills) = run_spike("spike_drop", 600, 4, 0);
        let st = p.stats();
        assert_eq!(st.underruns.load(Ordering::Relaxed), 1);
        let before = mean(&fills[300..600]);
        let after = mean(&fills[1000..]);
        assert!(after - before < 1.0, "avant {before} ms, après {after} ms");
        let target = st.target_frames.load(Ordering::Relaxed) as f64 / 48.0;
        assert!(target < 12.0, "cible {target} ms");
    }
}
