# Dawsend

Dawsend envoie le son de ton DAW (en ASIO) vers une sortie Windows normale, pour que
le partage d'écran (Discord, etc.) l'entende, sans OBS ni câble audio virtuel.

- **Plugin Dawsend** (VST3 et CLAP) : à placer en insert sur une piste ou sur le master.
  Il laisse passer le son sans le modifier et en copie une version vers l'appli.
- **Appli Dawsend** : lit les sources publiées par les plugins et les joue sur une
  sortie Windows normale (WASAPI).

> **État : étape 1 (prototype).** Pour l'instant, l'appli est une console et ne joue
> qu'une source à la fois, sans compensation de dérive d'horloge. Voir
> [la procédure de test de l'étape 1](docs/TEST-ETAPE-1.md),
> [le cahier des charges](docs/CAHIER_DES_CHARGES.md) et
> [l'architecture](docs/ARCHITECTURE.md).

## Installation (le plus simple)

Lance **`Dawsend-Setup-0.1.0.exe`**. Il installe :

- le plugin **VST3** dans `C:\Program Files\Common Files\VST3` ;
- le plugin **CLAP** dans `C:\Program Files\Common Files\CLAP` ;
- l'appli dans `C:\Program Files\Dawsend`, avec ses raccourcis dans le menu Démarrer
  (« Dawsend », « liste des sources », « bip de test », mode d'emploi, désinstallation) ;
- en option : un raccourci sur le bureau et le lancement au démarrage de Windows.

Ferme Studio One avant d'installer ou de mettre à jour. L'installateur n'est pas signé :
si Windows affiche « Windows a protégé votre ordinateur », clique sur *Informations
complémentaires* puis *Exécuter quand même*. On désinstalle depuis *Paramètres ›
Applications* ou depuis le menu Démarrer.

### Construire l'installateur

- **Sous Linux** (c'est ainsi qu'est produit l'installateur fourni) :
  `sudo apt install mingw-w64 nsis`, `rustup target add x86_64-pc-windows-gnu`, puis
  `installer/build.sh`.
- **Sous Windows** : installe en plus [NSIS 3](https://nsis.sourceforge.io), puis
  `powershell -ExecutionPolicy Bypass -File installer\build.ps1`.

Résultat : `target/installer/Dawsend-Setup-<version>.exe`.

## Compiler sous Windows (sans installateur)

### Prérequis (une seule fois)

1. **Visual Studio Build Tools** : <https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/>.
   Dans l'installateur, coche la charge de travail **« Développement Desktop en C++ »**.
2. **Rust** : <https://rustup.rs> (télécharger `rustup-init.exe`, choix par défaut).
3. **Git** : <https://git-scm.com/download/win>. Cargo en a besoin pour récupérer
   nih-plug, qui n'est pas publié sur crates.io.

Ouvre un nouveau terminal (PowerShell) et vérifie : `cargo --version`.

### Compilation

Depuis le dossier du projet :

```powershell
# Plugin VST3 + CLAP  →  target\bundled\Dawsend.vst3 et target\bundled\Dawsend.clap
cargo xtask bundle dawsend-plugin --release

# Appli console      →  target\release\dawsend-console.exe
cargo build --release -p dawsend-app --bin dawsend-console
```

La première compilation prend quelques minutes, les suivantes sont rapides.

### Installation du plugin

Copie le plugin dans le dossier standard (PowerShell **en administrateur**) :

```powershell
Copy-Item -Recurse -Force target\bundled\Dawsend.vst3 "C:\Program Files\Common Files\VST3\"
New-Item -ItemType Directory -Force "C:\Program Files\Common Files\CLAP" | Out-Null
Copy-Item -Force target\bundled\Dawsend.clap "C:\Program Files\Common Files\CLAP\"
```

Pense à fermer Studio One avant de remplacer le plugin : Windows refuse d'écraser un
plugin en cours d'utilisation.

## Utilisation dans Studio One

1. Relance Studio One. Si le plugin n'apparaît pas : *Studio One › Options ›
   Emplacements › Plug-ins VST* → *Réinitialiser la liste noire*, puis relance.
2. Ajoute **Dawsend** (catégorie *Fx / Tools*) en insert sur la piste guitare,
   **en dernier** dans la chaîne si tu veux envoyer le son traité.
3. Lance l'appli :

   ```powershell
   target\release\dawsend-console.exe play
   ```

   La console affiche la source jouée et, chaque seconde, le remplissage du buffer
   et les compteurs d'incidents.
4. Dans Discord, partage **l'écran entier** avec le son (voir le dépannage).

### Commandes de la console

| Commande | Rôle |
|---|---|
| `dawsend-console play` | joue la première source trouvée sur la sortie Windows par défaut |
| `dawsend-console play --source "Dawsend 2"` | joue une source précise |
| `dawsend-console play --device "Haut-parleurs"` | choisit la sortie (une partie du nom suffit) |
| `dawsend-console play --latency 20` | marge du buffer en ms (10 par défaut) |
| `dawsend-console list` | liste les sources publiées par les plugins |
| `dawsend-console devices` | liste les sorties audio |
| `dawsend-console sine` | publie un bip de test, comme le ferait un plugin (pour tester sans DAW) |
| `dawsend-console sine --rate 192000` | idem, à une autre fréquence |

### Fréquences d'échantillonnage

Toutes les fréquences de 8 kHz à 768 kHz sont prises en charge (44,1, 48, 88,2, 96,
176,4, 192 kHz…), côté DAW comme côté Windows, dans n'importe quelle combinaison.
Quand le projet tourne plus vite que la sortie Windows (par exemple 192 kHz → 48 kHz),
un filtre anti-repliement est appliqué, et la console l'indique.

À l'étape 1, chaque plugin s'appelle « Dawsend 1 », « Dawsend 2 »… selon l'ordre
d'ajout. Le nom est sauvegardé avec le projet. Le renommage depuis l'interface du
plugin arrive à l'étape 3.

## Dépannage

| Problème | Piste |
|---|---|
| `list` n'affiche aucune source | Vérifie que le plugin est bien inséré sur une piste **active** et que le moteur audio de Studio One tourne. |
| La source est « en pause » | Studio One ne fait pas tourner le plugin (piste désactivée, moteur audio arrêté). |
| Discord n'entend rien | Discord ne capture le son que de l'appli partagée quand on partage une **fenêtre**. Or le son de Dawsend vient de l'appli Dawsend, pas de Studio One : partage **l'écran entier**. |
| Tu entends la guitare en double, avec un léger écho | Le son ASIO direct et la sortie Windows arrivent dans le même casque. Choisis avec `--device` une sortie que tu n'écoutes pas, ou baisse son volume (à tester, voir la procédure). |
| Craquements, « underruns » qui augmentent | Augmente la marge : `--latency 20`. Elle doit dépasser la taille du buffer ASIO (par exemple 1 024 échantillons à 48 kHz = 21 ms). |
| Coupure régulière toutes les quelques minutes | C'est la dérive d'horloge entre la carte ASIO et la sortie Windows : normal à l'étape 1, l'étape 2 la corrige. |
| « mémoire partagée incompatible » | Le plugin et l'appli ne sont pas de la même version : recompile et réinstalle les deux. |

## Licences

- Le code source de Dawsend est sous licence **MIT** : voir [LICENSE](LICENSE).
  Copyright Romain Rousseau (Metaloul).
- Le **binaire VST3** (`Dawsend.vst3`) inclut les bindings `vst3-sys`, sous GPLv3.
  Ce binaire est donc distribué selon les termes de la **GPLv3**. Le binaire **CLAP**
  n'a pas cette contrainte. Détails : [docs/ARCHITECTURE.md § 7](docs/ARCHITECTURE.md#7-licences).
- VST est une marque de Steinberg Media Technologies GmbH.
