//! Appli Dawsend : joue le son des plugins Dawsend sur une sortie Windows normale.
//! Vit surtout dans la zone de notification ; la fenêtre sert aux réglages.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod portable;
mod settings;
mod tray;
mod updates;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use dawsend_app::output;
use dawsend_app::service::{Engine, EngineSettings, EngineView, Meters, SourceSettings};
use serde::Serialize;
use settings::{AppPrefs, SettingsFile};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tauri_plugin_opener::OpenerExt as _;

/// Argument passé par le démarrage automatique : rester dans la zone de notification.
const MINIMIZED_ARG: &str = "--minimized";

pub struct AppState {
    engine: Mutex<Option<Engine>>,
    engine_error: Option<String>,
    prefs: Mutex<AppPrefs>,
    /// Réglages modifiés depuis la dernière sauvegarde.
    dirty: AtomicBool,
    settings_path: PathBuf,
    log_dir: PathBuf,
    /// Version portable (`portable.txt` à côté de l'exécutable).
    portable: bool,
    updates: updates::Updates,
}

impl AppState {
    fn with_engine<R>(&self, f: impl FnOnce(&Engine) -> R) -> Result<R, String> {
        let guard = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        match guard.as_ref() {
            Some(e) => Ok(f(e)),
            None => Err(self
                .engine_error
                .clone()
                .unwrap_or_else(|| "moteur audio arrêté".into())),
        }
    }

    pub fn prefs(&self) -> AppPrefs {
        self.prefs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn engine_settings(&self) -> Option<EngineSettings> {
        self.with_engine(Engine::settings).ok()
    }

    pub fn engine_view(&self) -> Option<EngineView> {
        self.with_engine(Engine::view).ok()
    }

    /// Modifie les réglages du moteur et programme la sauvegarde.
    pub fn update_engine(&self, f: impl FnOnce(&mut EngineSettings)) -> Result<(), String> {
        self.with_engine(|e| e.update(f))?;
        self.dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn snapshot(&self) -> SettingsFile {
        SettingsFile {
            engine: self.engine_settings().unwrap_or_default(),
            app: self.prefs(),
        }
    }

    fn save_now(&self) {
        let s = self.snapshot();
        match settings::save(&self.settings_path, &s) {
            Ok(()) => log::info!(
                "réglages enregistrés (volume général {:.2}) dans {}",
                s.engine.master_gain,
                self.settings_path.display()
            ),
            Err(e) => log::error!(
                "enregistrement des réglages impossible ({}) : {e}",
                self.settings_path.display()
            ),
        }
    }
}

#[derive(Serialize)]
struct StateDto {
    version: &'static str,
    view: Option<EngineView>,
    settings: Option<EngineSettings>,
    prefs: AppPrefs,
    language: &'static str,
    engine_error: Option<String>,
    update: updates::UpdateStatus,
}

#[derive(Serialize)]
struct DeviceDto {
    name: String,
    is_default: bool,
}

#[tauri::command]
async fn get_state(state: State<'_, Arc<AppState>>) -> Result<StateDto, String> {
    let prefs = state.prefs();
    Ok(StateDto {
        version: env!("CARGO_PKG_VERSION"),
        view: state.engine_view(),
        settings: state.engine_settings(),
        language: i18n::resolve(&prefs.language),
        prefs,
        engine_error: state.engine_error.clone(),
        update: state.updates.status(),
    })
}

#[tauri::command]
async fn update_check(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<updates::UpdateStatus, String> {
    log::info!("mises à jour : vérification demandée");
    updates::check(&app, &state).await
}

#[tauri::command]
async fn update_install(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.portable {
        // Version portable : pas d'installateur, on ouvre la page de téléchargement.
        let url = portable::RELEASES_URL;
        log::info!("mises à jour : version portable, ouverture de {url}");
        return tauri::async_runtime::spawn_blocking(move || {
            app.opener().open_url(url, None::<&str>).map_err(|e| {
                log::warn!("ouverture de {url} impossible : {e}");
                e.to_string()
            })
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    log::info!("mises à jour : installation demandée");
    updates::install(state.inner().clone()).await
}

/// Crêtes pour les vumètres, lues à chaque image par l'interface : lecture d'atomiques
/// sous un verrou bref, rien d'autre.
#[tauri::command]
async fn get_meters(state: State<'_, Arc<AppState>>) -> Result<Meters, String> {
    state.with_engine(Engine::meters)
}

#[tauri::command]
async fn list_devices() -> Result<Vec<DeviceDto>, String> {
    let list = tauri::async_runtime::spawn_blocking(output::list_devices)
        .await
        .map_err(|e| e.to_string())??;
    Ok(list
        .into_iter()
        .map(|(name, is_default)| DeviceDto { name, is_default })
        .collect())
}

#[tauri::command]
async fn set_device(state: State<'_, Arc<AppState>>, device: Option<String>) -> Result<(), String> {
    log::info!("réglage : sortie {device:?}");
    state.update_engine(|s| s.device = device)
}

#[tauri::command]
async fn set_latency(state: State<'_, Arc<AppState>>, latency_ms: f64) -> Result<(), String> {
    if !latency_ms.is_finite() {
        return Err("valeur invalide".into());
    }
    let ms = latency_ms.clamp(
        EngineSettings::MIN_LATENCY_MS,
        EngineSettings::MAX_LATENCY_MS,
    );
    log::info!("réglage : marge {ms} ms");
    state.update_engine(|s| s.latency_ms = ms)
}

#[tauri::command]
async fn set_master(state: State<'_, Arc<AppState>>, gain: f32, muted: bool) -> Result<(), String> {
    // Le bouton rotatif envoie jusqu'à une valeur par image pendant un geste : le volume
    // seul part en `debug`, une coupure en `info`.
    let mut toggled = false;
    state.update_engine(|s| {
        toggled = s.master_muted != muted;
        s.master_gain = gain.clamp(0.0, 4.0);
        s.master_muted = muted;
    })?;
    if toggled {
        log::info!("réglage : volume général {gain:.2}, coupé {muted}");
    } else {
        log::debug!("réglage : volume général {gain:.2}");
    }
    Ok(())
}

#[tauri::command]
async fn set_source(
    state: State<'_, Arc<AppState>>,
    name: String,
    settings: SourceSettings,
) -> Result<(), String> {
    let line = format!(
        "réglage : source « {name} » active {}, volume {:.2}, coupée {}",
        settings.enabled, settings.gain, settings.muted
    );
    // Même règle que le volume général : un changement de volume seul part en `debug`.
    let mut toggled = true;
    state.update_engine(|s| {
        if let Some(prev) = s.sources.get(&name) {
            toggled = prev.enabled != settings.enabled || prev.muted != settings.muted;
        }
        s.sources.insert(
            name,
            SourceSettings {
                gain: settings.gain.clamp(0.0, 4.0),
                ..settings
            },
        );
    })?;
    if toggled {
        log::info!("{line}");
    } else {
        log::debug!("{line}");
    }
    Ok(())
}

#[tauri::command]
async fn set_prefs(state: State<'_, Arc<AppState>>, prefs: AppPrefs) -> Result<(), String> {
    log::info!(
        "réglage : langue {}, fenêtre au démarrage {}, journal détaillé {}, mises à jour auto {}",
        prefs.language,
        prefs.open_window_on_start,
        prefs.verbose_log,
        prefs.check_updates
    );
    apply_log_level(prefs.verbose_log);
    *state.prefs.lock().unwrap_or_else(PoisonError::into_inner) = prefs;
    state.dirty.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    // Lecture du registre : hors du fil des commandes.
    tauri::async_runtime::spawn_blocking(move || app.autolaunch().is_enabled())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_autostart_inner(&app, enabled))
        .await
        .map_err(|e| e.to_string())?
}

pub fn set_autostart_inner(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    match &r {
        Ok(()) => log::info!("réglage : démarrage avec Windows {enabled}"),
        Err(e) => log::error!("démarrage avec Windows ({enabled}) impossible : {e}"),
    }
    r.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_logs(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let dir = state.log_dir.clone();
    log::info!("ouverture du dossier des journaux {}", dir.display());
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("dossier des journaux {} : {e}", dir.display());
        }
        app.opener()
            .open_path(dir.to_string_lossy(), None::<&str>)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ouvre la page du projet sur GitHub. L'adresse vient de `Cargo.toml` : l'interface ne
/// peut pas faire ouvrir une autre adresse.
#[tauri::command]
async fn open_repo(app: AppHandle) -> Result<(), String> {
    let url = env!("CARGO_PKG_REPOSITORY");
    log::info!("ouverture de la page du projet {url}");
    tauri::async_runtime::spawn_blocking(move || {
        app.opener().open_url(url, None::<&str>).map_err(|e| {
            log::warn!("ouverture de {url} impossible : {e}");
            e.to_string()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn apply_log_level(verbose: bool) {
    log::set_max_level(if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    });
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Arrête proprement : le moteur annonce son départ aux plugins, puis réglages enregistrés.
pub fn quit(app: &AppHandle) {
    log::info!("arrêt demandé");
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.save_now();
        let engine = state
            .engine
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(engine);
    }
    app.exit(0);
}

fn main() {
    // Version portable : réglages et journaux à côté de l'exécutable.
    let portable_dir = portable::data_dir();
    let log_target = match &portable_dir {
        Some(dir) => TargetKind::Folder {
            path: dir.join("logs"),
            file_name: Some("dawsend".into()),
        },
        None => TargetKind::LogDir {
            file_name: Some("dawsend".into()),
        },
    };
    let log_plugin = tauri_plugin_log::Builder::new()
        .targets([Target::new(TargetKind::Stdout), Target::new(log_target)])
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .max_file_size(2_000_000)
        .level(log::LevelFilter::Debug)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .level_for("tauri", log::LevelFilter::Info)
        .build();

    let result = tauri::Builder::default()
        // En premier : une seconde instance réveille la première puis s'arrête.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            log::info!("seconde instance lancée ({args:?}) : fenêtre affichée");
            if !args.iter().any(|a| a == MINIMIZED_ARG) {
                show_main(app);
            }
        }))
        .plugin(log_plugin)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Même nom de valeur « Run » que celle écrite par l'installateur.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Dawsend")
                .arg(MINIMIZED_ARG)
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle().clone();
            let (config_dir, log_dir) = match &portable_dir {
                Some(dir) => (dir.clone(), dir.join("logs")),
                None => (app.path().app_config_dir()?, app.path().app_log_dir()?),
            };
            let settings_path = settings::path_in(&config_dir);
            let loaded = settings::load(&settings_path);
            apply_log_level(loaded.app.verbose_log);
            log::info!(
                "Dawsend {} démarré (réglages : {}, journaux : {})",
                env!("CARGO_PKG_VERSION"),
                settings_path.display(),
                log_dir.display()
            );
            if portable_dir.is_some() {
                log::info!(
                    "version portable ({} trouvé à côté de l'exécutable)",
                    portable::MARKER
                );
            }

            let (engine, engine_error) = match Engine::start(loaded.engine.clone()) {
                Ok(e) => (Some(e), None),
                Err(e) => {
                    log::error!("moteur audio impossible à démarrer : {e}");
                    (None, Some(e))
                }
            };
            let state = Arc::new(AppState {
                engine: Mutex::new(engine),
                engine_error,
                prefs: Mutex::new(loaded.app.clone()),
                dirty: AtomicBool::new(false),
                settings_path,
                log_dir,
                portable: portable_dir.is_some(),
                updates: updates::Updates::new(portable_dir.is_some()),
            });
            app.manage(state.clone());

            // Sauvegarde différée : au plus deux fois par seconde.
            let saver = state.clone();
            std::thread::Builder::new()
                .name("dawsend-settings".into())
                .spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_millis(500));
                        if saver.dirty.swap(false, Ordering::Relaxed) {
                            saver.save_now();
                        }
                    }
                })?;

            updates::start(handle.clone(), state.clone());
            tray::install(&handle, state)?;

            let minimized = std::env::args().any(|a| a == MINIMIZED_ARG);
            if minimized {
                log::info!("démarré dans la zone de notification ({MINIMIZED_ARG})");
            } else if loaded.app.open_window_on_start {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre la cache : l'appli continue dans la zone de notification.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                log::debug!("fenêtre cachée (l'appli reste dans la zone de notification)");
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_meters,
            list_devices,
            set_device,
            set_latency,
            set_master,
            set_source,
            set_prefs,
            get_autostart,
            set_autostart,
            open_logs,
            open_repo,
            update_check,
            update_install,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("l'appli s'est arrêtée sur une erreur : {e}");
        eprintln!("Dawsend : {e}");
        std::process::exit(1);
    }
}
