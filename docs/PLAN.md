# Plan : song-audio-bureau

Version de bureau du studio [song-audio](https://github.com/aL00ss/song-audio). Depuis le
2026-11-18, le but est un studio complet, au niveau des grands logiciels de production musicale
(FL Studio sert de référence pour l'étendue des fonctions) : mêmes possibilités, mais notre
propre forme. Rien n'est copié : ni nom, ni logo, ni habillage, ni contenu d'usine ; les panneaux
portent des noms français à nous.

## Principes

- **Le moteur Rust est le chef d'orchestre** : transport à l'échantillon près, graphe audio
  (canaux, pistes de mixage, effets, envois), automatisation, instruments, greffons. Il tourne
  sans l'interface (rendu hors ligne, tests).
- **L'interface React est un client** : elle garde le modèle du projet (TypeScript pur, testé,
  avec annuler/rétablir) et envoie au moteur une **session compilée** (ce qui doit sonner, quand
  et comment) ; le moteur renvoie l'horloge, les niveaux et les événements.
- On garde ce qui est déjà solide (modèle de projet, logique musicale, doigtés, import et export
  MIDI, tests) ; on remplace tout le son (SpessaSynth et la Web Audio API) par le moteur Rust.
- Chaque phase se termine par : tests verts (`cargo test`, Vitest, tests navigateur, tests de la
  vraie fenêtre), un `.exe` (installeur NSIS) reconstruit et vérifié, le plan mis à jour, un
  commit poussé.
- Licences : GPL-3.0 pour le code ; sons seulement sous licences redistribuables (CC0, CC BY,
  « sans restriction »), crédités dans l'application.

## Architecture cible

```text
interface (React, TypeScript)                moteur (Rust)
  modèle du projet + annuler/rétablir   ──►  session compilée (serde)
  panneaux, éditeurs, navigateur        ◄──  horloge, niveaux, spectres, événements
                                              transport (tempo, boucles, métronome)
                                              graphe : canaux -> pistes -> envois -> maître
                                              instruments : SF2, SFZ, échantillonneur,
                                                synthés, greffons CLAP/VST3
                                              effets, automatisation, enregistrement
                                              sortie cpal / rendu hors ligne (WAV, FLAC…)
```

Organisation du code Rust : une crate `moteur` (bibliothèque, sans Tauri, entièrement testée)
et l'application `src-tauri` qui l'expose à l'interface.

## Phases

| # | Phase | Contenu | Vérification |
|---|---|---|---|
| A | Moteur Rust (fondations du son) | Noyau : un synthétiseur par canal (SF2 GeneralUser GS), file d'événements datés, mixer complet (égaliseur 3 bandes, fader, panoramique, muet, solo, envoi réverbération, limiteur), sortie cpal, mesures ; client TypeScript qui remplace SpessaSynth (horloge synchronisée, événements groupés) ; export WAV par rendu hors ligne en Rust | Justesse, timing à l'échantillon, niveaux, latence ; l'interface entière fonctionne sur le moteur Rust |
| B | Transport dans le moteur | Le moteur lit lui-même la session (patterns, playlist) : plus de programmation à l'avance depuis l'interface ; boucles et sélection de temps, métronome, décompte, changements de tempo et de signature | Rendu identique en direct et hors ligne ; aucune note perdue sous charge |
| C | Annuler/rétablir et projets | Historique complet de toutes les opérations ; format de projet natif (archive avec les échantillons utilisés) ; sauvegarde automatique et récupération après plantage ; modèles de projet | Chaque opération annulable ; projet rouvert à l'identique |
| D | Échantillonneur et banques | Canaux échantillonneur (un son ou multi-échantillons : enveloppes, filtre, LFO, transposition, inversion, boucle) ; lecteur SFZ (sfizz) ; gestionnaire de banques (téléchargement, reprise, sommes de contrôle) ; banques professionnelles : Salamander complet, VCSL, VSCO 2 CE complet, Iowa, Naked Drums, DrumGizmo, Karoryfer | Justesse et niveaux de chaque instrument |
| E | Rack d'instruments avancé | Séquenceur pas à pas avec paramètres par pas (vélocité, panoramique, hauteur, relâchement, décalage), éditeur graphique, couches, groupes et filtres de canaux, aléatoire, glisser un son du navigateur vers le rack | Tests de chaque paramètre |
| F | Piano roll avancé | Notes fantômes des autres canaux, gammes et aimantation à la gamme, accords et arpèges, accords plaqués et égrenés, quantification avec force, legato, glissandos et portamento, paramètres par note (panoramique, relâchement, hauteur fine, deux modulations), couleurs de notes, découpe et outils | Tests de chaque outil |
| G | Arrangement avancé | Pistes illimitées, clips de pattern, **clips audio** (forme d'onde, étirement, transposition), **clips d'automatisation**, marqueurs, boucle de sélection, outils (couper, coller, scinder, fusionner, rendre unique, muet), couleurs, regroupement | Tests d'arrangement et de rendu |
| H | Mixer professionnel | Pistes d'insertion à volonté, 10 emplacements d'effets par piste, envois de n'importe quelle piste vers n'importe quelle autre, sidechain, inversion de phase, largeur stéréo, compensation de latence, vumètres crête et RMS | Tests de routage, sans boucle ; latence compensée |
| I | Effets | Égaliseur paramétrique 7 bandes avec spectre, compresseur (et multibande), limiteur, porte, réverbérations (algorithmique et à convolution), délais synchronisés au tempo, chorus, flanger, phaser, saturation, bitcrusher, filtres, élargisseur stéréo, transitoires, analyseur de spectre, oscilloscope | Mesures de chaque effet |
| J | Synthétiseurs | Soustractif à 3 oscillateurs, FM à 6 opérateurs, table d'ondes, basse « 808 » à glissando, boîte à rythmes synthétisée, orgue, piano électrique ; préréglages par style | Tests de hauteur, d'enveloppes, d'aliasing |
| K | Automatisation et MIDI | Automatiser n'importe quel paramètre (clic droit sur un bouton), courbes, LFO d'automatisation, enregistrement des mouvements, claviers MIDI externes (midir), apprentissage MIDI, sortie MIDI | Lecture d'automatisation à l'échantillon près |
| L | Audio : enregistrement et édition | Entrée micro/carte son, écoute, enregistrement dans l'arrangement, éditeur audio (normaliser, inverser, fondus, couper), étirement temporel et transposition (bibliothèque Signalsmith Stretch, MIT), détection du tempo, découpeur de boucles | Enregistrement sans décalage ; étirement sans changement de hauteur |
| M | Greffons | Accueil CLAP puis VST3 : instruments et effets, interfaces dans des fenêtres, sauvegarde de leur état dans le projet, analyse des greffons installés | Surge XT (gratuit) joue et s'enregistre |
| N | Navigateur et export | Navigateur de fichiers et d'échantillons avec écoute, favoris, glisser-déposer partout, base de greffons et de préréglages ; export WAV, FLAC, MP3, OGG, pistes séparées, sélection | Chaque format relu |
| O | Finitions | Fenêtres détachables, thèmes, barre d'aide, raccourcis partout, mise à l'échelle, indicateurs (processeur, mémoire, polyphonie), mode performance (lancement de clips), installeurs Windows/macOS/Linux, mises à jour automatiques, documentation | Charge : 64 pistes, 16 instruments, effets, sans craquement |

## Avancement

- [x] **Fondations de l'application** : Tauri 2 affiche l'interface de song-audio (le son passe
  encore par SpessaSynth dans WebView2) ; commande Rust `version` ; icône originale ;
  GeneralUser GS en SF2 (32 Mo) inclus ; tests de la vraie fenêtre par Playwright branché sur
  WebView2 (`npm run test:bureau`) ; CI sur Windows, macOS et Linux ; premier installeur
  `song-audio_0.1.0_x64-setup.exe`. Poste de développement Windows sans droits administrateur :
  chaîne Rust `x86_64-pc-windows-gnu` (`rustup override`) et MinGW-w64 portable (WinLibs : GCC,
  CMake) dans `C:\Users\user\outils\mingw64` ; la CI, elle, utilise MSVC. Le type de crate
  `cdylib` (mobile) est retiré : il dépasse la limite d'exports du lieur GNU.
- [x] **A — Moteur Rust** : crate `moteur` (bibliothèque sans Tauri) : un synthétiseur
  rustysynth par canal (32 canaux, batterie sur n'importe lequel), file d'événements datés à
  l'échantillon, mixer complet porté de la version web (égaliseur 3 bandes aux formules de la
  Web Audio API, fader quadratique, panoramique, muet et solo, envoi vers une réverbération
  Freeverb, limiteur -3 dB du maître), sortie cpal sans verrou ni allocation dans le fil audio
  (files rtrb), **horloge silencieuse** au rythme réel quand aucune carte son n'est disponible
  (message dans l'interface), rendu hors ligne et WAV ; commandes Tauri ; client TypeScript
  `MoteurNatif` derrière une interface commune `MoteurAudio` (le navigateur garde SpessaSynth) ;
  export WAV rendu par Rust. 10 tests Rust, 88 tests unitaires, 64 tests navigateur, 3 tests de
  la vraie fenêtre. Limites reportées : banques supplémentaires (SF3 de la version web) pas
  encore lues par le moteur Rust (phase D), changement d'appareil audio à chaud (phase O), la
  programmation des notes reste dans l'interface (phase B).

## Banques de sons retenues (recherche du 2026-11-18)

Licences vérifiées, toutes redistribuables :

| Famille | Banque | Licence | Taille |
|---|---|---|---|
| Piano à queue | Salamander Grand Piano V3 complet, 48 kHz 24 bits (Alexander Holm) | CC BY 3.0 | 1,2 Go |
| Pianos, clavecins | VCSL Keys (Versilian Studios) | CC0 | 680 Mo |
| Orchestre | VSCO 2 CE complet (Versilian Studios), toutes articulations | CC0 | 2,3 Go |
| Instruments variés | VCSL complet (Versilian Studios) | CC0 | 5 Go |
| Orchestre, complément | University of Iowa MIS (Lawrence Fritts) | « sans restriction » | — |
| Batterie | Naked Drums (Wilkinson Audio) | CC BY 4.0 | 1,3 Go |
| Batterie | DrumGizmo DRS Kit, Muldjord Kit | CC BY 4.0 | 1,1 Go |
| Batterie | Karoryfer Big Rusty Drums, Unruly Drums | CC0 | — |
| Basse électrique | Karoryfer Growlybass, Swagbass, Fashionbass, Black and Blue Basses | CC0 | — |
| General MIDI (secours) | GeneralUser GS (S. Christian Collins) | libre, logiciels compris | 32 Mo |

Écartées : Piano in 162, Philharmonia, Sonatina, Virtual Playing Orchestra, jRhodes3d
(licences qui interdisent ou rendent douteuse la redistribution). Piano électrique et orgue :
synthétisés (phase J).
