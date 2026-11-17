//! Textes du menu de la zone de notification (l'interface a ses propres textes, côté
//! Svelte). La langue vient des préférences : « auto » suit celle du système.

pub struct Texts {
    pub open: &'static str,
    pub mute_all: &'static str,
    pub sources: &'static str,
    pub no_source: &'static str,
    pub output: &'static str,
    pub default_output: &'static str,
    pub autostart: &'static str,
    pub quit: &'static str,
    /// « {v} » est remplacé par la version.
    pub update_available: &'static str,
    pub tip_waiting: &'static str,
    pub tip_playing: &'static str,
    pub tip_muted: &'static str,
    pub tip_error: &'static str,
}

const FR: Texts = Texts {
    open: "Ouvrir Dawsend",
    mute_all: "Couper le son",
    sources: "Sources",
    no_source: "Aucune source",
    output: "Sortie",
    default_output: "Sortie de Windows (par défaut)",
    autostart: "Lancer avec Windows",
    quit: "Quitter Dawsend",
    update_available: "Mise à jour {v} disponible…",
    tip_waiting: "Dawsend — en attente d'une source",
    tip_playing: "Dawsend — joue",
    tip_muted: "Dawsend — son coupé",
    tip_error: "Dawsend — sortie indisponible",
};

const EN: Texts = Texts {
    open: "Open Dawsend",
    mute_all: "Mute",
    sources: "Sources",
    no_source: "No source",
    output: "Output",
    default_output: "Windows output (default)",
    autostart: "Start with Windows",
    quit: "Quit Dawsend",
    update_available: "Update {v} available…",
    tip_waiting: "Dawsend — waiting for a source",
    tip_playing: "Dawsend — playing",
    tip_muted: "Dawsend — muted",
    tip_error: "Dawsend — output unavailable",
};

/// Langue effective (« fr » ou « en ») pour la préférence `pref`.
pub fn resolve(pref: &str) -> &'static str {
    match pref {
        "fr" => "fr",
        "en" => "en",
        _ => {
            let sys = sys_locale::get_locale().unwrap_or_default().to_lowercase();
            if sys.starts_with("fr") { "fr" } else { "en" }
        }
    }
}

pub fn texts(lang: &str) -> &'static Texts {
    if lang == "fr" { &FR } else { &EN }
}
