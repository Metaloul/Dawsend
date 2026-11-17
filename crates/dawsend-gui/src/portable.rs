//! Version portable : sans installation, lancée depuis un dossier (clé USB, dossier
//! personnel). Un fichier `portable.txt` à côté de `dawsend.exe` l'active :
//!
//! - réglages et journaux vont dans `data\` à côté de l'exécutable, au lieu de
//!   `%APPDATA%` et `%LOCALAPPDATA%` ;
//! - une mise à jour ouvre la page de téléchargement au lieu de lancer l'installateur,
//!   qui installerait Dawsend dans `Program Files` à côté de la version portable.

use std::path::PathBuf;

/// Fichier qui active le mode portable, livré dans le zip de la version portable.
pub const MARKER: &str = "portable.txt";

/// Page où télécharger la dernière version (zip portable compris).
pub const RELEASES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/latest");

/// Dossier des données en mode portable (`<dossier de dawsend.exe>\data`), `None` pour
/// une version installée.
pub fn data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    dir.join(MARKER).is_file().then(|| dir.join("data"))
}
