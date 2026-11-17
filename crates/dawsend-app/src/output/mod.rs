//! Sortie audio, isolée par plateforme.
//!
//! - Windows : cpal (WASAPI en mode partagé).
//! - macOS : cpal (CoreAudio) — étape 6.
//! - Linux : cpal (ALSA, donc PipeWire via pipewire-alsa) en attendant le backend
//!   PipeWire natif de l'étape 6.

mod cpal_backend;

pub use cpal_backend::{OutputInfo, OutputStream, default_device_name, list_devices, start};

/// Ce que le backend appelle à chaque période : remplir `out` en stéréo entrelacée.
pub trait Renderer: Send + 'static {
    fn render(&mut self, out: &mut [f32], frames: usize);
}

impl Renderer for crate::engine::mixer::Mixer {
    fn render(&mut self, out: &mut [f32], frames: usize) {
        crate::engine::mixer::Mixer::render(self, out, frames);
    }
}
