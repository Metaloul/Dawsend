# Étapes 2 à 4 — procédure de test

Dérive d'horloge (étape 2), plusieurs sources et interface du plugin (étape 3), appli
à fenêtre dans la zone de notification (étape 4), et installateur qui va avec.

## Ce qui a déjà été vérifié sur le PC de Romain (16/11/2026)

Windows 11, carte BEHRINGER UMC 404HD (sortie Windows « OUT 1-2 », 48 kHz), sans DAW :
les sources étaient publiées par `dawsend-console sine`.

| Vérification | Résultat |
|---|---|
| Dérive simulée de +400 ppm pendant 60 s, sortie capturée en loopback | 0 coupure, 0 resynchronisation, 0 discontinuité, 0 creux de 10 ms. La correction se cale vers +600 ppm : environ 200 ppm d'écart réel entre l'horloge du PC et celle de la Behringer, plus les 400 simulés |
| Tests automatiques (`cargo test --workspace`) | dérive de ±ppm à 44,1 et 96 kHz sur 240 s simulées, gros blocs de 2 048 trames, fondu croisé des resynchronisations, limiteur, réglages |
| Appli : deux sources (« Guitare » continue, « Voix » en bips) | cartes, états « Jouée », vumètres, volume général, marge et dérive affichées |
| Onglet Réglages | enregistrement dans `settings.json`, case « Journal détaillé » appliquée sans redémarrage |
| Langue | passage en anglais puis retour à « Celle du système » |
| Menu de la zone de notification | ouverture, sous-menu Sources avec les cases cochées |
| Installateur (`/S`) | fichiers posés, plugins VST3 et CLAP remplacés, valeur `Run` avec `--minimized` reconnue par la case « Lancer avec Windows » |
| Interface V2 de l'appli (17/11) | onglets Sources (tranches, boutons rotatifs, vumètres à segments, section Général), Réglages et état vide, vus avec des données simulées |
| Interface V2 du plugin (17/11) | vue en fenêtre autonome nih-plug hors DAW, l'appli ouverte : voyant vert « Relié », commutateur Envoi, état « Envoi coupé » |
| Démarrage `--minimized` puis second lancement | l'appli démarre sans fenêtre ; le second lancement affiche la fenêtre de la première instance |

## Ce qui reste à tester

### A. Plugin dans Studio One

1. Ferme Studio One, installe `Dawsend-Setup-0.1.0.exe`, rouvre Studio One.
2. Mets Dawsend en insert sur la piste guitare et **ouvre le plugin** :
   - le logo et le nom Dawsend s'affichent dans le bandeau du haut ;
   - le voyant passe au **vert** « Relié — l'appli joue cette piste » quand l'appli
     tourne et que le moteur audio est démarré ;
   - quitte l'appli (clic droit sur l'icône, *Quitter Dawsend*) : le voyant passe au
     **rouge** « Appli Dawsend fermée » ; relance l'appli : il repasse au vert tout
     seul ;
   - dans l'appli, éteins l'interrupteur de la source : voyant **orange** « Appli
     ouverte — piste non jouée » ; coupe-la (Couper) : « piste coupée dans l'appli » ;
   - le vumètre bouge quand tu joues ;
   - clique sur le commutateur **ENVOI** : la bague orange s'éteint, le voyant
     passe au gris « Envoi coupé » et la carte de la source ne bouge plus dans
     l'appli ; le son de la piste, lui, ne change pas. Reclique pour réactiver.
   - Dans l'appli, essaie le bouton rotatif d'une source : glisser vers le haut ou
     le bas (Maj pour un réglage fin), molette, double-clic pour revenir à 0 dB.
3. Renomme la source dans le plugin (« Guitare »), valide par Entrée : le nom change
   dans l'appli. Enregistre le projet, ferme-le, rouvre-le : le nom est gardé.
4. Mets un second Dawsend sur une autre piste (« Voix ») : deux cartes dans l'appli,
   volumes et coupures indépendants.
5. Ferme la fenêtre du plugin et rouvre-la plusieurs fois, change de projet : Studio
   One ne doit jamais se figer.

### B. Dérive en conditions réelles

Laisse tourner une session de 30 minutes avec Discord en partage d'écran entier.
Sous la carte de la source : la dérive se stabilise (quelques centaines de ppm au
plus) et le compteur « coupures » reste à 0. Si tu changes la taille du buffer ASIO
(256, 512, 1 024), la marge affichée monte toute seule au-dessus de 10 ms.

### C. Appli et installateur

- Après un redémarrage du PC, Dawsend est dans la zone de notification sans
  fenêtre.
- Menu de l'icône : *Couper le son*, choix d'une autre *Sortie*, puis retour à la
  sortie par défaut ; débranche la carte son : l'appli repasse sur la sortie par
  défaut, puis revient quand la carte est rebranchée.
- Réglages : *Ouvrir le dossier des journaux* ouvre
  `%LOCALAPPDATA%\com.metaloul.dawsend\logs`.
- Désinstalle depuis *Paramètres › Applications* : plus de plugin, plus de valeur
  `Run`, l'appli fermée.

### D. Vumètres et mises à jour

- Vumètres de l'appli : avec `dawsend-console sine --name Test`, ils bougent aussi
  souplement que ceux du plugin (lecture à chaque image, retombée de 24 dB/s).
- Discord : partage d'une **fenêtre** (Studio One) puis de l'**écran entier**, avec le
  son. Noter ce que les amis entendent dans chaque cas (voir
  [EVOLUTIONS.md § 1](EVOLUTIONS.md#1-à-vérifier-en-premier--ce-que-discord-capte)).
- Mises à jour, une fois une première release publiée avec la clé de signature :
  1. installer une version plus ancienne (par exemple 0.1.0) ;
  2. publier la suivante (tag `vX.Y.Z`) ; dans l'appli, **Réglages › Mises à jour ›
     Vérifier** : le bandeau annonce la version et ses notes ;
  3. avec Studio One ouvert et le plugin sur une piste, **Installer** est refusé
     (« Ferme ton DAW ») ;
  4. DAW fermé, **Installer** : téléchargement, l'appli se ferme, l'installateur
     passe sans question, l'appli revient seule, réglages et choix
     d'installation (raccourci du bureau, VST3, CLAP) gardés ;
  5. journal : lignes `mises à jour :` (vérification, téléchargement, signature
     vérifiée).
- Signature refusée : remplacer l'installateur de la release par un autre fichier
  (sans changer `latest.json`) : l'appli refuse (« signature ») et rien n'est lancé.

## Limites connues

- Installateur et binaires non signés : avertissement SmartScreen.
- `installer/build.sh` (compilation croisée depuis Linux) n'a pas été relancé depuis
  l'ajout de l'appli Tauri ; la construction sous Windows (`build.ps1`) est celle qui
  a été testée.
- Étape 6 (Linux, macOS) pas commencée.
