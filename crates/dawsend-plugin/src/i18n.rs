//! Textes de l'interface du plugin, en français et en anglais. La langue suit celle du
//! système (français si la langue de Windows commence par « fr », anglais sinon).

pub struct Texts {
    pub source_name: &'static str,
    pub send: &'static str,
    pub send_hint: &'static str,
    pub listening: &'static str,
    pub listening_hint: &'static str,
    pub listening_muted: &'static str,
    pub listening_muted_hint: &'static str,
    pub connected: &'static str,
    pub connected_hint: &'static str,
    pub app_absent: &'static str,
    pub app_absent_hint: &'static str,
    pub unavailable: &'static str,
    pub not_active: &'static str,
    pub not_active_hint: &'static str,
    pub rename_hint: &'static str,
    pub send_off: &'static str,
    pub send_off_hint: &'static str,
    pub github: &'static str,
}

pub const FR: Texts = Texts {
    source_name: "Source",
    send: "Envoi",
    send_hint: "Désactivé, le plugin envoie du silence à l'appli. Le son de la piste n'est jamais modifié.",
    listening: "Relié — l'appli joue cette piste",
    listening_hint: "Tout fonctionne : le son part vers la sortie Windows.",
    listening_muted: "Relié — piste coupée dans l'appli",
    listening_muted_hint: "Réactive le son de cette source dans l'appli Dawsend.",
    connected: "Appli ouverte — piste non jouée",
    connected_hint: "Active cette source dans l'appli Dawsend.",
    app_absent: "Appli Dawsend fermée",
    app_absent_hint: "Lance Dawsend (menu Démarrer) : la liaison se fait toute seule.",
    unavailable: "Envoi impossible",
    not_active: "En attente du moteur audio",
    not_active_hint: "Active le moteur audio du DAW ou lance la lecture.",
    rename_hint: "Entrée pour valider. Le nom est sauvegardé avec le projet.",
    send_off: "Envoi coupé",
    send_off_hint: "Le son passe dans la piste mais ne part plus vers Dawsend.",
    github: "Voir le projet sur GitHub",
};

pub const EN: Texts = Texts {
    source_name: "Source",
    send: "Send",
    send_hint: "When off, the plugin sends silence to the app. The track audio is never changed.",
    listening: "Linked — the app plays this track",
    listening_hint: "Everything works: audio goes to the Windows output.",
    listening_muted: "Linked — track muted in the app",
    listening_muted_hint: "Unmute this source in the Dawsend app.",
    connected: "App running — track not played",
    connected_hint: "Enable this source in the Dawsend app.",
    app_absent: "Dawsend app closed",
    app_absent_hint: "Start Dawsend (Start menu): the link is made automatically.",
    unavailable: "Cannot send",
    not_active: "Waiting for the audio engine",
    not_active_hint: "Turn on the DAW audio engine or start playback.",
    rename_hint: "Press Enter to apply. The name is saved with the project.",
    send_off: "Send off",
    send_off_hint: "Audio still plays through the track but no longer goes to Dawsend.",
    github: "View the project on GitHub",
};

/// Textes dans la langue du système.
pub fn system() -> &'static Texts {
    let lang = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    if lang.starts_with("fr") { &FR } else { &EN }
}
