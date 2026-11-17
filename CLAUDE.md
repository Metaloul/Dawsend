# Dawsend — contexte pour Claude Code

Projet de Romain Rousseau (Metaloul). On échange **en français** (docs, messages,
commentaires du code). Seuls le `README.md` et le schéma `docs/images/how-it-works.svg`
sont en anglais, sans émojis (icônes Octicons, voir `AGENTS.md` § 7).

Dawsend envoie le son d'un DAW en ASIO (Studio One) vers une sortie Windows normale,
pour que le partage d'écran (Discord) l'entende. Il se compose d'un plugin nih-plug
(VST3 + CLAP) et d'une appli Tauri (zone de notification) qui lit une mémoire partagée.

## À lire avant tout travail

0. `AGENTS.md` : le guide commun à tous les agents de code (carte du dépôt, règles,
   commandes, vérifications, pièges connus). Ce fichier-ci ne garde que l'état
   d'avancement.
1. `docs/CAHIER_DES_CHARGES.md` : la demande d'origine et le **tableau de suivi des
   étapes** (à tenir à jour).
2. `docs/ARCHITECTURE.md` : le format v1 de la mémoire partagée, les protocoles, et
   les décisions validées (§ 9).
3. `docs/TEST-ETAPES-2-4.md` : ce qui a été vérifié sur le PC et ce qui reste à
   tester (`docs/TEST-ETAPE-1.md` pour l'étape 1).

## État au 17/11/2026

- Étapes 1 à 4 livrées et vérifiées sur le PC de Romain (Windows 11, Behringer
  UMC 404HD) sans DAW : dérive (rubato + PI, `engine/drift.rs`), multi-sources et
  mixage, appli Tauri `crates/dawsend-gui` (interface Svelte dans `ui/`), zone de
  notification, réglages, journaux, français et anglais.
- Design V2 (maquette Figma `XRV3PYbMQ9VljOyYCER9Ch`, page « V2 — Interface » ;
  jetons et polices dans `assets/palette.md`) appliqué à l'appli (`ui/`) et au
  plugin (`editor.rs`, `i18n.rs`, polices dans `assets/fonts/`). Plugin vu en
  fenêtre autonome nih-plug hors DAW, **pas encore dans Studio One**.
- Installateur NSIS : installe `dawsend.exe` (+ `dawsend-console.exe` pour les
  tests), testé sur le PC. `installer/build.sh` pas relancé depuis l'appli Tauri.
- Vumètres de l'appli lus à chaque image (`get_meters`, `ui/src/lib/meters.svelte.js`).
- Mises à jour : `tauri-plugin-updater`, `latest.json` des releases GitHub
  (`Metaloul/Dawsend`), clé publique dans `crates/dawsend-gui/updater-key.pub`, clé
  privée hors dépôt (`C:\Users\Administrator\.tauri\dawsend.key`, secrets GitHub).
  Publication : tag `vX.Y.Z` → `.github/workflows/release.yml` (section du
  `CHANGELOG.md` obligatoire). Pas encore testées de bout en bout (aucune release).
- Sécurité : `deny.toml`, workflows `security.yml` (cargo-deny, cargo-audit, pnpm
  audit, CodeQL) et `scorecard.yml`, résultats dans `docs/SECURITE.md`.
- Pistes d'évolution classées : `docs/EVOLUTIONS.md`. **À tester d'abord : ce que
  Discord capte en partage de fenêtre** (§ 1).
- **Prochaine étape : 6, Linux (PipeWire) puis macOS.** Reste aussi : signature du
  code (Authenticode).

## Règles non négociables

- Thread audio du plugin (`process`) et callback de sortie de l'appli : **aucune
  allocation, aucun verrou, aucun appel système bloquant**. Toute création se fait
  ailleurs (`initialize()`, thread de contrôle), puis passe par des files lock-free
  (`rtrb`).
- Le plugin ne doit jamais bloquer ni faire planter le DAW. En cas de problème, il
  reste un simple pass-through.
- Le format de la mémoire partagée est un contrat binaire. Toute modification passe
  par `layout.rs` (asserts de taille et d'offsets) et par la doc. Une fois une version
  distribuée, toute modification **incrémente `LAYOUT_VERSION`**.
- L'appli valide tout ce qu'elle lit dans la mémoire partagée.

## Commandes

```sh
cargo test --workspace                       # tests (cœur + moteur)
cargo clippy --workspace --all-targets       # doit rester sans avertissement
cargo fmt --all
cargo xtask bundle dawsend-plugin --release  # → target/bundled/Dawsend.vst3, Dawsend.clap
pnpm --dir ui install && pnpm --dir ui build  # interface de l'appli → ui/dist
cargo build --release -p dawsend-gui         # appli → target/release/dawsend.exe
cargo build --release -p dawsend-app --bin dawsend-console
powershell -ExecutionPolicy Bypass -File installer\build.ps1   # installateur (Windows + NSIS)
installer/build.sh                                             # installateur (Linux, mingw)
```

Test sans DAW : `dawsend-console sine --name Guitare` dans un terminal, puis l'appli
(`dawsend.exe`) ou `dawsend-console play`.

## Licences

Code sous MIT. Le binaire `.vst3` est sous GPLv3 (à cause de `vst3-sys`), le `.clap`
sous MIT. Toute nouvelle dépendance doit avoir une licence compatible.
