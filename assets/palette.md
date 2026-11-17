# Dawsend — palette et typographie

Source : fichier Figma « Dawsend — identité », pages « Identité » (logo) et
« V2 — Interface » (appli et plugin).

## Couleurs

| Nom      | Hex       | Rôle                                                               |
|----------|-----------|--------------------------------------------------------------------|
| Encre    | `#121418` | Fond sombre (icône, bandeau du plugin), texte sur fond clair        |
| Graphite | `#2A2E36` | Surfaces et panneaux du plugin, filets, bordures                    |
| Papier   | `#F3EFE8` | Fond clair, texte et support du symbole sur fond sombre             |
| Ambre    | `#FF7A2F` | Signal (l'onde du symbole), accent sur fond sombre                  |
| Braise   | `#E0601F` | Accent sur fond clair, icône de la zone de notification             |

Contrastes utiles (WCAG) :

| Paire                                   | Rapport |
|-----------------------------------------|---------|
| Papier sur Encre                        | 16,1:1  |
| Ambre sur Encre                         | 7,1:1   |
| Braise sur Papier                       | 3,1:1   |
| Braise sur barre des tâches sombre `#202020` | 4,6:1 |
| Braise sur barre des tâches claire `#F3F3F3` | 3,2:1 |

L'Ambre n'est pas employé sur fond clair (2,3:1 sur Papier) : on y passe à la Braise.

## Symbole

- Grille de 48 × 48, trait de 6, extrémités et jonctions rondes.
- Un crochet (le support d'insert) d'où sort une onde qui part vers la droite.
- Sur fond sombre : crochet Papier, onde Ambre. Sur fond clair : crochet Encre, onde Braise.
- Zone de notification : symbole d'une seule couleur (Braise), lisible sur les barres des tâches claire et sombre.
- Icône d'appli : fond Encre plein, coins arrondis à 22,5 % du côté, symbole à l'échelle 16 (marge de 120 à 128 px sur 1024).

## Logotype

- Police : **Familjen Grotesk Bold**, approche −1 %.
- Licence : **SIL Open Font License 1.1** (Familjen STHLM AB, diffusée sur Google Fonts). L'OFL autorise l'usage dans un logotype ; les fichiers `wordmark*.svg` contiennent les lettres vectorisées en chemins et ne dépendent d'aucune police installée.
- Composition symbole + nom : nom centré sur la hauteur du symbole, écart égal à 0,38 × la hauteur du nom.

## Fichiers

| Fichier               | Contenu                                                         | Dimensions  |
|-----------------------|-----------------------------------------------------------------|-------------|
| `logo.svg`            | Icône d'appli (fond Encre, coins arrondis)                      | 1024 × 1024 |
| `logo-1024.png`       | Icône d'appli, coins transparents                                | 1024 × 1024 |
| `logo-mark.svg`       | Symbole seul sans fond, version pour fond sombre                | 48 × 48     |
| `tray-32.png`         | Icône de la zone de notification (Braise), fond transparent     | 32 × 32     |
| `wordmark.svg`        | Logotype pour fond sombre (Papier)                               | 800 × 136   |
| `wordmark-light.svg`  | Logotype pour fond clair (Encre)                                 | 800 × 136   |
| `plugin-header.png`   | Symbole et logotype du plugin, fond transparent (104,5 × 20 à l'échelle 3) | 314 × 60 |
| `fonts/`              | Familjen Grotesk (normal, demi-gras) et Geist Mono, intégrées au plugin, avec leurs licences OFL | — |

## Interface (V2)

Inspirée des façades d'appareils (Neural DSP, FabFilter) : fond sombre, commandes
en relief, voyants et vumètres à segments, étiquettes en petites capitales.

| Jeton    | Hex       | Rôle                                                      |
|----------|-----------|-----------------------------------------------------------|
| `ink`    | `#121418` | Fond de la fenêtre                                        |
| `face`   | `#181B20` | Façades : bandeau, tranches, groupes de réglages          |
| `well`   | `#0C0E11` | Creux : champs, listes, barre du bas, fond des boutons    |
| `ctrl`   | `#2A2E36` | Boutons, curseurs, onglet choisi                          |
| `line`   | `#262A31` | Filets et bordures                                        |
| `unlit`  | `#1E2228` | Segment de vumètre éteint                                 |
| `paper`  | `#F3EFE8` | Texte principal                                           |
| `muted`  | `#9A9FAA` | Texte secondaire, étiquettes                              |
| `dim`    | `#6B7280` | Voyant au repos, vumètre d'une source coupée              |
| `amber`  | `#FF7A2F` | Accent : arc des boutons rotatifs, bague du commutateur   |
| `ok`     | `#3DD68C` | Voyant « tout va bien », vumètre jusqu'à −9 dBFS          |
| `warn`   | `#F2B83B` | Voyant d'attention, vumètre de −9 à −1 dBFS               |
| `bad`    | `#F25555` | Voyant d'erreur, coupure, vumètre au-dessus de −1 dBFS    |

- **Polices** : Familjen Grotesk (textes, 13 px de base, demi-gras pour les titres) et
  Geist Mono (valeurs chiffrées : dB, ms, kHz, version). Licence OFL 1.1.
- **Étiquettes** : 10 px, demi-gras, capitales, espacement de 0,08 em.
- **Bouton rotatif** : course de 270°, de −40 dB (« −∞ ») à +6 dB, pas de 0,5 dB ;
  glisser verticalement (Maj : réglage fin), molette, flèches, double-clic pour 0 dB.
- **Vumètre** : segments de 3 px au pas de 4 px, de −60 à 0 dBFS, retombée de 24 dB/s.
- **Voyant** : disque de 10 px avec un halo de sa couleur.
