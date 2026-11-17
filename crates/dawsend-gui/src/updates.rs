//! Mises à jour : l'appli lit le `latest.json` de la dernière release publique sur GitHub
//! (adresse dans `tauri.conf.json`), vérifie la signature de l'installateur avec la clé
//! publique de Dawsend (`updater-key.pub`), puis le lance en mode silencieux.
//!
//! Vérification au démarrage (après 20 s) puis toutes les 6 heures, si l'utilisateur ne
//! l'a pas désactivée. L'appli prévient ; c'est l'utilisateur qui lance l'installation,
//! et seulement quand aucun DAW n'a le plugin chargé (Windows ne remplace pas un fichier
//! `.vst3` ouvert).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt as _};

use dawsend_app::service::Engine;

use crate::AppState;

/// Clé publique minisign (base64) produite par `tauri signer generate`. Vide : les mises à
/// jour sont désactivées dans cette version.
const PUBKEY: &str = include_str!("../updater-key.pub");
const FIRST_CHECK: Duration = Duration::from_secs(20);
const PERIOD: Duration = Duration::from_secs(6 * 3600);
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// État affiché par l'interface.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateStatus {
    /// Une clé publique est intégrée : les mises à jour sont possibles.
    pub enabled: bool,
    pub checking: bool,
    pub available: Option<UpdateInfo>,
    /// Dernière vérification réussie (secondes depuis 1970).
    pub last_check: Option<u64>,
    pub error: Option<String>,
    /// Téléchargement en cours : octets reçus et taille totale si connue.
    pub downloading: Option<(u64, Option<u64>)>,
}

#[derive(Default)]
pub struct Updates {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<Update>>,
}

fn pubkey() -> Option<&'static str> {
    let k = PUBKEY.trim();
    (!k.is_empty()).then_some(k)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Updates {
    pub fn new() -> Self {
        let u = Self::default();
        u.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .enabled = pubkey().is_some();
        u
    }

    pub fn status(&self) -> UpdateStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, f: impl FnOnce(&mut UpdateStatus)) {
        f(&mut self.status.lock().unwrap_or_else(PoisonError::into_inner));
    }
}

/// Lance la vérification périodique (tâche asynchrone de Tauri).
pub fn start(app: AppHandle, state: Arc<AppState>) {
    if pubkey().is_none() {
        log::info!(
            "mises à jour : pas de clé publique dans cette version, vérification désactivée"
        );
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio_sleep(FIRST_CHECK).await;
        loop {
            if state.prefs().check_updates {
                let _ = check(&app, &state).await;
            } else {
                log::debug!("mises à jour : vérification automatique désactivée");
            }
            tokio_sleep(PERIOD).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    // Pas de dépendance directe à tokio : une attente dans un thread bloquant suffit.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

/// Vérifie la présence d'une nouvelle version et met l'état à jour.
pub async fn check(app: &AppHandle, state: &AppState) -> Result<UpdateStatus, String> {
    let Some(key) = pubkey() else {
        return Err("mises à jour indisponibles dans cette version".into());
    };
    let up = &state.updates;
    up.set(|s| {
        s.checking = true;
        s.error = None;
    });
    log::info!(
        "mises à jour : vérification (version installée {})",
        env!("CARGO_PKG_VERSION")
    );
    let result = async {
        app.updater_builder()
            .pubkey(key)
            .timeout(TIMEOUT)
            .build()?
            .check()
            .await
    }
    .await;

    match result {
        Ok(Some(update)) => {
            log::info!("mises à jour : version {} disponible", update.version);
            let info = UpdateInfo {
                version: update.version.clone(),
                notes: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
            };
            *up.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some(update);
            up.set(|s| {
                s.available = Some(info);
                s.last_check = Some(now_secs());
            });
        }
        Ok(None) => {
            log::info!("mises à jour : Dawsend est à jour");
            *up.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
            up.set(|s| {
                s.available = None;
                s.last_check = Some(now_secs());
            });
        }
        // Aucune release publiée pour l'instant : pas une panne.
        Err(UpdaterError::ReleaseNotFound) => {
            log::info!("mises à jour : aucune version publiée pour l'instant");
            up.set(|s| s.last_check = Some(now_secs()));
        }
        Err(e) => {
            log::warn!("mises à jour : vérification impossible : {e}");
            up.set(|s| s.error = Some(e.to_string()));
        }
    }
    up.set(|s| s.checking = false);
    Ok(up.status())
}

/// Télécharge, vérifie et lance l'installateur ; l'appli se ferme et l'installateur la
/// relance. Refuse tant qu'un DAW a le plugin chargé.
pub async fn install(state: Arc<AppState>) -> Result<(), String> {
    let loaded = state.engine_view().map_or(0, |v| v.sources.len());
    if loaded > 0 {
        log::warn!("mises à jour : installation refusée, {loaded} plugin(s) chargé(s) dans un DAW");
        return Err("daw_open".into());
    }
    let update = state
        .updates
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or("aucune mise à jour à installer")?;
    log::info!(
        "mises à jour : téléchargement de la version {} ({})",
        update.version,
        update.download_url
    );

    let up = &state.updates;
    up.set(|s| {
        s.downloading = Some((0, None));
        s.error = None;
    });
    let mut received = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                up.set(|s| s.downloading = Some((received, total)));
            },
            || {},
        )
        .await;
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => {
            log::error!("mises à jour : téléchargement ou signature refusés : {e}");
            up.set(|s| {
                s.downloading = None;
                s.error = Some(e.to_string());
            });
            return Err(e.to_string());
        }
    };
    log::info!(
        "mises à jour : {} octets reçus, signature vérifiée ; lancement de l'installateur",
        bytes.len()
    );

    // Enregistre les réglages et arrête le moteur (les plugins voient l'appli partir)
    // avant que l'installateur ne prenne la main ; `install` quitte le processus.
    let before = state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        before.save_now();
        let settings = before.engine_settings().unwrap_or_default();
        let mut engine = before.engine.lock().unwrap_or_else(PoisonError::into_inner);
        drop(engine.take());
        let r = update.install(bytes);
        // Ici seulement si l'installateur n'a pas pu être lancé : on remet le son.
        match Engine::start(settings) {
            Ok(e) => *engine = Some(e),
            Err(e) => log::error!("moteur audio impossible à redémarrer : {e}"),
        }
        r
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(e) = result {
        log::error!("mises à jour : lancement de l'installateur impossible : {e}");
        up.set(|s| {
            s.downloading = None;
            s.error = Some(e.to_string());
        });
        return Err(e.to_string());
    }
    Ok(())
}
