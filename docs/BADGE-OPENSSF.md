# Badge OpenSSF Best Practices (niveau « passing »)

Réponses préparées pour le formulaire de [bestpractices.dev](https://www.bestpractices.dev/),
critère par critère (identifiants du site). Les justifications sont en anglais, comme
le formulaire. À tenir à jour si le projet change, et à relire avant chaque nouvelle
demande de niveau.

**Inscription** : se connecter sur bestpractices.dev avec son compte GitHub, **Get
Your Badge Now**, choisir `Metaloul/Dawsend`. Le site remplit une partie des réponses
tout seul ; vérifier chacune avec ce tableau. Dès l'inscription, le projet passe « in
progress » (Scorecard : 2/10), puis « passing » quand tous les MUST sont remplis
(5/10). Ajouter ensuite le badge en tête du README, avec le numéro du projet :
`[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/<n>/badge)](https://www.bestpractices.dev/projects/<n>)`.

Légende : **Met**, **N/A**, **Unmet**. Les deux critères marqués « à confirmer » portent
sur les connaissances du mainteneur : lui seul peut y répondre.

`R` = `https://github.com/Metaloul/Dawsend`

## Basics

| Critère | Réponse | Justification |
| --- | --- | --- |
| description_good | Met | README, first paragraph: what Dawsend does (sends a DAW's ASIO audio to a regular Windows output). |
| interact | Met | README: Installation (download), Contributing (issues, pull requests). |
| contribution | Met | `R/blob/main/CONTRIBUTING.md` |
| contribution_requirements | Met | `R/blob/main/CONTRIBUTING.md#exigences` (fmt, clippy -D warnings, tests, cargo deny). |
| floss_license | Met | MIT (`R/blob/main/LICENSE`); the `.vst3` binary is GPL-3.0 because of vst3-sys. |
| floss_license_osi | Met | MIT and GPL-3.0 are OSI-approved. |
| license_location | Met | `R/blob/main/LICENSE` |
| documentation_basics | Met | README: requirements, installation, usage, troubleshooting. |
| documentation_interface | Met | README "Usage" (plugin, app, CLI tool); `docs/ARCHITECTURE.md` documents the shared-memory format. |
| sites_https | Met | GitHub only (HTTPS). |
| discussion | Met | GitHub issues and pull requests: searchable, addressable by URL, no proprietary software needed. |
| english | Met | README in English; issues accepted in English. |
| maintained | Met | Actively maintained (commits, releases, Dependabot updates). |

## Change control

| Critère | Réponse | Justification |
| --- | --- | --- |
| repo_public | Met | `R` |
| repo_track | Met | Git history on GitHub. |
| repo_interim | Met | Every change is committed to `main` between releases. |
| repo_distributed | Met | Git. |
| version_unique | Met | Version in `Cargo.toml`, one tag per release. |
| version_semver | Met | SemVer (`0.1.0`). |
| version_tags | Met | Git tags `vX.Y.Z`. |
| release_notes | Met | `R/blob/main/CHANGELOG.md`; each section becomes the GitHub release notes. |
| release_notes_vulns | N/A | No publicly known vulnerability has been fixed yet. |

## Reporting

| Critère | Réponse | Justification |
| --- | --- | --- |
| report_process | Met | `R/issues` (process in CONTRIBUTING.md). |
| report_tracker | Met | GitHub issues. |
| report_responses | Met | Bug reports are acknowledged (none left unanswered). |
| enhancement_responses | Met | Enhancement requests are answered in their issue. |
| report_archive | Met | `R/issues?q=is%3Aissue` (public, searchable). |
| vulnerability_report_process | Met | `R/blob/main/SECURITY.md` |
| vulnerability_report_private | Met | GitHub private vulnerability reporting: `R/security/advisories/new`. |
| vulnerability_report_response | Met | SECURITY.md commits to a first answer within 14 days. |

## Quality

| Critère | Réponse | Justification |
| --- | --- | --- |
| build | Met | `cargo build`, `cargo xtask bundle`, `installer/build.ps1` (README "Development"). |
| build_common_tools | Met | Cargo, pnpm, NSIS. |
| build_floss_tools | Met | Rust, Cargo, Node.js, pnpm, NSIS are all FLOSS. |
| test | Met | `cargo test --workspace` (unit and integration tests of the core, engine and app). |
| test_invocation | Met | Standard `cargo test`. |
| test_most | Met | Core (ring buffer, registry, layout), engine (drift, resampling, mixing) and app logic are tested; audio-device code is tested by hand (procedures in `docs/TEST-*.md`). |
| test_continuous_integration | Met | `R/actions/workflows/ci.yml` on every push and pull request. |
| test_policy | Met | CONTRIBUTING.md "Exigences": new features come with tests, bug fixes with a regression test. |
| tests_are_added | Met | Recent features added tests (e.g. drift regulator, shared-memory reader fuzz test). |
| tests_documented_added | Met | Stated in CONTRIBUTING.md ("Exigences"). |
| warnings | Met | `cargo clippy --all-targets -- -D warnings` in CI. |
| warnings_fixed | Met | CI fails on any warning. |
| warnings_strict | Met | Warnings are errors (`-D warnings`). |

## Security

| Critère | Réponse | Justification |
| --- | --- | --- |
| know_secure_design | Met (à confirmer) | Le mainteneur connaît les principes de conception sûre (moindre privilège, validation des entrées, surface minimale). |
| know_common_errors | Met (à confirmer) | Le mainteneur connaît les erreurs courantes de ce type de logiciel (lecture hors limites, données non validées, chaîne de mise à jour) et leurs parades. |
| crypto_published | Met | Updates are verified with minisign (Ed25519), a published algorithm. |
| crypto_call | Met | Verification is done by tauri-plugin-updater (minisign crate), not reimplemented. |
| crypto_floss | Met | minisign, rustls: FLOSS. |
| crypto_keylength | Met | Ed25519 (about 128-bit security). |
| crypto_working | Met | No broken algorithm used for security. |
| crypto_weaknesses | Met | SHA-256 for release checksums; no SHA-1 or MD5. |
| crypto_pfs | Met | HTTPS through rustls (TLS 1.2/1.3 with ECDHE only). |
| crypto_password_storage | N/A | Dawsend stores no passwords. |
| crypto_random | N/A | Dawsend generates no keys or nonces. |
| delivery_mitm | Met | Releases served over HTTPS by GitHub; updates signed (minisign); SLSA provenance. |
| delivery_unsigned | Met | `SHA256SUMS.txt` is retrieved over HTTPS and the installer is signature-checked, never a hash fetched over HTTP. |
| vulnerabilities_fixed_60_days | Met | No unpatched vulnerability of medium or higher severity (`cargo audit`, OSV). |
| vulnerabilities_critical_fixed | Met | Same. |
| no_leaked_credentials | Met | Signing key and tokens are GitHub secrets, never in the repository. |

## Analysis

| Critère | Réponse | Justification |
| --- | --- | --- |
| static_analysis | Met | CodeQL (Rust, JavaScript, Actions) and clippy on every change: `R/actions/workflows/security.yml`. |
| static_analysis_common_vulnerabilities | Met | CodeQL security queries; cargo-deny and cargo-audit for dependencies. |
| static_analysis_fixed | Met | CodeQL alerts are fixed; the only dismissed one (SLSA generator referenced by tag) is required by that generator. |
| static_analysis_often | Met | On every push and pull request. |
| dynamic_analysis | Met | ClusterFuzzLite fuzzes the shared-memory reader: `R/actions/workflows/fuzzing.yml`. |
| dynamic_analysis_unsafe | Met | Fuzzing runs with AddressSanitizer (Rust with `unsafe` code). |
| dynamic_analysis_enable_assertions | Met | Fuzz targets are built with `--debug-assertions` and assert on every invariant. |
| dynamic_analysis_fixed | Met | No open finding. |
