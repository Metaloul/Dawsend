# Sécurité

## Signaler une faille

Merci de **ne pas ouvrir d'issue publique** pour une faille de sécurité. Utilise le
bouton **« Report a vulnerability »** de l'onglet
[Security](https://github.com/Metaloul/Dawsend/security/advisories/new) du dépôt :
le signalement reste privé jusqu'à la correction.

Indique la version de Dawsend, ton système et, si possible, les étapes pour reproduire.
Tu reçois une première réponse sous 14 jours au plus. Une faille confirmée est
corrigée dans une nouvelle version, puis publiée dans un avis de sécurité GitHub et
dans les notes de la release.

## Versions suivies

Seule la dernière version publiée reçoit des correctifs. L'appli propose elle-même la
mise à jour.

## Ce qui protège les utilisateurs

- **Mises à jour signées** : l'appli refuse tout installateur dont la signature ne
  correspond pas à la clé publique de Dawsend intégrée au programme, et toute
  signature émise pour une autre version que celle annoncée.
- **Rien n'écoute sur le réseau** : l'appli et le plugin communiquent par une mémoire
  partagée locale. La seule connexion sortante est la vérification des mises à jour sur
  `github.com`, désactivable dans les réglages.
- **Analyses automatiques** du code et des dépendances à chaque modification (CodeQL,
  cargo-deny, cargo-audit, pnpm audit, OpenSSF Scorecard) : voir
  [docs/SECURITE.md](docs/SECURITE.md).
- **Sommes SHA-256** publiées avec chaque version (`SHA256SUMS.txt`).
- **Provenance SLSA** jointe à chaque version (`Dawsend-X.Y.Z.intoto.jsonl`) : elle
  prouve que les fichiers sortent du workflow de release de ce dépôt.
- **Fuzzing** du lecteur de la mémoire partagée (ClusterFuzzLite).
