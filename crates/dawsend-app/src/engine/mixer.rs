//! Mixeur côté callback audio, piloté par le thread de contrôle via des files lock-free.
//!
//! Le thread de contrôle crée les `SourcePlayer` (allocation) et les envoie au callback.
//! Les lecteurs retirés repartent vers le thread de contrôle pour y être détruits :
//! le callback ne libère jamais de mémoire.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use dawsend_core::layout::SLOT_COUNT;
use rtrb::{Consumer as RxQueue, Producer as TxQueue, RingBuffer};

use super::source::{SourceKey, SourcePlayer};

/// Réglages et vumètres du mix, partagés avec le thread de contrôle.
pub struct MixerShared {
    master_gain_bits: AtomicU32,
    pub muted: AtomicBool,
    peak_bits: [AtomicU32; 2],
}

impl Default for MixerShared {
    fn default() -> Self {
        Self {
            master_gain_bits: AtomicU32::new(1.0f32.to_bits()),
            muted: AtomicBool::new(false),
            peak_bits: [AtomicU32::new(0), AtomicU32::new(0)],
        }
    }
}

impl MixerShared {
    pub fn set_master_gain(&self, gain: f32) {
        let g = if gain.is_finite() {
            gain.clamp(0.0, 4.0)
        } else {
            1.0
        };
        self.master_gain_bits.store(g.to_bits(), Ordering::Relaxed);
    }

    pub fn master_gain(&self) -> f32 {
        f32::from_bits(self.master_gain_bits.load(Ordering::Relaxed))
    }

    /// Crêtes (gauche, droite) du mix depuis le dernier appel, puis remise à zéro.
    pub fn take_peaks(&self) -> [f32; 2] {
        [0, 1].map(|c| f32::from_bits(self.peak_bits[c].swap(0, Ordering::Relaxed)))
    }
}

/// Seuil au-delà duquel le limiteur doux arrondit le signal.
const LIMIT_KNEE: f32 = 0.8;

/// Limiteur doux : linéaire sous le seuil, puis courbe tanh qui tend vers 1 sans jamais
/// le dépasser (pas de saturation numérique quand plusieurs sources s'additionnent).
#[inline]
fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT_KNEE {
        x
    } else {
        let room = 1.0 - LIMIT_KNEE;
        (LIMIT_KNEE + room * ((a - LIMIT_KNEE) / room).tanh()).copysign(x)
    }
}

pub enum Command {
    Add(Box<SourcePlayer>),
    Remove(SourceKey),
}

/// Partie « callback » : à donner au backend de sortie.
pub struct Mixer {
    // Les lecteurs restent dans leur `Box` : les en sortir libérerait la `Box` dans le
    // callback, ce qui est interdit.
    #[allow(clippy::vec_box)]
    players: Vec<Box<SourcePlayer>>,
    commands: RxQueue<Command>,
    trash: TxQueue<Box<SourcePlayer>>,
    shared: Arc<MixerShared>,
    /// Volume général courant (lissé).
    level: f32,
}

/// Partie « thread de contrôle ».
pub struct MixerControl {
    commands: TxQueue<Command>,
    trash: RxQueue<Box<SourcePlayer>>,
    shared: Arc<MixerShared>,
}

pub fn mixer() -> (Mixer, MixerControl) {
    let (cmd_tx, cmd_rx) = RingBuffer::new(64);
    let (trash_tx, trash_rx) = RingBuffer::new(4 * SLOT_COUNT);
    let shared = Arc::new(MixerShared::default());
    (
        Mixer {
            players: Vec::with_capacity(SLOT_COUNT),
            commands: cmd_rx,
            trash: trash_tx,
            shared: shared.clone(),
            level: 1.0,
        },
        MixerControl {
            commands: cmd_tx,
            trash: trash_rx,
            shared,
        },
    )
}

impl MixerControl {
    pub fn shared(&self) -> &Arc<MixerShared> {
        &self.shared
    }

    /// Envoie une commande au callback. Renvoie la commande si la file est pleine.
    pub fn send(&mut self, cmd: Command) -> Result<(), Command> {
        self.commands
            .push(cmd)
            .map_err(|rtrb::PushError::Full(c)| c)
    }

    /// Détruit les lecteurs rendus par le callback. Renvoie leurs clés.
    pub fn collect_garbage(&mut self) -> Vec<SourceKey> {
        let mut keys = Vec::new();
        while let Ok(p) = self.trash.pop() {
            keys.push(p.key());
        }
        keys
    }
}

impl Mixer {
    fn discard(&mut self, p: Box<SourcePlayer>) {
        if let Err(rtrb::PushError::Full(p)) = self.trash.push(p) {
            // File pleine (ne devrait pas arriver) : on fuit plutôt que de libérer ici.
            std::mem::forget(p);
        }
    }

    /// Remplit `out` (stéréo entrelacée, `frames` trames). Temps réel.
    pub fn render(&mut self, out: &mut [f32], frames: usize) {
        while let Ok(cmd) = self.commands.pop() {
            match cmd {
                Command::Add(p) => {
                    if self.players.len() < self.players.capacity() {
                        self.players.push(p);
                    } else {
                        self.discard(p);
                    }
                }
                Command::Remove(key) => {
                    if let Some(i) = self.players.iter().position(|p| p.key() == key) {
                        let p = self.players.swap_remove(i);
                        self.discard(p);
                    }
                }
            }
        }

        let out = &mut out[..frames * 2];
        out.fill(0.0);
        let mut i = 0;
        while i < self.players.len() {
            self.players[i].render_add(out, frames);
            if self.players[i].is_stale() {
                let p = self.players.swap_remove(i);
                self.discard(p);
            } else {
                i += 1;
            }
        }
        let wanted = if self.shared.muted.load(Ordering::Relaxed) {
            0.0
        } else {
            self.shared.master_gain()
        };
        // Lissage d'environ 20 ms à 48 kHz.
        const COEF: f32 = 0.001;
        let mut peak = [0.0f32; 2];
        for frame in out.chunks_exact_mut(2) {
            self.level += (wanted - self.level) * COEF;
            for (c, s) in frame.iter_mut().enumerate() {
                let v = soft_limit(*s * self.level);
                *s = if v.is_finite() { v } else { 0.0 };
                peak[c] = peak[c].max(s.abs());
            }
        }
        for (c, p) in peak.into_iter().enumerate() {
            let slot = &self.shared.peak_bits[c];
            if p > f32::from_bits(slot.load(Ordering::Relaxed)) {
                slot.store(p.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_limit_never_exceeds_one() {
        for i in 0..1000 {
            let x = i as f32 * 0.01;
            let y = soft_limit(x);
            assert!((0.0..=1.0).contains(&y));
            assert_eq!(soft_limit(-x), -y);
            if x <= LIMIT_KNEE {
                assert_eq!(y, x);
            }
        }
        // Continue au seuil et croissante.
        assert!((soft_limit(LIMIT_KNEE + 1e-4) - LIMIT_KNEE).abs() < 1e-3);
        assert!(soft_limit(1.5) > soft_limit(1.2));
    }
}
