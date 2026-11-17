# AGENTS.md — Dawsend

Guide pour les agents de code (Claude Code, Codex, Cursor, Copilot, Gemini…) et pour
les humains qui découvrent le dépôt. Il dit ce qu'est le projet, où sont les choses,
quelles règles ne se discutent pas et comment vérifier son travail. Le détail est dans
`docs/` ; ce fichier sert de point d'entrée.

> **Langue** : tout est en **français** (docs, commentaires du code, messages de
> commit, journaux, textes de l'interface). Les identifiants du code restent en
> anglais. Les textes visibles par l'utilisateur existent en **français et en
> anglais**. Exceptions : le `README.md` et le schéma `docs/images/how-it-works.svg`
> sont **en anglais** (vitrine publique du dépôt).

## 1. Le projet en bref

Dawsend envoie le son d'un DAW qui sort en **ASIO** (Studio One, Reaper…) vers une
**sortie Windows normale**, pour que le partage d'écran Discord ou OBS l'entende.

```
DAW (ASIO)                    mémoire partagée locale             appli Dawsend
plugin Dawsend (VST3/CLAP) ──► Local\Dawsend.v1 : registre ──► lecture + validation
pass-through, copie le son     + 32 anneaux SPSC « avec perte »   → rééchantillonnage + PI (dérive)
                                                                  → mixage → limiteur
                                                                  → sortie WASAPI (cpal)
                                                                  → Discord / OBS
```

- **Plugin** : nih-plug (VST3 + CLAP), interface egui. Laisse passer le son sans le
  modifier, en copie une version dans la mémoire partagée.
- **Appli** : Tauri 2 (zone de notification), interface Svelte 5, moteur audio en
  Rust. Lit les sources, recale les horloges, mixe, joue sur une sortie Windows.
- **Aucun réseau** sauf la vérification des mises à jour (releases GitHub).
- Cible actuelle : **Windows 10/11 x64**. Linux (PipeWire) et macOS sont prévus ; le
  code isole déjà la plateforme (`dawsend-core/src/shm/`).

Schéma illustré (en anglais) : `docs/images/how-it-works.svg`. Pourquoi ce projet existe et ce
qu'il promet à l'utilisateur : `README.md`.

## 2. À lire avant de toucher au code

| Document | Pourquoi |
| --- | --- |
| `docs/ARCHITECTURE.md` | Format **v1** de la mémoire partagée (§ 3), threads (§ 5.1), dérive (§ 5.2), latence (§ 5.3), licences (§ 7), décisions validées (§ 9). |
| `docs/CAHIER_DES_CHARGES.md` | La demande d'origine et le **tableau de suivi des étapes**, à tenir à jour. |
| `docs/TEST-ETAPES-2-4.md`, `docs/TEST-ETAPE-1.md` | Ce qui a été vérifié sur un vrai PC, ce qui reste à tester. |
| `docs/EVOLUTIONS.md` | Feuille de route : idées classées, principes d'extensibilité. |
| `docs/SECURITE.md` | Ce que l'appli fait sur la machine, contrôles de sécurité et résultats. |
| `assets/palette.md` | Couleurs, polices et jetons du design (appli et plugin). |
| `CLAUDE.md` | État d'avancement daté, pour reprendre une session. |

## 3. Règles non négociables

1. **Temps réel.** Dans `process()` du plugin et dans le callback de sortie de
   l'appli : **aucune allocation, aucun verrou, aucun appel système bloquant**, pas
   de journal. Tout se crée ailleurs (`initialize()`, thread de contrôle) puis passe
   par des files sans verrou (`rtrb`) ou des atomiques. En debug, nih-plug fait
   planter le plugin si `process()` alloue (`assert_process_allocs`).
2. **Le plugin ne bloque jamais le DAW et ne le fait jamais planter.** Au moindre
   problème (mémoire partagée absente, appli fermée, erreur), il reste un simple
   pass-through. Il n'attend jamais le lecteur et ne lit pas sa position.
3. **La mémoire partagée est un contrat binaire.** Toute modification passe par
   `crates/dawsend-core/src/layout.rs` (structures `#[repr(C)]`, asserts de taille et
   d'offset à la compilation) **et** par `docs/ARCHITECTURE.md` § 3. Une fois une
   version distribuée, toute rupture **incrémente `LAYOUT_VERSION`** (le nom de la
   zone change avec elle).
4. **L'appli valide tout ce qu'elle lit** dans la mémoire partagée (version, bornes,
   cohérence). Elle ne doit ni paniquer ni avoir de comportement indéfini, même face à
   une zone corrompue.
5. **Licences.** Code sous MIT ; le binaire `.vst3` est sous GPLv3 (à cause de
   `vst3-sys`), le `.clap` sous MIT. Toute nouvelle dépendance doit passer
   `cargo deny check` (liste des licences dans `deny.toml`). Pas de code GPL dans
   l'appli ni dans `dawsend-core`.
6. **Asynchrone.** Les commandes Tauri sont `async`. Le travail bloquant (disque,
   audio, fenêtres, attente) part dans `tauri::async_runtime::spawn_blocking`.
7. **Secrets.** Jamais de clé privée, de mot de passe ou de jeton dans le dépôt.
   `crates/dawsend-gui/updater-key.pub` est la clé **publique** de mise à jour (sa
   copie est aussi dans `tauri.conf.json`). La clé privée vit hors du dépôt et dans
   les secrets GitHub. Ne jamais générer ni remplacer la clé de signature : c'est le
   mainteneur qui le fait.

## 4. Carte du dépôt

| Chemin | Rôle |
| --- | --- |
| `crates/dawsend-core/` | Sans audio ni GUI. `layout.rs` (contrat binaire), `ring.rs` (anneau SPSC sur pointeurs bruts), `registry.rs` (slots, seqlock des métadonnées), `region.rs`, `clock.rs`, `process.rs`, `shm/{windows,unix}.rs` (création de la zone). |
| `crates/dawsend-plugin/` | `lib.rs` (plugin nih-plug, `process()`), `editor.rs` (interface egui, façade V2), `i18n.rs` (textes FR/EN). |
| `crates/dawsend-app/` | Moteur : `engine/source.rs` (lecture d'une source), `engine/drift.rs` (régulateur PI + rubato), `engine/mixer.rs`, `output/` (trait `Renderer`, backend cpal/WASAPI), `service.rs` (`Engine`, thread de contrôle, vues et vumètres). `src/bin/dawsend-console.rs` : outil de test (`list`, `devices`, `sine`, `play`). |
| `crates/dawsend-gui/` | Appli Tauri (`dawsend.exe`) : `main.rs` (commandes, état, journaux), `tray.rs` (zone de notification), `settings.rs` (réglages JSON écrits d'un seul coup), `updates.rs` (mises à jour), `i18n.rs` (textes du menu), `tauri.conf.json`, `capabilities/`. |
| `ui/` | Interface Svelte 5 (Vite) : `App.svelte`, `components/` (`SourceCard`, `Knob`, `Meter`), `lib/api.js` (appels au cœur), `lib/i18n.js` (textes FR/EN), `lib/meters.svelte.js` (vumètres à chaque image). |
| `installer/` | `dawsend.nsi` (NSIS), `build.ps1` (Windows), `build.sh` (Linux, mingw), `LISEZMOI.txt`. |
| `scripts/latest-json.mjs` | Écrit `latest.json` et les notes de release à partir de `CHANGELOG.md`. |
| `xtask/` | `cargo xtask bundle dawsend-plugin --release` (nih-plug). |
| `.github/workflows/` | `ci.yml`, `security.yml`, `scorecard.yml`, `release.yml`. |
| `assets/` | Logo, wordmark, icônes, polices (OFL), palette. |
| `LICENSES/` | Textes GPL-3.0, OFL-1.1 et récapitulatif des licences. |

## 5. Commandes

```sh
cargo test --workspace                        # tests (cœur, moteur, appli)
cargo clippy --workspace --all-targets -- -D warnings   # zéro avertissement
cargo fmt --all
cargo deny check                              # failles, licences, provenance
cargo audit                                   # failles connues (RustSec)
cargo xtask bundle dawsend-plugin --release   # → target/bundled/Dawsend.vst3, Dawsend.clap
pnpm --dir ui install && pnpm --dir ui build  # interface → ui/dist
pnpm --dir ui audit --prod
cargo build --release -p dawsend-gui          # → target/release/dawsend.exe
cargo build --release -p dawsend-app --bin dawsend-console
powershell -ExecutionPolicy Bypass -File installer\build.ps1   # installateur complet
```

- **Interface en direct** : `pnpm --dir ui dev`, puis
  `cargo run -p dawsend-gui --no-default-features` (sans `custom-protocol`, l'appli
  charge `http://localhost:5173`).
- **Tester sans DAW** : `dawsend-console sine --name Guitare` (publie un bip comme un
  plugin), puis l'appli ou `dawsend-console play`. `--rate` et `--ppm` simulent une
  autre fréquence et une dérive.
- **Toolchain** : Rust **1.93** au minimum (`rust-version` du workspace, utilisé par
  le résolveur de Cargo), édition 2024. Node 22, pnpm 10, NSIS 3.

## 6. Avant de dire « c'est fini »

1. `cargo fmt --all`, `cargo clippy … -D warnings`, `cargo test --workspace` passent.
2. Si l'interface a changé : `pnpm --dir ui build` passe.
3. Si une dépendance a changé : `cargo deny check` et `cargo audit` passent.
4. Si `layout.rs` a changé : asserts mis à jour, `docs/ARCHITECTURE.md` § 3 aussi,
   `LAYOUT_VERSION` incrémentée si une version est déjà distribuée.
5. Une fonctionnalité visible arrive avec ses textes **FR et EN**, ses lignes de
   journal et sa place dans le README ; une étape du cahier des charges, avec la mise
   à jour de son tableau de suivi et du doc de test.
6. Dire honnêtement ce qui a été testé pour de vrai (sur un PC, avec un DAW) et ce qui
   ne l'a été que par des tests automatiques.

## 7. Conventions

### Journaux

- Appli : crate `log` vers `tauri-plugin-log` (fichier
  `%LOCALAPPDATA%\com.metaloul.dawsend\logs\dawsend.log`, rotation 2 Mo × 5).
- `info` : démarrage et version, sortie ouverte, sources qui arrivent ou partent,
  réglages enregistrés, mises à jour. `warn`/`error` : toute erreur rattrapée, avec
  ce qu'on tentait de faire. `debug` : timing et décisions (activable sans
  redémarrer, case « Journal détaillé »).
- Rien en `info` à chaque bloc audio ou à chaque image. Jamais de journal dans le
  thread temps réel.
- L'interface écrit dans le même journal (`log` de `ui/src/lib/api.js`), pas dans
  `console.log`.

### Textes de l'interface

Trois endroits, toujours en FR **et** EN : `ui/src/lib/i18n.js` (fenêtre),
`crates/dawsend-gui/src/i18n.rs` (menu de la zone de notification),
`crates/dawsend-plugin/src/i18n.rs` (plugin). La doc utilisateur (README) tutoie et
reprend les libellés exacts de l'interface.

### Design

Jetons et polices dans `assets/palette.md` (façade sombre, accent ambre `#FF7A2F`,
Familjen Grotesk et Geist Mono). La maquette de référence est dans Figma (fichier
`XRV3PYbMQ9VljOyYCER9Ch`, page « V2 — Interface »).

### Documentation

- Le README est **en anglais** et décrit le projet **tel qu'il est**, jamais par
  rapport à un changement (« now », « replaces »…) : l'historique va dans les commits
  et le `CHANGELOG.md`.
- **Aucun émoji** dans le README. Les icônes sont des Octicons (MIT) générées dans
  `docs/icons/` par `scripts/readme-icons.mjs` (mode d'emploi en tête du script),
  couleur Braise `#E0601F`, insérées en `<img … width="16" height="16" alt="">`.
  Vérifier le rendu en thème clair et sombre de GitHub.
- `CHANGELOG.md` : une section `## X.Y.Z` par version, écrite pour les utilisateurs.
  Elle devient les notes de la release et le texte du bandeau de mise à jour ; le
  workflow de release échoue si elle manque.

### Git

- Messages de commit en français, à l'impératif ou descriptifs, sujet court.
- Pas de commit ni de push sans que le mainteneur le demande.

## 8. Publier une version

1. Monter `version` dans `Cargo.toml` (`[workspace.package]`).
2. Ajouter `## X.Y.Z` en tête de `CHANGELOG.md`.
3. Pousser le tag `vX.Y.Z` : `.github/workflows/release.yml` vérifie que le tag
   correspond à la version, teste, construit l'installateur et le zip portable, signe
   l'installateur (`tauri signer sign`), écrit `latest.json` et `SHA256SUMS.txt`,
   publie la release et envoie l'installateur et le zip à VirusTotal si `VT_API_KEY`
   est configuré.

L'appli lit `https://github.com/Metaloul/Dawsend/releases/latest/download/latest.json`
(clés de plateforme `windows-x86_64` et `windows-x86_64-nsis`). Elle installe en
mode silencieux : l'installateur reçoit `/S /UPDATE`, garde les choix précédents
(raccourci, VST3, CLAP) et relance l'appli sans droits administrateur. L'appli refuse
d'installer tant qu'un plugin est chargé dans un DAW (Windows ne remplace pas un
`.vst3` ouvert).

**Version portable** (`Dawsend-X.Y.Z-portable.zip`, faite par `installer/build.ps1`) :
les fichiers de l'installateur plus `installer/portable.txt`. La présence de ce
fichier à côté de `dawsend.exe` (`crates/dawsend-gui/src/portable.rs`) range réglages
et journaux dans `data\` à côté de l'exécutable, et le bouton de mise à jour ouvre la
page de la release au lieu de lancer l'installateur.

## 9. Pièges connus

- **La CI prend le Rust stable du moment**, plus récent que celui du poste : un
  nouveau lint de clippy peut la faire échouer sans changement de code. Garder la
  toolchain locale à jour (`rustup update stable`) avant de pousser.

- **`installer/dawsend.nsi` est en CRLF avec BOM UTF-8.** Le garder tel quel (en
  Python : `open(..., newline="")`). Le vérifier avec
  `makensis` après chaque modification.
- **Encodage sous Windows** : un script Python passé par un heredoc de Git Bash peut
  être lu dans le mauvais encodage et casser les accents. Écrire le script dans un
  fichier et le lancer avec `python -X utf8`.
- **Plugin en fenêtre autonome** : la feature `standalone` de nih-plug entre en
  conflit (`links = "alsa"`) avec la version de cpal du workspace. Pour voir
  l'interface du plugin hors DAW, utiliser une crate à part, hors du workspace.
- **`assert_process_allocs`** est désactivé pour la cible `windows-gnu` (compilation
  croisée depuis Linux) : les vérifications d'allocation ne se font qu'avec la cible
  MSVC.
- **nih-plug et ses dépendances** viennent de dépôts Git épinglés : les dépôts
  autorisés sont listés dans `deny.toml` (`[sources] allow-git`).
- **Vumètres de l'appli** : ils passent par la commande `get_meters`, lue à chaque
  image ; l'état complet (`get_state`) n'est lu que quelques fois par seconde. Ne pas
  remettre les crêtes dans l'état complet.
- **Le panneau d'aperçu d'un navigateur intégré** ralentit `requestAnimationFrame`
  quand il est caché : on n'y mesure pas la fluidité des vumètres.
- **Discord et le partage d'une seule fenêtre** : il ne capte probablement que le son
  du processus de la fenêtre (voir `docs/EVOLUTIONS.md` § 1). À ne pas « corriger »
  dans le code sans avoir testé.

## 10. Où aller ensuite

La prochaine étape prévue est l'**étape 6 : Linux (PipeWire) puis macOS**, puis la
signature du code (Authenticode). Les idées plus larges (assistant de premier
lancement, VBAN, plugin OBS, API de contrôle OSC, format v2) sont classées dans
`docs/EVOLUTIONS.md`, avec la règle : chaque extension reste invisible pour qui ne
s'en sert pas.
