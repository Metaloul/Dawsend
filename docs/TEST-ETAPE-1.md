# Étape 1 — procédure de test

But : vérifier que le son d'une piste de Studio One arrive sur une sortie Windows
normale, entendue par le partage d'écran, et que rien ne plante quel que soit
l'ordre de démarrage.

Installation : lance `Dawsend-Setup-0.1.0.exe` (voir le [README](../README.md)).

**Nouveau :** prise en charge des hautes fréquences (96, 192 kHz…), avec un filtre
anti-repliement quand le projet est plus rapide que la sortie Windows.

## Ce que j'ai déjà vérifié de mon côté (Linux, sans carte son)

- 23 tests automatiques. Ils couvrent le format de la mémoire partagée, la réservation
  et la libération des emplacements, l'écriture et la lecture concurrentes sur deux
  mappings, la détection des processus morts, le rééchantillonnage 44,1 → 48 kHz et
  les underruns.
- Un mini-hôte CLAP qui charge `Dawsend.clap` compilé. Résultats :
  - la source « Dawsend 1 » est publiée à l'activation ;
  - le son ressort **identique** (pass-through) ;
  - 51 200 trames sont copiées sans erreur dans la mémoire partagée ;
  - la réactivation à 44,1 kHz réutilise le même emplacement ;
  - l'emplacement est libéré à la destruction.
- Une lecture de 10 s entre deux processus, avec une sortie simulée cadencée à 48 kHz :
  aucun underrun, à 48 kHz comme en 44,1 kHz.
- Tout le workspace compile pour Windows (`x86_64-pc-windows-msvc`).
- Hautes fréquences : 8 combinaisons testées, de 44,1 à 384 kHz et dans les deux sens.
  Aucun underrun, niveau conservé, et un 40 kHz ultrasonique n'est pas replié dans
  l'audible. Une lecture réelle de 6 s à 96 et à 192 kHz passe sans incident.
- **Installateur, testé sous Wine** (Windows simulé sous Linux) :
  - l'installation silencieuse place les fichiers, les plugins, les raccourcis et la
    clé de désinstallation ;
  - `dawsend-console.exe` voit une source publiée par un autre processus Windows ;
  - le `Dawsend.clap` **Windows installé**, chargé dans un hôte, laisse passer le son
    à l'identique et copie exactement l'audio dans la mémoire partagée ;
  - la désinstallation enlève tout.

Ce que je **ne peux pas** tester : WASAPI, Studio One, Discord. C'est ton rôle.

## Tests à faire

Coche au fur et à mesure et note ce qui cloche.

### A. L'appli seule (sans DAW)

1. Terminal 1 : `dawsend-console sine` → affiche « Source de test publiée… ».
2. Terminal 2 : `dawsend-console play` → tu entends un bip (0,5 s sur 1 s) dans la
   sortie Windows.
3. Le terminal 1 affiche « l'appli écoute cette source ».
4. `dawsend-console devices`, puis `play --device "<un autre>"` → le son change bien
   de sortie.

### B. Studio One

1. Plugin Dawsend en insert sur la piste guitare, moteur audio actif.
2. `dawsend-console list` → « Dawsend 1 », avec la bonne fréquence, `active`.
3. `dawsend-console play` → la guitare sort sur la sortie Windows.
   - Le son de Studio One (ASIO) est-il **inchangé** ?
   - Note la valeur « buffer » et les compteurs pendant 2 minutes de jeu.
4. Discord : partage de l'**écran entier** avec le son → un ami entend-il la guitare ?
   Essaie aussi le partage de la **fenêtre** Studio One et dis-moi ce qui se passe.
5. Est-ce que tu entends la guitare en double (avec un petit écho) dans ton casque ?
   Dis-moi comment tes sorties sont branchées : interface ASIO, sortie Windows par
   défaut, casque…
6. Hautes fréquences : passe le projet à 96 kHz puis 192 kHz → le son reste propre,
   et la console indique « filtre anti-repliement actif » si Windows est à 48 kHz.

### C. Robustesse

1. Lance la console **avant** Studio One, puis ouvre le projet → la lecture démarre
   seule.
2. Ferme Studio One pendant la lecture → la console repasse en attente, sans planter.
3. Ferme la console (Ctrl+C) pendant que Studio One joue → aucun craquement, aucun
   blocage dans Studio One.
4. Deux instances du plugin (deux pistes) → `list` affiche « Dawsend 1 » et
   « Dawsend 2 ». `play --source "Dawsend 2"` joue la seconde.
5. Change la fréquence du projet (48 → 44,1 kHz) pendant la lecture → la console
   reprend à la nouvelle fréquence.
6. Sauvegarde, ferme, rouvre le projet → les noms des sources sont conservés.

### D. Mesures pour l'étape 2

1. Taille du buffer ASIO et fréquence du projet ?
2. Fréquence de la sortie Windows (affichée par la console au démarrage) ?
3. En laissant tourner 10 min : combien de « resynchros » et d'underruns ? Ça mesure
   la dérive d'horloge que l'étape 2 doit compenser.

## Limites connues de l'étape 1

- Rééchantillonnage linéaire à rapport fixe, sans compensation de dérive : une petite
  coupure toutes les quelques minutes est attendue (corrigé à l'étape 2 avec rubato).
- La marge (`--latency`) doit dépasser la taille du buffer ASIO.
- Une seule source jouée à la fois (mixage à l'étape 3).
- Pas d'interface dans le plugin : nom automatique « Dawsend N » (étape 3).
