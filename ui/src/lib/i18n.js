// Textes de l'interface. La langue vient des préférences (« auto » : celle du système,
// résolue par le cœur Rust).
const fr = {
  secUpdates: "Mises à jour",
  github: "Voir le projet sur GitHub",
  updAvailable: "Dawsend {v} est disponible",
  updNotes: "Nouveautés",
  updInstall: "Installer",
  updDownload: "Télécharger",
  updPortable:
    "Version portable : la page de téléchargement s'ouvre. Quitte Dawsend, puis remplace les fichiers du dossier par ceux du nouveau zip (garde le dossier data).",
  updDownloading: "Téléchargement… {pct}",
  updDawOpen:
    "Ferme d'abord ton DAW (ou retire Dawsend de tes pistes) : Windows ne peut pas remplacer un plugin chargé.",
  updUac: "Windows va demander l'autorisation d'installer. Dawsend se relance ensuite tout seul.",
  updVersion: "Version installée",
  updVersionPortable: "Version portable",
  updCheck: "Vérifier",
  updChecking: "Vérification…",
  updUpToDate: "Dawsend est à jour (vérifié à {time}).",
  updNever: "Pas encore vérifié.",
  updDisabled: "Les mises à jour ne sont pas disponibles dans cette version de développement.",
  updError: "Vérification impossible : {e}",
  updAuto: "Chercher les mises à jour",
  updAutoHint:
    "Au démarrage puis toutes les 6 heures, sur la page GitHub de Dawsend. Rien ne s'installe sans ton accord.",
  secOutput: "Sortie",
  secMaster: "Général",
  statusOn: "En service",
  statusIdle: "En veille",
  stMuted: "Coupée",
  channels: "{n} canaux",
  latencyShort: "latence ≈ {ms} ms",
  autostartHint: "Dawsend démarre dans la zone de notification, sans fenêtre.",
  verboseHint: "Pour un dépannage : note le détail du timing et des décisions.",
  knobHint: "Glisser vers le haut ou le bas, molette ou flèches. Double-clic : 0 dB.",
  tabSources: "Sources",
  tabSettings: "Réglages",
  outputIdle: "En veille — aucune source à jouer",
  outputError: "Sortie indisponible",
  engineError: "Le moteur audio n'a pas pu démarrer",
  noSources: "Aucune source",
  noSourcesHint:
    "Ajoute le plugin Dawsend en insert sur une piste de ton DAW (en dernier dans la chaîne), puis lance le moteur audio. La source apparaîtra ici.",
  stPlaying: "Jouée",
  mono: "mono",
  stereo: "stéréo",
  stPriming: "Démarrage…",
  stPaused: "En pause",
  stPausedHint: "DAW arrêté ou piste inactive",
  stDisabled: "Désactivée",
  enable: "Jouer cette source",
  mute: "Couper",
  unmute: "Rétablir",
  volume: "Volume",
  details: "Marge {fill}/{target} ms · dérive {ppm} ppm",
  dropouts: "coupures : {n}",
  master: "Volume général",
  output: "Sortie audio",
  outputHint:
    "Le partage d'écran (Discord…) capte la sortie Windows par défaut. Garde-la sauf besoin particulier.",
  defaultOutput: "Sortie de Windows (par défaut)",
  latency: "Marge de sécurité",
  latencyHint:
    "Plus petite : moins de décalage. Plus grande : moins de risque de coupure. Latence totale estimée : {total} ms.",
  latencyAuto: "Relevée automatiquement à {ms} ms pour « {name} » (gros blocs du DAW).",
  language: "Langue",
  langAuto: "Celle du système",
  startup: "Démarrage",
  autostart: "Lancer avec Windows",
  openWindow: "Ouvrir la fenêtre au lancement",
  closeHint:
    "Fermer la fenêtre ne quitte pas Dawsend : il reste près de l'horloge. Pour quitter : clic droit sur l'icône, « Quitter Dawsend ».",
  logs: "Journaux",
  openLogs: "Ouvrir le dossier",
  verbose: "Journal détaillé",
  version: "Version {v}",
  antiAlias: "anti-repliement",
};

const en = {
  secUpdates: "Updates",
  github: "View the project on GitHub",
  updAvailable: "Dawsend {v} is available",
  updNotes: "What's new",
  updInstall: "Install",
  updDownload: "Download",
  updPortable:
    "Portable version: the download page opens. Quit Dawsend, then replace the files in the folder with those from the new zip (keep the data folder).",
  updDownloading: "Downloading… {pct}",
  updDawOpen:
    "Close your DAW first (or remove Dawsend from your tracks): Windows cannot replace a loaded plugin.",
  updUac: "Windows will ask for permission to install. Dawsend then restarts by itself.",
  updVersion: "Installed version",
  updVersionPortable: "Portable version",
  updCheck: "Check",
  updChecking: "Checking…",
  updUpToDate: "Dawsend is up to date (checked at {time}).",
  updNever: "Not checked yet.",
  updDisabled: "Updates are not available in this development build.",
  updError: "Could not check: {e}",
  updAuto: "Check for updates",
  updAutoHint:
    "At startup, then every 6 hours, on Dawsend's GitHub page. Nothing is installed without your consent.",
  secOutput: "Output",
  secMaster: "Master",
  statusOn: "Running",
  statusIdle: "Standby",
  stMuted: "Muted",
  channels: "{n} channels",
  latencyShort: "latency ≈ {ms} ms",
  autostartHint: "Dawsend starts in the notification area, without a window.",
  verboseHint: "For troubleshooting: records timing and decision details.",
  knobHint: "Drag up or down, scroll or use the arrow keys. Double-click: 0 dB.",
  tabSources: "Sources",
  tabSettings: "Settings",
  outputIdle: "Standby — no source to play",
  outputError: "Output unavailable",
  engineError: "The audio engine could not start",
  noSources: "No source",
  noSourcesHint:
    "Insert the Dawsend plugin on a track in your DAW (last in the chain), then turn on the audio engine. The source will show up here.",
  stPlaying: "Playing",
  mono: "mono",
  stereo: "stereo",
  stPriming: "Starting…",
  stPaused: "Paused",
  stPausedHint: "DAW stopped or track inactive",
  stDisabled: "Disabled",
  enable: "Play this source",
  mute: "Mute",
  unmute: "Unmute",
  volume: "Volume",
  details: "Buffer {fill}/{target} ms · drift {ppm} ppm",
  dropouts: "dropouts: {n}",
  master: "Master volume",
  output: "Audio output",
  outputHint:
    "Screen sharing (Discord…) captures the default Windows output. Keep it unless you need otherwise.",
  defaultOutput: "Windows output (default)",
  latency: "Safety margin",
  latencyHint:
    "Smaller: less delay. Larger: less risk of dropouts. Estimated total latency: {total} ms.",
  latencyAuto: "Raised automatically to {ms} ms for “{name}” (large DAW blocks).",
  language: "Language",
  langAuto: "System language",
  startup: "Startup",
  autostart: "Start with Windows",
  openWindow: "Open the window at launch",
  closeHint:
    "Closing the window does not quit Dawsend: it stays next to the clock. To quit: right-click the icon, “Quit Dawsend”.",
  logs: "Logs",
  openLogs: "Open folder",
  verbose: "Detailed log",
  version: "Version {v}",
  antiAlias: "anti-aliasing",
};

const dicts = { fr, en };

export function translator(lang) {
  const d = dicts[lang] ?? en;
  const t = (key, vars = {}) =>
    (d[key] ?? en[key] ?? key).replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");
  // Nombre à `digits` décimales, avec la virgule ou le point de la langue.
  const locale = dicts[lang] ? lang : "en";
  t.time = (secs) =>
    new Date(secs * 1000).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  t.num = (v, digits = 1) =>
    v.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  return t;
}
