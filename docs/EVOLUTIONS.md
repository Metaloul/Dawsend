# Évolutions possibles

Pistes pour rendre Dawsend plus modulaire et plus extensible **sans perdre ce qui fait
son intérêt** : on installe, on met le plugin sur une piste, et ça marche.

> **Règle de conduite** : chaque extension reste invisible pour qui ne s'en sert pas.
> Mêmes réglages par défaut, aucune étape de plus à l'installation, aucun pilote.

Ce document est une feuille de route, pas une promesse. Les idées sont classées par
intérêt pour l'usage principal (jouer de son DAW pendant un partage d'écran), puis par
coût.

## 1. À vérifier en premier : ce que Discord capte

Sous Windows, quand on partage **une fenêtre**, Discord semble ne capter que le son de
**l'application de cette fenêtre** (et de ses processus enfants). C'est le
fonctionnement de l'API *application loopback* de Windows
(`AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS`), décrit par Microsoft et utilisé par des
plugins OBS de capture par application.

Conséquence probable :

- partage de **l'écran entier** : Discord capte tout le son de Windows, donc celui de
  Dawsend. **Ça marche** ;
- partage de **la fenêtre du DAW** : Discord capte le processus du DAW, qui sort en
  ASIO, donc rien, et pas Dawsend, qui est un autre processus.

À tester avec les deux modes de partage. Si c'est confirmé, les parades, de la plus
simple à la plus risquée :

1. **Guider** : un assistant de premier lancement et une ligne de dépannage
   « partage l'écran entier » ; ou une petite fenêtre « Dawsend – Partage » à choisir
   dans Discord (le projet DiscordAudioStream fait ça).
2. **Sortie depuis le processus du DAW** : un thread du plugin, hors temps réel, ouvre
   lui-même un flux WASAPI. Le son appartient alors au DAW. Risqué (contraire au
   principe « le plugin ne dépend de rien », un plantage WASAPI se produirait dans le
   DAW) : option avancée, désactivée par défaut, seulement si la parade 1 ne suffit
   pas.
3. **Micro virtuel** (pilote noyau) : voir § 4.4.

## 2. Ce que font les autres outils

| Outil | Ce qu'on lui reproche | Ce qu'on en retient |
| --- | --- | --- |
| **VoiceMeeter** | Interface d'ingénieur du son, périphérique par défaut à changer, craquements après des mises à jour de Windows | Ne jamais toucher au périphérique par défaut. Un seul écran. |
| **VB-Cable** | Craquements dus à la dérive d'horloge (« augmentez la latence à 7168 échantillons »), intégration interdite sans contrat | La compensation de dérive de Dawsend est un vrai avantage. |
| **Virtual Audio Cable** | Correction d'horloge globale au câble | Dawsend corrige par source, plus finement. |
| **ReaStream / ReaRoute** | Craquements dans OBS, ReaRoute limité à Reaper et à certains hôtes ASIO | La mémoire partagée locale est plus sûre que l'UDP local. |
| **OBS + ASIO** | La plupart des pilotes ASIO n'acceptent qu'un client : DAW **ou** OBS | Même avec l'ASIO natif d'OBS, Dawsend reste utile. |
| **Elgato Wave Link, SteelSeries Sonar** | Pilotes virtuels qui plantent, disparaissent ou changent de nom, écho dans le partage Discord | Rester une sortie Windows normale tant que possible. |
| **SonoBus, BlackHole** | — | Sous GPLv3 : on ne peut pas reprendre leur code dans un projet MIT. |

## 3. Principes pour l'extensibilité

### 3.1 Plusieurs lecteurs sur le même anneau

Le plugin n'attend jamais le lecteur et ne lit pas sa position. Plusieurs lecteurs
peuvent donc lire le même anneau **sans changer le format v1** : la sortie Windows,
un envoi réseau, un plugin OBS, un enregistreur. C'est le levier le moins cher : à
inscrire dans [ARCHITECTURE.md](ARCHITECTURE.md) comme garantie du contrat.

### 3.2 Des briques interchangeables dans le moteur

- **`trait Source`** : fournit des trames avec sa propre horloge. Mémoire partagée
  (l'actuelle), entrée micro, capture d'une application.
- **`trait OutputSink`** (prolongement du `Renderer` actuel) : sorties *tirées* par
  le périphérique (WASAPI, PipeWire, CoreAudio) et sorties *poussées* par un thread
  hors temps réel (VBAN, fichier, OBS) à travers une file `rtrb`.
- **Une horloge maîtresse** : les sources sont mixées sur l'horloge de la sortie
  principale ; chaque sortie secondaire reçoit un seul rééchantillonneur sur le mix
  (N + M rééchantillonneurs au lieu de N × M).
- **Pas de `.dll` chargées à chaud** dans le moteur temps réel : trop risqué, et
  l'ABI de Rust n'est pas stable. L'extension passe par la mémoire partagée (§ 3.1) et
  par l'API de contrôle (§ 3.4).

### 3.3 Faire évoluer la mémoire partagée

- Ajouter dans l'en-tête, sur des octets réservés, un `layout_minor` (ajouts
  compatibles) et des bits de capacités (`features`). `LAYOUT_VERSION` reste réservé
  aux ruptures.
- Pendant une transition, l'appli ouvre **v1 et v2 à la fois** (deux noms, deux
  zones) : un vieux plugin dans un vieux projet continue de marcher.
- Pour une v2 : sens du slot (DAW → appli ou appli → DAW), format d'échantillon,
  plus de deux canaux.
- Tests : fichier binaire de référence par version, *fuzzing* du lecteur
  (`cargo-fuzz`), vérification des ordres atomiques avec `loom`.

### 3.4 Une API de contrôle

Un seul service, trois portes d'entrée :

- **ligne de commande** : `dawsend ctl mute Guitare` ;
- **OSC** en UDP, pilotable sans développement par le module *Generic OSC* de
  Bitfocus Companion (Stream Deck) ;
- **WebSocket local** pour un tableau de bord (état, vumètres, commandes).

Écoute sur `127.0.0.1` par défaut, jeton dans les réglages, adresse journalisée au
démarrage.

## 4. Idées de fonctionnalités

### 4.1 Confort

| Idée | Intérêt | Coût | Priorité |
| --- | --- | --- | --- |
| Assistant de premier lancement (partage Discord, bip de test) | Très élevé | Faible | **Haute** |
| Latence affichée par source, test en boucle | Moyen | Faible | Haute |
| Raccourcis globaux (couper tout) | Moyen | Faible | Moyenne |
| Profils ou scènes (volumes et coupures par contexte) | Moyen | Faible | Moyenne |
| Enregistrement du mix en WAV ou FLAC | Moyen | Faible à moyen | Moyenne |
| EQ, compresseur | Faible : le DAW le fait mieux | Moyen | **Non** |

### 4.2 Sorties multiples et réseau

| Idée | Intérêt | Coût | Priorité |
| --- | --- | --- | --- |
| Envoi **VBAN** (protocole ouvert, UDP 6980, compatible VoiceMeeter) : le PC de musique envoie, le PC de stream joue | Élevé pour l'usage à deux PC | Faible à moyen | **Haute** (moyen terme) |
| Réception VBAN dans Dawsend | Élevé | Moyen | Moyenne |
| Même mix vers plusieurs sorties | Moyen | Moyen | Moyenne |
| Matrice source → sortie | Faible | Moyen | Basse, sous « Avancé » |
| NDI, AES67, Dante | Faible pour la cible | Élevé, licences à vérifier | Basse |

### 4.3 Micro et sens inverse

| Idée | Intérêt | Coût | Priorité |
| --- | --- | --- | --- |
| Micro mixé dans la sortie (parler sans passer par le DAW) | Moyen | Faible à moyen | Moyenne |
| *Ducking* : la musique baisse quand on parle | Moyen | Faible une fois le micro là | Basse à moyenne |
| **Dawsend Retour** : le son de Discord dans une piste du DAW, pour enregistrer ou jouer avec un invité | Élevé | Élevé (format v2, capture par processus) | Moyenne (long terme) |

### 4.4 Intégrations et autres systèmes

| Idée | Intérêt | Coût | Priorité |
| --- | --- | --- | --- |
| **Plugin OBS** qui lit la mémoire partagée : chaque piste devient une source OBS, sans périphérique | Élevé pour les streamers | Moyen (binaire GPL, à cause de l'API d'OBS) | **Haute** (moyen terme) |
| **Linux** : nœud PipeWire et micro virtuel « Dawsend » | Élevé | Moyen | Haute |
| **macOS** : sortie CoreAudio, puis Core Audio taps pour le retour | Moyen | Moyen | Moyenne |
| Pilote de micro virtuel Windows | Élevé | Très élevé (code noyau, signature EV, attestation Microsoft) | Basse, seulement si le § 1 l'impose |
| Remplacer `vst3-sys` (GPL) par la crate `vst3` (MIT) | Licence plus simple | Élevé | Basse |

## 5. Feuille de route

### Court terme : fiabiliser l'usage principal

1. Tester le § 1 ; assistant de premier lancement ; dépannage « Discord n'entend rien ».
2. Documenter les lecteurs multiples ; ajouter `layout_minor` et `features`.
3. Introduire `Source` et `OutputSink`, avec `loom` et le *fuzzing* du lecteur.
4. Latence par source, raccourcis globaux.
5. API de contrôle minimale : ligne de commande et OSC sur `127.0.0.1`.

### Moyen terme : ouvrir sans compliquer

1. Linux (PipeWire, micro virtuel), puis macOS.
2. Plugin OBS, dans une crate à part sous GPL.
3. Envoi et réception VBAN, sous « Avancé → Autre PC ».
4. Plusieurs sorties, profils, enregistrement du mix.
5. Micro mixé avec un *ducking* simple.

### Long terme : le sens inverse et les cas avancés

1. Format v2 (sens du slot, plus de canaux), lu en parallèle de v1.
2. Plugin Dawsend Retour (Discord → DAW).
3. Module Bitfocus Companion dédié, WebSocket pour tableaux de bord.
4. Seulement si le besoin est prouvé : micro virtuel Windows, sortie depuis le
   processus du DAW, NDI.

## Sources

- Microsoft, *application loopback* : <https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/>
- win-capture-audio (OBS) : <https://github.com/bozbez/win-capture-audio>
- Discord, partage d'écran : <https://support.discord.com/hc/en-us/articles/360040816151-Go-Live-and-Screen-Share>
- DiscordAudioStream : <https://github.com/p-rivero/DiscordAudioStream>
- OBS, ASIO natif (PR 12733) : <https://github.com/obsproject/obs-studio/pull/12733> ; API des sources : <https://docs.obsproject.com/reference-sources>
- VB-Audio, forum (dérive, latence) : <https://forum.vb-audio.com/viewtopic.php?p=6359> ; licences : <https://vb-audio.com/Services/licensing.htm>
- VBAN, spécification : <https://vb-audio.com/Voicemeeter/VBANProtocol_Specifications.pdf>
- Virtual Audio Cable : <https://vac.muzychenko.net/en/manual/features.htm>
- ReaRoute : <https://www.cockos.com/wiki/index.php/ReaRoute>
- SonoBus : <https://github.com/essej/sonobus>
- Elgato Wave Link, écho dans Discord : <https://help.elgato.com/hc/en-us/articles/46471850961297-Wave-Link-3-0-Discord-Screensharing-Audio-Echo-Issues>
- SteelSeries Sonar, périphériques virtuels : <https://support.steelseries.com/hc/en-us/articles/16955010049933-Why-do-I-have-so-many-Virtual-devices-in-Windows>
- BlackHole : <https://github.com/ExistentialAudio/BlackHole> ; Core Audio taps : <https://github.com/insidegui/AudioCap>
- SysVAD : <https://learn.microsoft.com/en-us/samples/microsoft/windows-driver-samples/sysvad-virtual-audio-device-driver-sample>
- PipeWire, micro virtuel : <https://github.com/Vencord/venmic>
- NDI, licence : <https://docs.ndi.video/docs/sdk/licensing>
- Companion Generic OSC : <https://github.com/bitfocus/companion-module-generic-osc>
