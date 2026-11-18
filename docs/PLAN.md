# Plan : song-audio-bureau

Étape 10 du plan de [song-audio](https://github.com/aL00ss/song-audio/blob/main/docs/PLAN.md),
dans son propre dépôt. But : des sons qui tiennent la comparaison avec un studio commercial,
en gardant l'interface de song-audio. Le détail des banques retenues et de leurs licences est
dans la section « Étape 10 » du plan de song-audio (recherche du 2026-11-18).

## Choix

| Sujet | Choix |
|---|---|
| Application | Tauri 2 (fenêtre WebView2 / WebKit), interface React reprise de song-audio |
| Sortie audio | cpal (WASAPI, CoreAudio, ALSA/JACK), faible latence |
| Lecture SFZ | sfizz (BSD-2), bibliothèque C++ liée depuis Rust par son API C |
| General MIDI de secours | GeneralUser GS en SF2, lu par un lecteur SoundFont en Rust |
| Synthèse et effets | fundsp (MIT/Apache) ; moteur de Surge XT envisageable (GPL-3, compatible) |
| Greffons | CLAP et VST3 (synthés gratuits installés par l'utilisateur : Surge XT, Vital, Dexed…) |
| Licence | GPL-3.0 |

## Étapes

| # | Étape | Contenu | Vérification |
|---|---|---|---|
| 10.0 | Fondations | Outils (Rust, MSVC), squelette Tauri, interface de song-audio affichée, CI | L'application s'ouvre ; CI verte sur Windows, macOS, Linux |
| 10.1 | Moteur Rust | Sortie cpal, horloge, file d'événements datés, transport en tics, General MIDI (GeneralUser GS) ; l'interface passe par des commandes Tauri au lieu de SpessaSynth | Notes justes et à l'heure (rendu hors ligne), latence < 10 ms |
| 10.2 | Gestionnaire de banques | Téléchargement au premier lancement (choix, progression, reprise, somme de contrôle), rangement sur le disque, page Crédits | Téléchargement interrompu puis repris |
| 10.3 | SFZ et banques acoustiques | sfizz intégré ; Salamander complet, VCSL Keys, VSCO 2 CE complet, VCSL, Iowa MIS, Naked Drums, DrumGizmo, Karoryfer ; articulations choisies dans l'interface | Justesse et niveaux de chaque instrument |
| 10.4 | Synthèse | Soustractif à 3 oscillateurs, FM, table d'ondes, batteries électroniques synthétisées, orgue, piano électrique ; préréglages par style | Tests de hauteur et d'enveloppes |
| 10.5 | Effets | Réverbération (algorithmique, convolution), délai, chorus, flanger, phaser, compresseur avec sidechain, saturation, égaliseur paramétrique, filtres automatisables | Mesures de chaque effet |
| 10.6 | Greffons | Accueil CLAP puis VST3 dans le Channel Rack et le mixer | Un synthé gratuit joue et s'enregistre dans le projet |
| 10.7 | Livraison | Installeurs (Windows, macOS, Linux), mises à jour, charge avec 16 instruments | 16 instruments sans craquement |

## Avancement

- [ ] 10.0 Fondations : dépôt créé (licence, plan) ; reste à installer Rust et les outils C++
  de Microsoft (MSVC, SDK Windows) sur la machine de développement.
