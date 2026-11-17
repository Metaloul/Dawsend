//! Réglages de l'appli, dans `settings.json` du dossier de configuration de l'appli
//! (`%APPDATA%\com.metaloul.dawsend\` sous Windows).
//!
//! Écriture atomique : fichier temporaire, `sync`, puis renommage. Un fichier illisible
//! est mis de côté (`settings.json.bak`) et l'appli repart des réglages par défaut.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use dawsend_app::service::EngineSettings;
use serde::{Deserialize, Serialize};

/// Préférences propres à l'appli (hors moteur audio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPrefs {
    /// « auto » (langue du système), « fr » ou « en ».
    pub language: String,
    /// Ouvrir la fenêtre quand l'appli démarre à la main (au démarrage de Windows, elle
    /// reste toujours dans la zone de notification).
    pub open_window_on_start: bool,
    /// Journal détaillé (niveau debug).
    pub verbose_log: bool,
    /// Chercher les nouvelles versions au démarrage puis toutes les 6 heures.
    pub check_updates: bool,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            open_window_on_start: true,
            verbose_log: false,
            check_updates: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsFile {
    pub engine: EngineSettings,
    pub app: AppPrefs,
}

pub fn path_in(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(path: &Path) -> SettingsFile {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(s) => {
                log::info!("réglages chargés depuis {}", path.display());
                s
            }
            Err(e) => {
                let bak = path.with_extension("json.bak");
                log::warn!(
                    "réglages illisibles dans {} ({e}) : mis de côté dans {}, réglages par défaut",
                    path.display(),
                    bak.display()
                );
                let _ = fs::rename(path, &bak);
                SettingsFile::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!(
                "pas encore de réglages ({}) : réglages par défaut",
                path.display()
            );
            SettingsFile::default()
        }
        Err(e) => {
            log::warn!("lecture des réglages impossible ({}) : {e}", path.display());
            SettingsFile::default()
        }
    }
}

pub fn save(path: &Path, settings: &SettingsFile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(settings)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("dawsend-settings-{}", std::process::id()));
        let p = path_in(&dir);
        let mut s = SettingsFile::default();
        s.engine.latency_ms = 17.0;
        s.engine.sources.insert(
            "Guitare".into(),
            dawsend_app::service::SourceSettings {
                enabled: true,
                gain: 0.5,
                muted: true,
            },
        );
        s.app.language = "en".into();
        save(&p, &s).unwrap();
        let back = load(&p);
        assert_eq!(back.engine, s.engine);
        assert_eq!(back.app, s.app);

        fs::write(&p, "{ pas du json").unwrap();
        let back = load(&p);
        assert_eq!(back.app, AppPrefs::default());
        assert!(p.with_extension("json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
