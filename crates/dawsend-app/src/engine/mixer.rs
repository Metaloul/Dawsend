//! Mixeur côté callback audio, piloté par le thread de contrôle via des files lock-free.
//!
//! Le thread de contrôle crée les `SourcePlayer` (allocation) et les envoie au callback.
//! Les lecteurs retirés repartent vers le thread de contrôle pour y être détruits :
//! le callback ne libère jamais de mémoire.

use dawsend_core::layout::SLOT_COUNT;
use rtrb::{Consumer as RxQueue, Producer as TxQueue, RingBuffer};

use super::source::{SourceKey, SourcePlayer};

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
}

/// Partie « thread de contrôle ».
pub struct MixerControl {
    commands: TxQueue<Command>,
    trash: RxQueue<Box<SourcePlayer>>,
}

pub fn mixer() -> (Mixer, MixerControl) {
    let (cmd_tx, cmd_rx) = RingBuffer::new(64);
    let (trash_tx, trash_rx) = RingBuffer::new(4 * SLOT_COUNT);
    (
        Mixer {
            players: Vec::with_capacity(SLOT_COUNT),
            commands: cmd_rx,
            trash: trash_tx,
        },
        MixerControl {
            commands: cmd_tx,
            trash: trash_rx,
        },
    )
}

impl MixerControl {
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
        // Protection minimale contre la saturation (le vrai limiteur arrive à l'étape 3).
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}
