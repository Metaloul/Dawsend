# Licences des binaires distribués

- **Code source de Dawsend** : MIT, voir [`../LICENSE`](../LICENSE).
- **`Dawsend.vst3`** : ce binaire inclut les bindings `vst3-sys` (GPLv3). Il est donc
  distribué selon les termes de la GPLv3, voir [`GPL-3.0.txt`](GPL-3.0.txt). Le code
  source correspondant est celui de ce dépôt.
- **`Dawsend.clap`** : MIT.
- **`dawsend.exe`** et **`dawsend-console.exe`** incluent `audio_thread_priority`
  (Mozilla, MPL-2.0), utilisé sans modification pour la priorité temps réel du thread
  de sortie. Son code source : <https://github.com/mozilla/audio_thread_priority>.
- **Polices de l'interface** : Familjen Grotesk et Geist Mono, sous SIL Open Font
  License 1.1, voir [`OFL-1.1.txt`](OFL-1.1.txt). Elles sont intégrées à `dawsend.exe`
  (interface de l'appli) et aux deux binaires du plugin, sans modification. Fichiers
  d'origine : `assets/fonts/` et les paquets `@fontsource/familjen-grotesk` et
  `@fontsource/geist-mono`.
- **Marque GitHub** du lien vers le projet (appli et plugin) : dessin de
  [Primer Octicons](https://github.com/primer/octicons) (`mark-github`), sous licence
  MIT, Copyright GitHub Inc. Le logo GitHub est une marque de GitHub Inc., utilisée
  ici seulement pour renvoyer vers la page du projet. Fichier : `assets/github-mark.png`.
