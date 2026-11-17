# Contribuer à Dawsend

Merci de l'intérêt ! Les signalements de bugs, les idées et les pull requests sont les
bienvenus. Ce guide dit comment proposer un changement et ce qu'il doit respecter pour
être accepté.

## Signaler un bug ou proposer une idée

- **Bug ou demande** : ouvre une [issue](https://github.com/Metaloul/Dawsend/issues).
  Indique la version de Dawsend (fenêtre de l'appli ou `CHANGELOG.md`), Windows, le DAW
  et l'interface audio, ce que tu as fait, ce que tu attendais et ce qui s'est passé.
  Joins le journal de l'appli si possible (chemin dans le
  [README](README.md#logs-and-settings)).
- **Faille de sécurité** : jamais d'issue publique. Utilise le signalement privé
  décrit dans [SECURITY.md](SECURITY.md).

Les issues et leurs réponses restent publiques et consultables : elles servent
d'archive des problèmes connus.

## Proposer un changement

1. Forke le dépôt et crée une branche à partir de `main`.
2. Fais ton changement, avec ses tests (voir plus bas).
3. Vérifie localement :

   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo deny check
   pnpm --dir ui build        # si l'interface a changé
   ```

4. Ouvre une pull request vers `main` en expliquant le pourquoi du changement. La CI
   (GitHub Actions) relance ces contrôles, plus CodeQL, cargo-audit, `pnpm audit` et,
   si `dawsend-core` change, le fuzzing.

Le guide détaillé pour travailler dans le code (carte du dépôt, règles du thread
audio, pièges connus) est [AGENTS.md](AGENTS.md).

## Exigences

- **Zéro avertissement** : `cargo clippy -D warnings` et `cargo fmt` doivent passer.
- **Tests** : toute nouvelle fonctionnalité arrive avec des tests automatisés qui la
  couvrent, et toute correction de bug avec un test qui échouait avant elle, quand le
  comportement se teste sans matériel audio. Les tests vivent à côté du code
  (`#[cfg(test)]`) et se lancent avec `cargo test --workspace`.
- **Temps réel** : dans le thread audio du plugin (`process`) et le callback de sortie
  de l'appli, aucune allocation, aucun verrou, aucun appel système bloquant.
- **Mémoire partagée** : son format est un contrat binaire. Toute modification passe
  par `crates/dawsend-core/src/layout.rs` (asserts de taille et d'offsets) et par
  `docs/ARCHITECTURE.md`.
- **Dépendances** : licence compatible (liste autorisée dans `deny.toml`), et
  `cargo deny check` vert.
- **Textes** : tout texte de l'interface existe en français et en anglais.
- **Workflows GitHub** : actions épinglées par SHA, droits en lecture seule par défaut
  (voir AGENTS.md § 8).
- **Documentation** : une fonctionnalité visible arrive avec sa place dans le README,
  et sa ligne dans `CHANGELOG.md`.

Code, commentaires et documentation sont en français, sauf le `README.md`, en
anglais. Messages de commit en français, sujet court.

## Licence des contributions

En proposant une contribution, tu acceptes qu'elle soit publiée sous la licence MIT
du projet ([LICENSE](LICENSE)). Le binaire `.vst3` est distribué sous GPLv3 à cause de
`vst3-sys` : voir [LICENSES/README.md](LICENSES/README.md).
