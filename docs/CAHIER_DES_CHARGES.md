# Cahier des charges — Dawsend

> Copie du cahier des charges fourni par Romain (Metaloul) le 16/11/2026.
> Ce fichier sert de mémoire au projet : toute session de travail future repart d'ici
> et de [`ARCHITECTURE.md`](ARCHITECTURE.md).

Nom : jeu de mots entre « Dawson » et « DAW » + « send ». Licence MIT, « Copyright Romain Rousseau (Metaloul) ».

## Le problème
Ma carte son tourne en ASIO avec Studio One. Le son ASIO contourne le mixeur Windows, donc quand je partage mon écran (Discord, etc.), mes amis n'entendent pas ma guitare. Aujourd'hui je bricole avec ReaStream + OBS + Virtual Audio Cable. Je veux remplacer tout ça par une solution légère, sans OBS ni câble virtuel.

## La solution voulue
Deux composants en Rust :

1. **Plugin Dawsend** (VST3 + CLAP, avec nih-plug)
   - Se place en insert sur une piste ou sur le master.
   - Laisse passer le son sans le modifier (pass-through).
   - Copie le son dans une mémoire partagée vers l'appli.
   - Plusieurs instances possibles, chacune avec un nom de source (ex. « Guitare », « Master »).
   - Thread audio temps réel strict : aucune allocation, aucun verrou, aucun appel système bloquant. Ring buffer SPSC lock-free.
   - Petite interface : nom de la source, état de connexion à l'appli, vumètre.

2. **Appli Dawsend** (tourne en fond, icône dans la barre des tâches, interface minimisable)
   - Lit les sources publiées par les plugins.
   - Joue le son sur une sortie Windows normale (WASAPI partagé, via cpal), que le partage d'écran capte directement.
   - Interface : liste des sources, volume et mute par source, choix du périphérique de sortie, vumètres, latence cible réglable, démarrage avec Windows en option.
   - Interface en Tauri 2 + Svelte, le plus léger possible.

## Points techniques importants
- **Dérive d'horloge** : la carte ASIO et la sortie WASAPI n'ont pas la même horloge. Il faut un rééchantillonnage adaptatif (ex. rubato) piloté par le niveau de remplissage du buffer, pour éviter craquements et décalage qui grossit.
- **Fréquences différentes** : gérer le cas où le DAW et la sortie Windows n'ont pas la même fréquence d'échantillonnage.
- **Latence** : viser 20 à 30 ms au total, réglable. Gérer proprement les underruns (silence plutôt que craquement).
- **Robustesse** : l'appli peut démarrer avant ou après le DAW ; le plugin ne doit jamais bloquer ni planter le DAW si l'appli est absente. Heartbeat pour détecter une source morte.
- **Performance** : CPU quasi nul au repos.
- Vérifier les licences (bindings VST3 utilisés par nih-plug, SDK VST3) et me signaler toute contrainte.

## Plateformes
1. Windows d'abord (cible principale).
2. Linux ensuite : PipeWire (idéalement, l'appli crée sa propre sortie ou un nœud nommé).
3. macOS en dernier : CoreAudio.

Structurer le code pour que le backend audio et l'IPC soient isolés par plateforme dès le départ.

## Organisation
- Workspace Cargo : `dawsend-plugin`, `dawsend-app`, `dawsend-core` (format du buffer partagé, protocole, utilitaires communs).
- Avancer par étapes, avec un test par Romain à chaque fois :
  1. Prototype : plugin qui écrit dans la mémoire partagée + appli console qui la lit et la joue en WASAPI.
  2. Compensation de dérive et gestion de la latence.
  3. Plusieurs sources et mixage.
  4. Interface Tauri, barre des tâches, réglages sauvegardés.
  5. Empaquetage (installeur Windows, plugin copié dans le dossier VST3).
  6. Linux, puis macOS.
- Commencer par proposer l'architecture détaillée et le format de la mémoire partagée avant de coder.
- README clair : installation, utilisation dans Studio One, dépannage.

## Suivi des étapes

| Étape | État |
|---|---|
| 0. Architecture et format de la mémoire partagée | Proposée, **en attente de validation** |
| 1. Prototype plugin + appli console | À faire |
| 2. Dérive d'horloge et latence | À faire |
| 3. Plusieurs sources et mixage | À faire |
| 4. Interface Tauri, barre des tâches, réglages | À faire |
| 5. Empaquetage Windows | À faire |
| 6. Linux, puis macOS | À faire |
