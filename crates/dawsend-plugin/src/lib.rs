//! Plugin Dawsend : insert pass-through qui copie le son vers l'appli Dawsend
//! à travers la mémoire partagée.
//!
//! Règles du thread audio (`process`) : aucune allocation, aucun verrou, aucun appel
//! système bloquant. Toute l'ouverture de la mémoire partagée et la réservation du slot
//! se font dans `initialize()`. Si quoi que ce soit échoue, le plugin reste un simple
//! pass-through : il ne doit jamais bloquer ni faire planter le DAW.

use std::sync::{Arc, Mutex, RwLock, Weak};

use dawsend_core::{OpenError, Producer, Region, SlotClaim};
use nih_plug::prelude::*;

/// Zone partagée commune à toutes les instances du plugin dans ce processus.
static REGION: Mutex<Weak<Region>> = Mutex::new(Weak::new());

fn shared_region() -> Result<Arc<Region>, OpenError> {
    let mut guard = REGION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = guard.upgrade() {
        return Ok(r);
    }
    let r = Region::open()?;
    *guard = Arc::downgrade(&r);
    Ok(r)
}

pub struct Dawsend {
    params: Arc<DawsendParams>,
    claim: Option<SlotClaim>,
    /// Utilisé uniquement dans `process`.
    producer: Option<Producer>,
    /// Nom actuellement publié dans la mémoire partagée.
    published_name: String,
}

#[derive(Params)]
pub struct DawsendParams {
    /// Nom de la source, sauvegardé avec le projet du DAW.
    #[persist = "source-name"]
    pub source_name: RwLock<String>,

    /// Envoi vers l'appli. Désactivé, le plugin envoie du silence (le flux reste continu).
    #[id = "send"]
    pub send: BoolParam,
}

impl Default for DawsendParams {
    fn default() -> Self {
        Self {
            source_name: RwLock::new(String::new()),
            send: BoolParam::new("Envoi", true),
        }
    }
}

impl Default for Dawsend {
    fn default() -> Self {
        Self {
            params: Arc::new(DawsendParams::default()),
            claim: None,
            producer: None,
            published_name: String::new(),
        }
    }
}

impl Dawsend {
    fn wanted_name(&self) -> String {
        self.params
            .source_name
            .read()
            .map(|n| n.trim().to_owned())
            .unwrap_or_default()
    }

    /// Publie le nom de la source ; s'il est vide, en choisit un par défaut et le
    /// sauvegarde dans les paramètres persistants.
    fn sync_name(&mut self) {
        let Some(claim) = &self.claim else { return };
        let mut name = self.wanted_name();
        if name.is_empty() {
            name = format!("Dawsend {}", claim.index() + 1);
            if let Ok(mut n) = self.params.source_name.write() {
                *n = name.clone();
            }
        }
        if name != self.published_name {
            claim.set_name(&name);
            self.published_name = name;
        }
    }
}

impl Plugin for Dawsend {
    const NAME: &'static str = "Dawsend";
    const VENDOR: &'static str = "Metaloul";
    const URL: &'static str = "https://github.com/metaloul/dawsend";
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

        if let Some(claim) = &self.claim {
            if let Err(e) = claim.reconfigure(sample_rate, channels) {
                nih_warn!("Dawsend : reconfiguration impossible ({e}), envoi désactivé");
                self.claim = None;
            }
        } else {
            match shared_region() {
                Ok(region) => {
                    let name = self.wanted_name();
                    match SlotClaim::claim(region, sample_rate, channels, &name) {
                        Ok(claim) => {
                            self.published_name = name;
                            self.claim = Some(claim);
                        }
                        Err(e) => nih_warn!("Dawsend : {e}, envoi désactivé"),
                    }
                }
                Err(e) => nih_warn!("Dawsend : {e}, envoi désactivé"),
            }
        }

        self.sync_name();
        self.producer = self.claim.as_ref().map(SlotClaim::producer);
        // Toujours `true` : un problème côté Dawsend ne doit jamais empêcher le DAW
        // d'utiliser la piste. Le plugin reste alors un simple pass-through.
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Some(producer) = &self.producer {
            if self.params.send.value() {
                producer.write_planar(buffer.as_slice_immutable());
            } else {
                producer.write_silence(buffer.samples());
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
