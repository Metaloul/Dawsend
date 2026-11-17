# Sécurité et conformité

Ce document dit ce que fait Dawsend sur ta machine, ce qu'il ne fait pas, et comment le
code, les dépendances et les fichiers publiés sont contrôlés. Pour signaler une faille :
[SECURITY.md](../SECURITY.md).

## Ce que Dawsend fait sur ta machine

| Question | Réponse |
| --- | --- |
| Droits administrateur ? | Seulement pendant l'installation (copie dans `Program Files` et dans le dossier VST3/CLAP commun). L'appli tourne ensuite avec tes droits normaux. |
| Réseau ? | Aucun port ouvert, aucune écoute. Une seule connexion sortante : la lecture de `latest.json` sur `github.com` pour les mises à jour (au démarrage puis toutes les 6 h), désactivable dans les réglages. Ni télémétrie, ni statistiques, ni compte. |
| Comment le plugin parle à l'appli ? | Par une mémoire partagée locale (`Local\Dawsend.v1`), visible des seuls programmes de ta session Windows. Le format est décrit dans [ARCHITECTURE.md](ARCHITECTURE.md) ; l'appli vérifie tout ce qu'elle y lit (version, tailles, positions, valeurs) avant de s'en servir. |
| Ce qui est écrit sur le disque ? | Les réglages (`%APPDATA%\com.metaloul.dawsend`), les journaux (`%LOCALAPPDATA%\com.metaloul.dawsend\logs`) et, si tu l'as coché, une entrée de démarrage automatique. Rien d'autre. |
| Micro, caméra, écran ? | Rien de tout ça. Dawsend ne lit que le son que le plugin lui envoie depuis ton DAW. |

## Mises à jour

- L'appli lit `https://github.com/Metaloul/Dawsend/releases/latest/download/latest.json`.
- L'installateur est signé avec une clé [minisign](https://jedisct1.github.io/minisign/)
  propre à Dawsend. La clé publique est intégrée au programme. L'appli refuse tout
  fichier dont la signature ne correspond pas, et toute signature faite pour une autre
  version que celle annoncée (protection contre le retour à une ancienne version).
- La clé privée n'est pas dans le dépôt : elle vit chez l'auteur et dans les secrets
  GitHub du workflow de release.
- Rien ne s'installe tout seul : l'appli prévient, et c'est toi qui cliques sur
  **Installer**. L'installation est refusée tant qu'un DAW a le plugin chargé.
- Chaque release publie aussi `SHA256SUMS.txt` pour vérifier un téléchargement à la
  main :

  ```powershell
  Get-FileHash .\Dawsend-Setup-0.1.0.exe -Algorithm SHA256
  ```

## Contrôles automatiques (GitHub Actions)

| Outil | Ce qu'il vérifie | Quand |
| --- | --- | --- |
| [CodeQL](https://codeql.github.com/) | Analyse statique du code Rust, du JavaScript/Svelte de l'interface et des workflows GitHub | À chaque push, chaque PR, et chaque semaine |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Failles connues (base RustSec), licences des dépendances, provenance (crates.io ou dépôts Git autorisés) | À chaque push et chaque PR |
| [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) | Failles connues dans `Cargo.lock` | À chaque push et chaque PR |
| `pnpm audit` | Failles connues dans les dépendances de l'interface | À chaque push et chaque PR |
| [OpenSSF Scorecard](https://scorecard.dev/) | Bonnes pratiques du dépôt (protection de branche, workflows, dépendances épinglées…) | Chaque semaine |
| [Dependabot](https://docs.github.com/code-security/dependabot) | Propose les mises à jour des dépendances Rust, npm et des actions GitHub | Chaque semaine |
| `cargo clippy -D warnings` | Lints Rust, aucun avertissement toléré | À chaque push et chaque PR |
| [VirusTotal](https://www.virustotal.com/) | Analyse de l'installateur publié par environ 70 antivirus | À chaque release (si la clé d'API est configurée) |

Les règles de cargo-deny sont dans [`deny.toml`](../deny.toml) ; les workflows dans
[`.github/workflows/`](../.github/workflows/).

## Résultats au 17 novembre 2026 (version 0.1.0)

### Dépendances

| Contrôle | Résultat |
| --- | --- |
| `cargo deny check` (cargo-deny 0.21) | `advisories ok, bans ok, licenses ok, sources ok` |
| `cargo audit` | **Aucune faille connue.** 8 avertissements sur des dépendances indirectes, détaillés ci-dessous |
| `pnpm audit --prod` | `No known vulnerabilities found` |

Avertissements restants, tous sur des crates tirés indirectement par nos dépendances
(on ne les choisit pas, ils partiront quand celles-ci les remplaceront) :

| Crate | Avis | Nature | Tiré par | Portée réelle |
| --- | --- | --- | --- | --- |
| `atty` | [RUSTSEC-2024-0375](https://rustsec.org/advisories/RUSTSEC-2024-0375) | Plus maintenu | `nih_log` (nih-plug) | Sert une fois au chargement du plugin à savoir si la console est un terminal |
| `atty` | [RUSTSEC-2021-0145](https://rustsec.org/advisories/RUSTSEC-2021-0145) | Lecture potentiellement non alignée | `nih_log` (nih-plug) | Seulement avec un allocateur global non standard, ce que ni Dawsend ni un DAW n'imposent au plugin |
| `ttf-parser` | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192) | Plus maintenu | `ab_glyph` (egui) | Ne lit que les polices intégrées au plugin |
| `proc-macro-error` | [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370) | Plus maintenu | `glib` (Tauri sous Linux) | Utilisé seulement à la compilation, absent du programme Windows |
| `glib` | [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429) | `VariantStrIter` incorrect | GTK (Tauri sous Linux) | Absent du programme Windows |
| `zvariant`, `zvariant_derive`, `zbus_names` | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370), [-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371), [-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372) | Plus maintenus | `zbus` (D-Bus, plugins Tauri sous Linux) | Absents du programme Windows |

### Licences

Toutes les dépendances ont une licence de la liste autorisée de `deny.toml` (MIT,
Apache-2.0, BSD, ISC, Zlib, Unicode, MPL-2.0, OFL-1.1 pour les polices, GPL-3.0 pour
`vst3-sys`). Le récapitulatif est dans [LICENSES/README.md](../LICENSES/README.md).

### Antivirus

Fichiers analysés avec Microsoft Defender (`MpCmdRun -Scan -ScanType 3`, signatures
1.463.338.0 du 17/11/2026) : **aucune menace détectée**.

| Fichier | Taille | SHA-256 |
| --- | --- | --- |
| `dawsend.exe` | 13,6 Mo | voir `SHA256SUMS.txt` de la release |
| `dawsend-console.exe` | 0,7 Mo | voir `SHA256SUMS.txt` de la release |
| `Dawsend.vst3` / `Dawsend.clap` | 3,8 Mo | voir `SHA256SUMS.txt` de la release |
| `Dawsend-Setup-0.1.0.exe` | 5,1 Mo | voir `SHA256SUMS.txt` de la release |

Les sommes changent à chaque compilation : celles qui font foi sont celles publiées
avec la release, calculées par le workflow sur les fichiers mêmes qu'il publie. Le
rapport VirusTotal de chaque installateur publié est lié dans les notes de la release.

### Pourquoi un antivirus peut quand même râler

L'installateur et les programmes ne sont **pas encore signés** avec un certificat de
signature de code (Authenticode). Windows SmartScreen peut donc afficher « Windows a
protégé votre ordinateur » au premier lancement : clique sur **Informations
complémentaires**, puis **Exécuter quand même**. Certains antivirus peu répandus
signalent aussi par prudence tout installateur NSIS non signé (faux positif
« générique »). La signature du code est prévue ; en attendant, compare la somme
SHA-256 et le rapport VirusTotal de la release.

## Refaire les contrôles chez toi

```sh
cargo install cargo-deny cargo-audit --locked
cargo deny check
cargo audit
pnpm --dir ui audit --prod
```

```powershell
& "$env:ProgramFiles\Windows Defender\MpCmdRun.exe" -Scan -ScanType 3 -File "$PWD\target\installer\Dawsend-Setup-0.1.0.exe"
```
