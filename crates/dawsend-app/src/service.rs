//! Service de l'appli : un thread de contrôle (10 Hz) qui découvre les sources publiées
//! par les plugins, ouvre la sortie audio, crée et retire les lecteurs, applique les
//! réglages et publie un état lisible par l'interface (console ou fenêtre).
//!
//! Le callback audio ne voit que le [`Mixer`](crate::engine::mixer::Mixer) ; tout ce qui
//! alloue ou bloque se fait ici.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dawsend_core::layout::{CONSUMER_FLAG_LISTENING, CONSUMER_FLAG_MUTED};
use dawsend_core::registry::{self, SlotMeta};
use dawsend_core::{Consumer, Region, clock};
use serde::{Deserialize, Serialize};

use crate::engine::mixer::{Command, MixerControl, MixerShared, mixer};
use crate::engine::source::{SourceKey, SourcePlayer, SourceStats};
use crate::output::{self, OutputInfo, OutputStream};

/// Période du thread de contrôle.
const TICK: Duration = Duration::from_millis(100);
/// Sans source à jouer depuis ce délai, la sortie est fermée (CPU quasi nul au repos).
const IDLE_CLOSE: Duration = Duration::from_secs(5);
/// Délai entre deux tentatives d'ouverture de la sortie.
const RETRY: Duration = Duration::from_secs(2);
/// Une source est « active » si le plugin a écrit depuis moins que ça.
const ACTIVE_NS: u64 = 250 * clock::MS;

/// Réglages d'une source, associés à son nom (l'identifiant d'instance change à chaque
/// session du DAW).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSettings {
    pub enabled: bool,
    pub gain: f32,
    pub muted: bool,
}

impl Default for SourceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gain: 1.0,
            muted: false,
        }
    }
}

/// Réglages du moteur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    /// Périphérique de sortie (nom exact ou partie du nom) ; `None` : celui du système.
    pub device: Option<String>,
    /// Marge visée dans le buffer, en ms.
    pub latency_ms: f64,
    pub master_gain: f32,
    pub master_muted: bool,
    pub sources: BTreeMap<String, SourceSettings>,
    /// Ne jouer que cette source (option `--source` de la console). Non sauvegardé.
    #[serde(skip)]
    pub solo: Option<String>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            device: None,
            latency_ms: 10.0,
            master_gain: 1.0,
            master_muted: false,
            sources: BTreeMap::new(),
            solo: None,
        }
    }
}

impl EngineSettings {
    pub const MIN_LATENCY_MS: f64 = 3.0;
    pub const MAX_LATENCY_MS: f64 = 200.0;

    pub fn source(&self, name: &str) -> SourceSettings {
        self.sources.get(name).copied().unwrap_or_default()
    }

    fn wants(&self, name: &str) -> bool {
        match &self.solo {
            Some(s) => s.eq_ignore_ascii_case(name),
            None => self.source(name).enabled,
        }
    }

    fn latency(&self) -> f64 {
        if self.latency_ms.is_finite() {
            self.latency_ms
                .clamp(Self::MIN_LATENCY_MS, Self::MAX_LATENCY_MS)
        } else {
            10.0
        }
    }
}

/// Vue d'une source, pour l'interface.
#[derive(Debug, Clone, Serialize)]
pub struct SourceView {
    pub slot: usize,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u32,
    /// Le plugin envoie du son en ce moment (sinon : DAW arrêté ou piste inactive).
    pub active: bool,
    pub enabled: bool,
    pub gain: f32,
    pub muted: bool,
    /// La source est jouée sur la sortie.
    pub playing: bool,
    pub fill_ms: f64,
    pub target_ms: f64,
    pub correction_ppm: f32,
    pub underruns: u64,
    pub resyncs: u64,
    pub anti_alias: bool,
}

/// État publié par le service.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EngineView {
    pub output: Option<OutputInfo>,
    pub output_error: Option<String>,
    /// Sortie fermée faute de source à jouer.
    pub idle: bool,
    pub sources: Vec<SourceView>,
    /// Délai propre au rééchantillonneur, en ms (pour la latence estimée).
    pub resampler_delay_ms: f64,
}

/// Crêtes d'une source depuis la lecture précédente.
#[derive(Debug, Clone, Serialize)]
pub struct SourceMeter {
    pub slot: usize,
    pub peak: [f32; 2],
}

/// Crêtes lues par l'interface à chaque image (environ 60 fois par seconde), à part de
/// la vue publiée toutes les 100 ms : les vumètres restent fluides.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Meters {
    pub master: [f32; 2],
    pub sources: Vec<SourceMeter>,
}

/// Compteurs de crêtes des sources jouées, tenus à jour par le thread de contrôle.
#[derive(Default)]
struct MeterTaps {
    master: Option<Arc<MixerShared>>,
    sources: Vec<(usize, Arc<SourceStats>)>,
}

enum Cmd {
    Settings(Box<EngineSettings>),
    Shutdown,
}

/// Service en cours. S'arrête au `Drop`.
pub struct Engine {
    tx: Sender<Cmd>,
    view: Arc<Mutex<EngineView>>,
    taps: Arc<Mutex<MeterTaps>>,
    settings: Mutex<EngineSettings>,
    handle: Option<JoinHandle<()>>,
}

impl Engine {
    /// Ouvre la mémoire partagée et démarre le thread de contrôle.
    pub fn start(settings: EngineSettings) -> Result<Self, String> {
        let region = Region::open().map_err(|e| e.to_string())?;
        region.app_announce();
        log::info!(
            "moteur démarré : mémoire partagée {}, marge {} ms",
            region.name(),
            settings.latency()
        );
        let (tx, rx) = mpsc::channel();
        let view = Arc::new(Mutex::new(EngineView::default()));
        let taps = Arc::new(Mutex::new(MeterTaps::default()));
        let mut ctl = Control::new(region, settings.clone(), view.clone(), taps.clone());
        let handle = std::thread::Builder::new()
            .name("dawsend-control".into())
            .spawn(move || {
                loop {
                    match rx.recv_timeout(TICK) {
                        Ok(Cmd::Settings(s)) => ctl.apply(*s),
                        Ok(Cmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                    ctl.tick();
                }
                ctl.shutdown();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx,
            view,
            taps,
            settings: Mutex::new(settings),
            handle: Some(handle),
        })
    }

    pub fn view(&self) -> EngineView {
        self.view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Crêtes depuis la lecture précédente (remises à zéro par la lecture).
    pub fn meters(&self) -> Meters {
        let taps = self.taps.lock().unwrap_or_else(PoisonError::into_inner);
        Meters {
            master: taps.master.as_ref().map_or([0.0; 2], |s| s.take_peaks()),
            sources: taps
                .sources
                .iter()
                .map(|(slot, st)| SourceMeter {
                    slot: *slot,
                    peak: st.take_peaks(),
                })
                .collect(),
        }
    }

    pub fn settings(&self) -> EngineSettings {
        self.settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Modifie les réglages ; renvoie les nouveaux.
    pub fn update(&self, f: impl FnOnce(&mut EngineSettings)) -> EngineSettings {
        let mut s = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut s);
        let _ = self.tx.send(Cmd::Settings(Box::new(s.clone())));
        s.clone()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Player {
    name: String,
    stats: Arc<SourceStats>,
    anti_alias: bool,
    last_counts: (u64, u64),
}

struct Running {
    stream: OutputStream,
    info: OutputInfo,
    control: MixerControl,
    players: HashMap<SourceKey, Player>,
    resampler_delay_ms: f64,
}

struct Control {
    region: Arc<Region>,
    settings: EngineSettings,
    view: Arc<Mutex<EngineView>>,
    taps: Arc<Mutex<MeterTaps>>,
    running: Option<Running>,
    shared: Option<Arc<MixerShared>>,
    output_error: Option<String>,
    next_open: Instant,
    last_wanted: Instant,
    /// Périphérique par défaut au moment de l'ouverture (pour suivre ses changements).
    opened_default: Option<String>,
    next_default_check: Instant,
    known: HashMap<SourceKey, String>,
}

impl Control {
    fn new(
        region: Arc<Region>,
        settings: EngineSettings,
        view: Arc<Mutex<EngineView>>,
        taps: Arc<Mutex<MeterTaps>>,
    ) -> Self {
        let now = Instant::now();
        Self {
            region,
            settings,
            view,
            taps,
            running: None,
            shared: None,
            output_error: None,
            next_open: now,
            last_wanted: now,
            opened_default: None,
            next_default_check: now,
            known: HashMap::new(),
        }
    }

    fn apply(&mut self, s: EngineSettings) {
        let reopen = s.device != self.settings.device
            || (s.latency() - self.settings.latency()).abs() > f64::EPSILON;
        self.settings = s;
        if reopen && self.running.is_some() {
            log::info!(
                "réglages de sortie changés (périphérique {:?}, marge {} ms) : réouverture",
                self.settings.device,
                self.settings.latency()
            );
            self.close();
            self.next_open = Instant::now();
        }
    }

    fn close(&mut self) {
        if let Some(r) = self.running.take() {
            for key in r.players.keys() {
                registry::mark_consumer(&self.region, key.index, 0);
            }
        }
        self.shared = None;
    }

    fn open(&mut self) {
        let (mix, control) = mixer();
        let wanted = self.settings.device.clone();
        match output::start(wanted.as_deref(), mix) {
            Ok(ok) => self.finish_open(Ok(ok), control, None),
            Err(err) if wanted.is_some() => {
                // Périphérique choisi débranché ou renommé : repli sur celui du système.
                log::warn!(
                    "sortie {wanted:?} indisponible ({err}), repli sur la sortie par défaut"
                );
                let (mix, control) = mixer();
                let result = output::start(None, mix);
                self.finish_open(result, control, Some(err));
            }
            Err(err) => self.finish_open(Err(err), control, None),
        }
    }

    fn finish_open(
        &mut self,
        result: Result<(OutputStream, OutputInfo), String>,
        control: MixerControl,
        note: Option<String>,
    ) {
        match result {
            Ok((stream, info)) => {
                let shared = control.shared().clone();
                shared.set_master_gain(self.settings.master_gain);
                shared
                    .muted
                    .store(self.settings.master_muted, Ordering::Relaxed);
                self.shared = Some(shared);
                self.opened_default = if self.settings.device.is_none() || note.is_some() {
                    output::default_device_name()
                } else {
                    None
                };
                self.output_error = note.map(|e| format!("périphérique choisi indisponible : {e}"));
                self.running = Some(Running {
                    stream,
                    info,
                    control,
                    players: HashMap::new(),
                    resampler_delay_ms: 0.0,
                });
            }
            Err(e) => {
                if self.output_error.as_deref() != Some(e.as_str()) {
                    log::error!("ouverture de la sortie impossible : {e}");
                }
                self.output_error = Some(e);
                self.next_open = Instant::now() + RETRY;
            }
        }
    }

    fn tick(&mut self) {
        let now_ns = clock::now_ns();
        let has_players = self.running.as_ref().is_some_and(|r| !r.players.is_empty());
        self.region.app_heartbeat(has_players);
        let reaped = registry::reap_dead(&self.region);
        if reaped > 0 {
            log::info!("{reaped} source(s) libérée(s) : processus du DAW disparu");
        }
        let sources = registry::active_sources(&self.region);
        self.log_source_changes(&sources);

        let wanted: Vec<&SlotMeta> = sources
            .iter()
            .filter(|m| self.settings.wants(&m.name))
            .collect();
        let now = Instant::now();
        if !wanted.is_empty() {
            self.last_wanted = now;
        }

        // Sortie : erreur, changement du périphérique par défaut, veille.
        if let Some(r) = &self.running
            && r.stream.failed()
        {
            log::warn!("sortie « {} » perdue, réouverture", r.info.device_name);
            self.close();
            self.next_open = now;
        }
        if self.running.is_some() && self.opened_default.is_some() && now >= self.next_default_check
        {
            self.next_default_check = now + RETRY;
            let current = output::default_device_name();
            if current != self.opened_default {
                log::info!(
                    "périphérique par défaut changé ({:?} → {:?}), réouverture",
                    self.opened_default,
                    current
                );
                self.close();
                self.next_open = now;
            }
        }
        if self.running.is_some() && wanted.is_empty() && now - self.last_wanted >= IDLE_CLOSE {
            log::info!(
                "aucune source à jouer depuis {} s : sortie en veille",
                IDLE_CLOSE.as_secs()
            );
            self.close();
        }
        if self.running.is_none() && !wanted.is_empty() && now >= self.next_open {
            self.open();
        }

        if let Some(r) = &mut self.running {
            Self::sync_players(r, &self.region, &self.settings, &wanted);
        }
        self.publish(&sources, now_ns);
    }

    fn sync_players(
        r: &mut Running,
        region: &Arc<Region>,
        settings: &EngineSettings,
        wanted: &[&SlotMeta],
    ) {
        for key in r.control.collect_garbage() {
            r.players.remove(&key);
        }
        let wanted_keys: Vec<SourceKey> = wanted
            .iter()
            .map(|m| SourceKey {
                index: m.index,
                generation: m.generation,
            })
            .collect();
        let stale: Vec<SourceKey> = r
            .players
            .keys()
            .filter(|k| !wanted_keys.contains(k))
            .copied()
            .collect();
        for key in stale {
            if send(&mut r.control, Command::Remove(key)) {
                if let Some(p) = r.players.remove(&key) {
                    log::info!("source « {} » : lecture arrêtée", p.name);
                }
                registry::mark_consumer(region, key.index, 0);
            }
        }
        for (m, key) in wanted.iter().zip(wanted_keys) {
            if !r.players.contains_key(&key) {
                match SourcePlayer::new(
                    Consumer::new(region.clone(), m),
                    r.info.sample_rate,
                    settings.latency(),
                ) {
                    Ok(player) => {
                        let stats = player.stats();
                        let anti_alias = player.has_anti_alias();
                        r.resampler_delay_ms =
                            r.resampler_delay_ms.max(player.resampler_delay_ms());
                        if send(&mut r.control, Command::Add(Box::new(player))) {
                            log::info!(
                                "source « {} » : lecture ({} Hz, {} canal(aux) → {} Hz{})",
                                m.name,
                                m.sample_rate,
                                m.channels,
                                r.info.sample_rate,
                                if anti_alias { ", anti-repliement" } else { "" }
                            );
                            r.players.insert(
                                key,
                                Player {
                                    name: m.name.clone(),
                                    stats,
                                    anti_alias,
                                    last_counts: (0, 0),
                                },
                            );
                        }
                    }
                    Err(e) => log::error!("source « {} » : lecteur impossible ({e})", m.name),
                }
            }
            if let Some(p) = r.players.get_mut(&key) {
                let s = settings.source(&m.name);
                p.name.clone_from(&m.name);
                p.stats.set_gain(s.gain);
                p.stats.muted.store(s.muted, Ordering::Relaxed);
                let flags = CONSUMER_FLAG_LISTENING | if s.muted { CONSUMER_FLAG_MUTED } else { 0 };
                registry::mark_consumer(region, key.index, flags);
                let counts = (
                    p.stats.underruns.load(Ordering::Relaxed),
                    p.stats.overruns.load(Ordering::Relaxed),
                );
                if counts != p.last_counts {
                    log::warn!(
                        "source « {} » : underruns {} (+{}), resynchros {} (+{}), marge {:.1} ms, correction {:.0} ppm",
                        p.name,
                        counts.0,
                        counts.0 - p.last_counts.0,
                        counts.1,
                        counts.1 - p.last_counts.1,
                        p.stats.target_frames.load(Ordering::Relaxed) as f64 * 1000.0
                            / f64::from(m.sample_rate.max(1)),
                        p.stats.correction_ppm()
                    );
                    p.last_counts = counts;
                }
            }
        }
        let shared = r.control.shared();
        shared.set_master_gain(settings.master_gain);
        shared.muted.store(settings.master_muted, Ordering::Relaxed);
    }

    fn log_source_changes(&mut self, sources: &[SlotMeta]) {
        let current: HashMap<SourceKey, String> = sources
            .iter()
            .map(|m| {
                (
                    SourceKey {
                        index: m.index,
                        generation: m.generation,
                    },
                    m.name.clone(),
                )
            })
            .collect();
        for (k, name) in &current {
            match self.known.get(k) {
                None => log::info!("source apparue : « {name} » (emplacement {})", k.index + 1),
                Some(old) if old != name => log::info!("source renommée : « {old} » → « {name} »"),
                _ => {}
            }
        }
        for (k, name) in &self.known {
            if !current.contains_key(k) {
                log::info!("source disparue : « {name} » (emplacement {})", k.index + 1);
            }
        }
        self.known = current;
    }

    fn publish(&mut self, sources: &[SlotMeta], now_ns: u64) {
        let mut v = EngineView {
            output: self.running.as_ref().map(|r| r.info.clone()),
            output_error: self.output_error.clone(),
            idle: self.running.is_none(),
            resampler_delay_ms: self.running.as_ref().map_or(0.0, |r| r.resampler_delay_ms),
            sources: Vec::with_capacity(sources.len()),
        };
        let mut taps = MeterTaps {
            master: self.running.as_ref().and(self.shared.clone()),
            sources: Vec::new(),
        };
        for m in sources {
            let key = SourceKey {
                index: m.index,
                generation: m.generation,
            };
            let age = now_ns.saturating_sub(
                self.region
                    .slot(m.index)
                    .producer_heartbeat_ns
                    .load(Ordering::Relaxed),
            );
            let s = self.settings.source(&m.name);
            let player = self.running.as_ref().and_then(|r| r.players.get(&key));
            let rate = f64::from(m.sample_rate.max(1));
            let mut sv = SourceView {
                slot: m.index,
                name: m.name.clone(),
                sample_rate: m.sample_rate,
                channels: m.channels,
                active: age < ACTIVE_NS,
                enabled: self.settings.wants(&m.name),
                gain: s.gain,
                muted: s.muted,
                playing: false,
                fill_ms: 0.0,
                target_ms: 0.0,
                correction_ppm: 0.0,
                underruns: 0,
                resyncs: 0,
                anti_alias: false,
            };
            if let Some(p) = player {
                let st = &p.stats;
                sv.playing = st.playing.load(Ordering::Relaxed) == 1;
                sv.fill_ms = st.smoothed_fill_frames.load(Ordering::Relaxed) as f64 * 1000.0 / rate;
                sv.target_ms = st.target_frames.load(Ordering::Relaxed) as f64 * 1000.0 / rate;
                sv.correction_ppm = st.correction_ppm();
                sv.underruns = st.underruns.load(Ordering::Relaxed);
                sv.resyncs = st.overruns.load(Ordering::Relaxed);
                taps.sources.push((m.index, st.clone()));
                sv.anti_alias = p.anti_alias;
            }
            v.sources.push(sv);
        }
        *self.view.lock().unwrap_or_else(PoisonError::into_inner) = v;
        *self.taps.lock().unwrap_or_else(PoisonError::into_inner) = taps;
    }

    fn shutdown(&mut self) {
        self.close();
        self.region.app_leave();
        log::info!("moteur arrêté");
    }
}

/// Envoie une commande au callback ; `false` si la sortie ne répond plus.
fn send(control: &mut MixerControl, mut cmd: Command) -> bool {
    // La file contient 64 places et le callback la vide à chaque période.
    for _ in 0..20 {
        match control.send(cmd) {
            Ok(()) => return true,
            Err(c) => {
                cmd = c;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    log::warn!("la sortie audio ne répond plus (commande ignorée)");
    false
}
