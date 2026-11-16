# Dawsend — contexte pour Claude Code

Projet de Romain Rousseau (Metaloul). On échange **en français** (docs, messages,
commentaires du code).

Dawsend envoie le son d'un DAW en ASIO (Studio One) vers une sortie Windows normale,
pour que le partage d'écran (Discord) l'entende. Il se compose d'un plugin nih-plug
(VST3 + CLAP) et d'une appli qui lit une mémoire partagée.

## À lire avant tout travail

1. `docs/CAHIER_DES_CHARGES.md` : la demande d'origine et le **tableau de suivi des
   étapes** (à tenir à jour).
2. `docs/ARCHITECTURE.md` : le format v1 de la mémoire partagée, les protocoles, et
   les décisions validées (§ 9).
3. `docs/TEST-ETAPE-1.md` : ce qui a été vérifié et ce qui reste à tester sur le PC.

## État au 16/11/2026

- Étape 1 (prototype plugin + console) : livrée, **pas encore testée sur le vrai PC**
  (WASAPI, Studio One, Discord).
- Hautes fréquences (8 à 768 kHz, anti-repliement) : faites.
- Installateur NSIS 64 bits : fait (`installer/`), testé seulement sous Wine.
- **Prochaine étape : 2, compensation de dérive.** Remplacer le rééchantillonneur
  linéaire de `crates/dawsend-app/src/engine/source.rs` par rubato (asynchrone,
  sans allocation dans le callback). Ajouter un régulateur PI sur le remplissage
  lissé (± 1 000 ppm max, en rampe), un fondu croisé lors des resynchros, et une
  marge minimale qui s'adapte à la taille de bloc observée du DAW (voir
  ARCHITECTURE § 5.2). Ensuite : étapes 3 (multi-sources, interface du plugin
  egui), 4 (Tauri), 5, puis 6.

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
cargo build --release -p dawsend-app --bin dawsend-console
powershell -ExecutionPolicy Bypass -File installer\build.ps1   # installateur (Windows + NSIS)
installer/build.sh                                             # installateur (Linux, mingw)
```

Test sans DAW : `dawsend-console sine` dans un terminal, `dawsend-console play` dans
un autre.

## Licences

Code sous MIT. Le binaire `.vst3` est sous GPLv3 (à cause de `vst3-sys`), le `.clap`
sous MIT. Toute nouvelle dépendance doit avoir une licence compatible.
