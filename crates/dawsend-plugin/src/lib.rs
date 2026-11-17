//! Plugin Dawsend : insert pass-through qui copie le son vers l'appli Dawsend
//! à travers la mémoire partagée.
//!
//! Règles du thread audio (`process`) : aucune allocation, aucun verrou, aucun appel
//! système bloquant. Toute l'ouverture de la mémoire partagée et la réservation du slot
//! se font dans `initialize()`. Si quoi que ce soit échoue, le plugin reste un simple
//! pass-through : il ne doit jamais bloquer ni faire planter le DAW.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};

use dawsend_core::{LinkStatus, OpenError, Producer, Region, SlotClaim};
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

mod editor;
mod i18n;

/// Zone partagée commune à toutes les instances du plugin dans ce processus.
static REGION: Mutex<Weak<Region>> = Mutex::new(Weak::new());

fn shared_region() -> Result<Arc<Region>, OpenError> {
    let mut guard = REGION.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(r) = guard.upgrade() {
        return Ok(r);
    }
    let r = Region::open()?;
    *guard = Arc::downgrade(&r);
    Ok(r)
}

/// État partagé entre le plugin et son interface.
///
/// Le thread audio n'y touche que par des opérations atomiques (`peak`, `processing`) ;
/// les `Mutex` ne sont pris que par `initialize()`, l'interface et `Drop`.
pub struct Shared {
    claim: Mutex<Option<SlotClaim>>,
    /// Raison pour laquelle l'envoi est impossible (mode pass-through seul).
    error: Mutex<Option<String>>,
    /// Nom publié dans la mémoire partagée.
    published_name: Mutex<String>,
    /// Crête de chaque canal depuis la dernière lecture par l'interface (bits d'un `f32`).
    peak: [AtomicU32; 2],
    /// `true` entre `initialize()` et `deactivate()`.
    processing: AtomicBool,
}

/// Ce que l'interface affiche comme état de la liaison.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkView {
    /// Le moteur audio du DAW n'a pas (encore) activé le plugin.
    NotActive,
    Unavailable(String),
    Link(LinkStatus),
}

impl Shared {
    fn new() -> Self {
        Self {
            claim: Mutex::new(None),
            error: Mutex::new(None),
            published_name: Mutex::new(String::new()),
            peak: [AtomicU32::new(0), AtomicU32::new(0)],
            processing: AtomicBool::new(false),
        }
    }

    fn lock_claim(&self) -> std::sync::MutexGuard<'_, Option<SlotClaim>> {
        self.claim.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_error(&self, e: Option<String>) {
        *self.error.lock().unwrap_or_else(PoisonError::into_inner) = e;
    }

    /// État de la liaison (pour l'interface ; hors thread audio).
    pub fn link(&self) -> LinkView {
        if let Some(e) = self
            .error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            return LinkView::Unavailable(e);
        }
        if !self.processing.load(Ordering::Relaxed) {
            return LinkView::NotActive;
        }
        match self.lock_claim().as_ref() {
            Some(c) => LinkView::Link(c.status()),
            None => LinkView::NotActive,
        }
    }

    /// `true` si l'appli a coupé (mute) cette source.
    pub fn muted_in_app(&self) -> bool {
        self.lock_claim()
            .as_ref()
            .is_some_and(SlotClaim::muted_by_app)
    }

    /// Publie `name` si différent du nom actuel.
    fn publish_name(&self, name: &str) {
        let mut published = self
            .published_name
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *published != name
            && let Some(c) = self.lock_claim().as_ref()
        {
            c.set_name(name);
            *published = name.to_owned();
        }
    }

    /// Crêtes depuis le dernier appel, puis remise à zéro.
    pub fn take_peaks(&self) -> [f32; 2] {
        [0, 1].map(|c| f32::from_bits(self.peak[c].swap(0, Ordering::Relaxed)))
    }

    fn slot_index(&self) -> Option<usize> {
        self.lock_claim().as_ref().map(SlotClaim::index)
    }
}

pub struct Dawsend {
    params: Arc<DawsendParams>,
    shared: Arc<Shared>,
    /// Utilisé uniquement dans `process`.
    producer: Option<Producer>,
}

#[derive(Params)]
pub struct DawsendParams {
    /// Nom de la source, sauvegardé avec le projet du DAW.
    #[persist = "source-name"]
    pub source_name: RwLock<String>,

    /// Taille de la fenêtre de l'interface.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    /// Envoi vers l'appli. Désactivé, le plugin envoie du silence (le flux reste continu).
    #[id = "send"]
    pub send: BoolParam,
}

impl Default for DawsendParams {
    fn default() -> Self {
        Self {
            source_name: RwLock::new(String::new()),
            editor_state: editor::default_state(),
            send: BoolParam::new("Envoi", true),
        }
    }
}

impl DawsendParams {
    fn wanted_name(&self) -> String {
        self.source_name
            .read()
            .map(|n| n.trim().to_owned())
            .unwrap_or_default()
    }

    /// Renomme la source (depuis l'interface) et publie le nouveau nom.
    fn rename(&self, shared: &Shared, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Ok(mut n) = self.source_name.write() {
            *n = name.to_owned();
        }
        shared.publish_name(name);
    }
}

impl Default for Dawsend {
    fn default() -> Self {
        Self {
            params: Arc::new(DawsendParams::default()),
            shared: Arc::new(Shared::new()),
            producer: None,
        }
    }
}

impl Dawsend {
    /// Publie le nom de la source ; s'il est vide, en choisit un par défaut et le
    /// sauvegarde dans les paramètres persistants.
    fn sync_name(&self) {
        let Some(index) = self.shared.slot_index() else {
            return;
        };
        let mut name = self.params.wanted_name();
        if name.is_empty() {
            name = format!("Dawsend {}", index + 1);
            if let Ok(mut n) = self.params.source_name.write() {
                *n = name.clone();
            }
        }
        self.shared.publish_name(&name);
    }
}

impl Plugin for Dawsend {
    const NAME: &'static str = "Dawsend";
    const VENDOR: &'static str = "Metaloul";
    const URL: &'static str = env!("CARGO_PKG_REPOSITORY");
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.shared.clone())
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let channels = audio_io_layout
            .main_input_channels
            .map_or(2, NonZeroU32::get);
        let sample_rate = buffer_config.sample_rate.round() as u32;
        self.producer = None;

        {
            let mut claim = self.shared.lock_claim();
            if let Some(c) = claim.as_ref() {
                if let Err(e) = c.reconfigure(sample_rate, channels) {
                    nih_warn!("Dawsend : reconfiguration impossible ({e}), envoi désactivé");
                    self.shared.set_error(Some(e.to_string()));
                    *claim = None;
                }
            } else {
                match shared_region() {
                    Ok(region) => {
                        let name = self.params.wanted_name();
                        match SlotClaim::claim(region, sample_rate, channels, &name) {
                            Ok(c) => {
                                *self
                                    .shared
                                    .published_name
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner) = name;
                                *claim = Some(c);
                                self.shared.set_error(None);
                            }
                            Err(e) => {
                                nih_warn!("Dawsend : {e}, envoi désactivé");
                                self.shared.set_error(Some(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        nih_warn!("Dawsend : {e}, envoi désactivé");
                        self.shared.set_error(Some(e.to_string()));
                    }
                }
            }
            self.producer = claim.as_ref().map(SlotClaim::producer);
        }

        self.sync_name();
        self.shared.processing.store(true, Ordering::Relaxed);
        // Toujours `true` : un problème côté Dawsend ne doit jamais empêcher le DAW
        // d'utiliser la piste. Le plugin reste alors un simple pass-through.
        true
    }

    fn deactivate(&mut self) {
        self.shared.processing.store(false, Ordering::Relaxed);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let send = self.params.send.value();
        if let Some(producer) = &self.producer {
            if send {
                producer.write_planar(buffer.as_slice_immutable());
            } else {
                producer.write_silence(buffer.samples());
            }
        }
        // Vumètre de l'interface : crête de chaque canal (opérations atomiques seulement).
        if send {
            let chans = buffer.as_slice_immutable();
            for (c, data) in chans.iter().take(2).enumerate() {
                let p = data.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                let slot = &self.shared.peak[c];
                if p > f32::from_bits(slot.load(Ordering::Relaxed)) {
                    slot.store(p.to_bits(), Ordering::Relaxed);
                }
            }
            if chans.len() == 1 {
                let v = self.shared.peak[0].load(Ordering::Relaxed);
                self.shared.peak[1].store(v, Ordering::Relaxed);
            }
        }
        // Le son n'est pas modifié. KeepAlive : le DAW ne doit pas suspendre le plugin
        // sur du silence, sinon l'appli verrait la source en pause.
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Dawsend {
    const CLAP_ID: &'static str = "com.metaloul.dawsend";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Envoie le son de la piste vers l'appli Dawsend (sortie Windows normale)");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for Dawsend {
    const VST3_CLASS_ID: [u8; 16] = *b"DawsendMetaloul1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Dawsend);
nih_export_vst3!(Dawsend);
