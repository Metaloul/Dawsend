//! Ring SPSC « avec perte » entre le thread audio du plugin et l'appli.
//!
//! - L'écrivain n'attend jamais le lecteur et ne lit pas sa position : il écrase.
//! - Les positions sont des compteurs de trames sur 64 bits qui ne rebouclent jamais.
//! - Le lecteur valide après coup que les données copiées n'ont pas été écrasées
//!   pendant la copie (principe du seqlock).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering, fence};

use crate::clock;
use crate::layout::*;
use crate::region::Region;
use crate::registry::SlotMeta;

const MASK: u64 = RING_CAPACITY_FRAMES as u64 - 1;

/// Taille max d'un bloc du DAW couverte par la marge de sécurité (les DAW plafonnent
/// en pratique à 8 192 échantillons).
pub const WRITER_MARGIN_FRAMES: u64 = 16_384;

/// Fenêtre lisible sans risque : l'écrivain peut être en train d'écrire jusqu'à un bloc
/// au-delà de `write_pos`, on garde donc une marge d'un gros bloc.
/// 49 152 trames = 1 s à 48 kHz, 256 ms à 192 kHz.
pub const READABLE_FRAMES: u64 = RING_CAPACITY_FRAMES as u64 - WRITER_MARGIN_FRAMES;

#[inline]
fn sample_index(pos: u64, ch: usize) -> usize {
    (pos & MASK) as usize * MAX_CHANNELS + ch
}

/// Côté plugin : écrit dans le ring d'un slot. **Utilisable sur le thread audio** :
/// aucune allocation, aucun verrou, aucun appel système bloquant.
pub struct Producer {
    region: Arc<Region>,
    index: usize,
    channels: usize,
}

impl Producer {
    pub(crate) fn new(region: Arc<Region>, index: usize, channels: usize) -> Self {
        Self {
            region,
            index,
            channels: channels.clamp(1, MAX_CHANNELS),
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Écrit un bloc planaire (un slice par canal, tous de même longueur).
    /// Les canaux manquants sont écrits à zéro, les canaux en trop ignorés.
    #[inline]
    pub fn write_planar<S: AsRef<[f32]>>(&self, chans: &[S]) {
        let n = chans.first().map_or(0, |c| c.as_ref().len());
        self.write_with(n, |ch, i| chans.get(ch).map_or(0.0, |c| c.as_ref()[i]));
    }

    /// Écrit `n` trames de silence (envoi désactivé : le flux reste continu).
    #[inline]
    pub fn write_silence(&self, n: usize) {
        self.write_with(n, |_, _| 0.0);
    }

    #[inline]
    fn write_with(&self, n: usize, sample: impl Fn(usize, usize) -> f32) {
        let slot = self.region.slot(self.index);
        let ring = self.region.ring(self.index);
        let w = slot.write_pos.load(Ordering::Relaxed); // seul écrivain
        // Un bloc plus grand que le ring : seules les dernières trames comptent.
        let skip = n.saturating_sub(RING_CAPACITY_FRAMES);
        for i in skip..n {
            let pos = w + i as u64;
            for ch in 0..self.channels {
                ring[sample_index(pos, ch)].store(sample(ch, i).to_bits(), Ordering::Relaxed);
            }
        }
        slot.write_pos.store(w + n as u64, Ordering::Release);
        slot.producer_heartbeat_ns
            .store(clock::now_ns(), Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Pas assez de données : `available` trames seulement.
    Underrun { available: u64 },
    /// Le lecteur est trop en retard (ou les données ont été écrasées pendant la copie).
    Overrun,
    /// Le slot a été libéré ou reconfiguré : recréer le consommateur.
    Stale,
}

/// Côté appli : lit le ring d'un slot. Utilisable dans le callback audio de l'appli.
pub struct Consumer {
    region: Arc<Region>,
    index: usize,
    generation: u32,
    channels: usize,
    sample_rate: u32,
    read_pos: u64,
}

impl Consumer {
    /// Crée un lecteur positionné sur les données les plus récentes.
    pub fn new(region: Arc<Region>, meta: &SlotMeta) -> Self {
        let mut c = Self {
            region,
            index: meta.index,
            generation: meta.generation,
            channels: (meta.channels as usize).clamp(1, MAX_CHANNELS),
            sample_rate: meta.sample_rate,
            read_pos: 0,
        };
        c.read_pos = c.write_pos();
        c
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[inline]
    fn write_pos(&self) -> u64 {
        self.region
            .slot(self.index)
            .write_pos
            .load(Ordering::Acquire)
    }

    /// `true` si le slot a été libéré ou reconfiguré depuis la création du lecteur.
    #[inline]
    pub fn is_stale(&self) -> bool {
        let s = self.region.slot(self.index);
        s.state.load(Ordering::Acquire) != SLOT_ACTIVE
            || s.generation.load(Ordering::Relaxed) != self.generation
    }

    /// Trames disponibles à la lecture (peut dépasser `READABLE_FRAMES` en cas de retard).
    #[inline]
    pub fn available(&self) -> u64 {
        let w = self.write_pos();
        if w < self.read_pos {
            // write_pos remis à zéro (reconfiguration) : tout est à jeter.
            u64::MAX
        } else {
            w - self.read_pos
        }
    }

    /// Place la lecture `frames` trames derrière l'écrivain.
    #[inline]
    pub fn seek_behind_writer(&mut self, frames: u64) {
        let w = self.write_pos();
        self.read_pos = w.saturating_sub(frames.min(READABLE_FRAMES));
    }

    /// Âge du dernier bloc écrit par le plugin, en ns.
    #[inline]
    pub fn producer_age_ns(&self, now_ns: u64) -> u64 {
        now_ns.saturating_sub(
            self.region
                .slot(self.index)
                .producer_heartbeat_ns
                .load(Ordering::Relaxed),
        )
    }

    /// Lit `frames` trames en stéréo entrelacée dans `dst[..frames * 2]`
    /// (une source mono est dupliquée sur les deux canaux).
    ///
    /// En cas d'erreur, `dst` peut contenir des données partielles et la position de
    /// lecture n'avance pas.
    pub fn read_stereo(&mut self, dst: &mut [f32], frames: usize) -> Result<(), ReadError> {
        assert!(dst.len() >= frames * 2);
        if self.is_stale() {
            return Err(ReadError::Stale);
        }
        let avail = self.available();
        if avail > READABLE_FRAMES {
            return Err(ReadError::Overrun);
        }
        if avail < frames as u64 {
            return Err(ReadError::Underrun { available: avail });
        }
        let ring = self.region.ring(self.index);
        let r = self.read_pos;
        let load = |a: &AtomicU32| f32::from_bits(a.load(Ordering::Relaxed));
        for (i, out) in dst[..frames * 2]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .enumerate()
        {
            let pos = r + i as u64;
            let l = load(&ring[sample_index(pos, 0)]);
            let rr = if self.channels >= 2 {
                load(&ring[sample_index(pos, 1)])
            } else {
                l
            };
            out[0] = l;
            out[1] = rr;
        }
        // Validation : l'écrivain a-t-il pu écraser ce qu'on vient de copier ?
        fence(Ordering::Acquire);
        let w2 = self
            .region
            .slot(self.index)
            .write_pos
            .load(Ordering::Relaxed);
        if w2 < r || w2 - r > READABLE_FRAMES {
            return Err(ReadError::Overrun);
        }
        self.read_pos = r + frames as u64;
        self.region
            .slot(self.index)
            .read_pos
            .store(self.read_pos, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::tests::TestRegion;
    use crate::registry::{SlotClaim, read_meta};

    fn ramp(start: usize, n: usize, ch: f32) -> Vec<f32> {
        (start..start + n).map(|i| i as f32 + ch).collect()
    }

    #[test]
    fn write_then_read_stereo() {
        let t = TestRegion::new();
        let region = t.open();
        let claim = SlotClaim::claim(region.clone(), 48_000, 2, "g").unwrap();
        let mut cons = Consumer::new(region.clone(), &read_meta(&region, claim.index()).unwrap());
        let prod = claim.producer();

        prod.write_planar(&[ramp(0, 64, 0.0), ramp(0, 64, 0.5)]);
        assert_eq!(cons.available(), 64);
        let mut buf = vec![0.0; 128];
        cons.read_stereo(&mut buf, 64).unwrap();
        assert_eq!(&buf[..4], &[0.0, 0.5, 1.0, 1.5]);
        assert_eq!(buf[126], 63.0);
        assert_eq!(cons.available(), 0);
        assert_eq!(
            cons.read_stereo(&mut buf, 1),
            Err(ReadError::Underrun { available: 0 })
        );
    }

    #[test]
    fn mono_is_duplicated() {
        let t = TestRegion::new();
        let region = t.open();
        let claim = SlotClaim::claim(region.clone(), 48_000, 1, "v").unwrap();
        let mut cons = Consumer::new(region.clone(), &read_meta(&region, claim.index()).unwrap());
        claim.producer().write_planar(&[ramp(10, 4, 0.0)]);
        let mut buf = vec![0.0; 8];
        cons.read_stereo(&mut buf, 4).unwrap();
        assert_eq!(buf, vec![10.0, 10.0, 11.0, 11.0, 12.0, 12.0, 13.0, 13.0]);
    }

    #[test]
    fn wraparound_and_overrun() {
        let t = TestRegion::new();
        let region = t.open();
        let claim = SlotClaim::claim(region.clone(), 48_000, 2, "g").unwrap();
        let mut cons = Consumer::new(region.clone(), &read_meta(&region, claim.index()).unwrap());
        let prod = claim.producer();
        let block = 1000;
        let mut written = 0;
        // Plusieurs tours de ring en lisant au fur et à mesure : jamais d'erreur.
        let mut buf = vec![0.0; block * 2];
        for _ in 0..(3 * RING_CAPACITY_FRAMES / block) {
            prod.write_planar(&[ramp(written, block, 0.0), ramp(written, block, 0.0)]);
            cons.read_stereo(&mut buf, block).unwrap();
            assert_eq!(buf[0], written as f32);
            written += block;
        }
        // Le lecteur s'arrête, l'écrivain continue : dépassement détecté.
        for _ in 0..(READABLE_FRAMES as usize / block + 1) {
            prod.write_silence(block);
        }
        assert_eq!(cons.read_stereo(&mut buf, block), Err(ReadError::Overrun));
        cons.seek_behind_writer(480);
        assert_eq!(cons.available(), 480);
        cons.read_stereo(&mut buf, 480).unwrap();
    }

    #[test]
    fn reconfigure_makes_consumer_stale() {
        let t = TestRegion::new();
        let region = t.open();
        let claim = SlotClaim::claim(region.clone(), 48_000, 2, "g").unwrap();
        let mut cons = Consumer::new(region.clone(), &read_meta(&region, claim.index()).unwrap());
        claim.reconfigure(44_100, 2).unwrap();
        let mut buf = vec![0.0; 2];
        assert_eq!(cons.read_stereo(&mut buf, 1), Err(ReadError::Stale));
        drop(claim);
        assert!(cons.is_stale());
    }

    /// Écrivain et lecteur sur deux threads et deux mappings distincts de la même zone :
    /// aucune trame lue ne doit être corrompue.
    #[test]
    fn concurrent_two_mappings() {
        let t = TestRegion::new();
        let region_w = t.open();
        let region_r = t.open();
        let claim = SlotClaim::claim(region_w.clone(), 48_000, 2, "g").unwrap();
        let meta = read_meta(&region_r, claim.index()).unwrap();
        let prod = claim.producer();
        let total = 400_000usize;
        let writer = std::thread::spawn(move || {
            let mut n = 0;
            while n < total {
                let l: Vec<f32> = (n..n + 128).map(|i| i as f32).collect();
                let r: Vec<f32> = l.iter().map(|v| -v).collect();
                prod.write_planar(&[l, r]);
                n += 128;
                if n % 4096 == 0 {
                    std::thread::yield_now();
                }
            }
        });
        let mut cons = Consumer::new(region_r, &meta);
        let mut buf = vec![0.0; 256 * 2];
        let mut checked = 0;
        while !writer.is_finished() || cons.available() >= 256 {
            match cons.read_stereo(&mut buf, 256) {
                Ok(()) => {
                    let first = buf[0];
                    for (i, fr) in buf.as_chunks::<2>().0.iter().enumerate() {
                        assert_eq!(fr[0], first + i as f32, "trame déchirée");
                        assert_eq!(fr[1], -fr[0]);
                    }
                    checked += 1;
                }
                Err(ReadError::Underrun { .. }) => std::thread::yield_now(),
                Err(ReadError::Overrun) => cons.seek_behind_writer(1024),
                Err(ReadError::Stale) => panic!("stale"),
            }
        }
        writer.join().unwrap();
        assert!(checked > 0);
        drop(claim);
    }
}
