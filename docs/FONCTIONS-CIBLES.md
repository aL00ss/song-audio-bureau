# Fonctions cibles : le but final de song-audio

Date : 2026-11-18. Ce document est **le but final du projet** : la liste de toutes les fonctions
de FL Studio (jusqu'à la version 2026), une par une, avec ce qui leur correspondra dans
song-audio, leur état et la phase du plan qui les réalise ([PLAN.md](PLAN.md)).

## Règles

- **Mêmes possibilités, notre propre forme.** Les noms de FL Studio et de ses greffons sont
  cités dans la colonne « Référence » uniquement pour savoir de quoi l'on parle. song-audio ne
  reprend ni ces noms, ni le logo, ni l'habillage, ni les sons ou préréglages d'usine : chaque
  fonction porte un nom français à nous, et le contenu vient de banques libres
  ([banques retenues](PLAN.md#banques-de-sons-retenues-recherche-du-2026-11-18)).
- **Pas de limite** là où FL Studio en a une (pistes de mixage, canaux, notes…) : voir
  « Exigence : pas de limite » dans le plan.
- **Services en ligne payants** (bibliothèque de sons en nuage, distribution vers les plateformes,
  greffons de tiers par abonnement) : remplacés par des équivalents libres et locaux quand c'est
  possible ; sinon, notés hors du périmètre.

## Légende

| État | Sens |
|---|---|
| ✅ | Fait dans l'application native (Rust) |
| 🌐 | Fait dans la version web ou l'ancienne application Tauri, à reconstruire en natif |
| 🟡 | Commencé |
| ⬜ | À faire |
| ⛔ | Hors du périmètre (service commercial), remplacé par la note indiquée |

Phases : N0 à N9 (reconstruction native), A à O (fonctions), ⟂ (exigence « pas de limite »), voir le plan.

---

## 1. Application, fenêtres, interface

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Windows, macOS (Intel et Apple Silicon) | FL Studio Windows/macOS | Application native Rust | 🟡 (Windows ; macOS et Linux construits en CI) | O |
| Linux | — (non pris en charge par FL) | Application native Rust | 🟡 | O |
| Version dans le navigateur | Version web (bêta fermée 2025) | Site song-audio (TypeScript) | 🌐 | — |
| Panneaux ancrables, onglets | Fenêtres ancrables | Coquille à panneaux (egui_dock) | ✅ | N1 |
| Fenêtres détachées, plusieurs écrans | Détacher une fenêtre | Fenêtres détachables multi-écrans | ⬜ | O |
| Dispositions mémorisées | Window sets | Dispositions enregistrées et nommées | 🟡 (une disposition gardée) | N1, O |
| Raccourcis des panneaux (F5 à F9…) | F5 à F9, F10, F11, F12 | Mêmes touches, configurables | ✅ (F5 à F9) | N1 |
| Barre d'aide (texte au survol) | Hint bar | Barre d'aide en bas de l'écran | 🟡 (infobulles) | O |
| Thèmes de couleurs | Theme / Hue | Thèmes et teinte réglable | ⬜ | O |
| Mise à l'échelle de l'interface, écrans 4K | Scaling, vectoriel | Mise à l'échelle egui | ⬜ | O |
| Écrans tactiles, multi-touch | Multi-touch | Gestes tactiles | ⬜ | O |
| Indicateurs processeur, mémoire, polyphonie | CPU / RAM / poly | Indicateurs de charge (canaux éveillés, voix, cœurs) | ⬜ | O |
| Barre de transport, afficheurs | Toolbar | Barre de transport | ✅ | N0 |
| Page d'accueil, projets récents | Start page | Accueil : projets récents, modèles | ⬜ | N8 |
| Tutoriels intégrés | Tutorials | Aide intégrée en français | ⬜ | O |
| Assistant conversationnel | Gopher (IA, 2025 ; agit dans le projet en 2026) | Assistant (Claude ou modèle local) qui répond et agit dans le projet | ⬜ | O |
| Langues | Interface anglaise surtout | Français d'abord, puis autres langues | ✅ (français) | — |

## 2. Projets, fichiers, sécurité

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Nouveau, ouvrir, enregistrer, enregistrer sous | Fichier | Menu Fichier, boîtes du système | ✅ | N0 |
| Modèles de projet | Templates | Modèles (vide, hip-hop, électro, orchestre, piano, démo) | 🌐 | N8 |
| Sauvegarde automatique, copies de secours | Autosave, backups | Sauvegarde automatique horodatée | 🌐 | N8 |
| Reprise après plantage | Recover | Reprise du dernier état | 🌐 | N8 |
| Annuler, rétablir, historique visible | Undo history (suppression de pistes annulable en 2025) | Historique complet, liste des étapes | 🌐 | N8 |
| Projet compressé avec ses échantillons | Export project data files / ZIP | Archive de projet avec ses sons | ⬜ | D |
| Infos du projet (titre, auteur, genre, notes) | Project info | Fiche du projet | ⬜ | N8 |
| Sauvegarde du projet en ligne | FL Cloud backup (2026) | Dossier synchronisé au choix (local, Git, nuage personnel) | ⛔ → dossier synchronisé | O |
| Import de projets d'autres logiciels | (partiel) | Import MIDI (un instrument par canal), puis formats ouverts (DAWproject) | 🟡 (MIDI ✅) | N2, N |
| Format ouvert compatible web | — | Même JSON que la version web | ✅ | N0 |

## 3. Transport, temps, enregistrement

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Lecture, pause, arrêt | Play / Stop | Transport du moteur | ✅ | B, N0 |
| Mode pattern / mode morceau | PAT / SONG | Pattern / Morceau | ✅ | B, N0 |
| Tempo (décimal), tap tempo | Tempo, Tap | Tempo, tap tempo | 🟡 (tempo) | N0, K |
| Changements de tempo dans le morceau | Tempo automation | Automatisation du tempo | ⬜ | K |
| Signature rythmique et ses changements | Time signature markers | Marqueurs de signature | 🟡 (signature fixe) | G |
| Métronome, décompte | Metronome, countdown | Métronome, décompte | ✅ (décompte : moteur) | B |
| Attente d'une note pour enregistrer | Wait for input | Attendre la première note | ⬜ | L |
| Enregistrement des notes, de l'audio, de l'automatisation | Recording filter | Filtres d'enregistrement | 🌐 (notes) | L |
| Enregistrement en boucle, superposition, remplacement | Loop record, overdub, blend | Boucle, superposition, remplacement | 🟡 (superposition, web) | L |
| Punch in/out | Punch | Punch | ⬜ | L |
| Quantification à l'enregistrement | Quantize on record | Quantification à l'enregistrement | ⬜ | L |
| Mode pas à pas (saisie note par note) | Step editing | Saisie pas à pas | ⬜ | F |
| Clavier de l'ordinateur comme piano | Typing keyboard to piano | Clavier de l'ordinateur | 🌐 | N4 |
| Afficheur mesures / temps / secondes | Song position | Afficheurs | ✅ | N0 |
| Sélection de temps en boucle | Time selection loop | Boucle de sélection | ⬜ | B, G |
| Journal audio (rien n'est perdu) | Audio Logger (2026) | Enregistrement continu de la sortie | ⬜ | L |
| Synchronisation MIDI (horloge) | MIDI clock sync | Horloge MIDI entrée et sortie | ⬜ | K |

## 4. Navigateur et sons

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Arborescence des dossiers et fichiers | Browser | Navigateur de fichiers | ✅ | N2 |
| Écoute avant d'ajouter, au tempo | Preview, tempo sync | Écoute synchronisée | ⬜ | D, N |
| Glisser-déposer partout | Drag and drop | Glisser vers rack, arrangement, piano roll | 🟡 (fichiers sur la fenêtre) | N2, N |
| Favoris, recherche, étiquettes | Favorites, search, tags | Favoris, recherche, étiquettes | ⬜ | N |
| Contenu du projet (sons, greffons, patterns) | Current project | Contenu du projet | ⬜ | N |
| Base de greffons et de préréglages | Plugin database | Base des instruments et effets | ⬜ | M, N |
| Bibliothèque de sons en ligne | FL Cloud Sounds (2 millions de sons) | Banques libres téléchargées (CC0, CC BY) | ⛔ → banques libres | D |
| Trouver des sons semblables | Find similar samples (2025.2) | Recherche par ressemblance (analyse locale) | ⬜ | N |
| Recherche de morceaux MIDI en ligne | — | Recherche BitMidi | ✅ | N2 |
| Instruments et synthés intégrés à ajouter | Add channel | Liste des instruments et synthés | ✅ | N0 |
| Démarrage par boucles assorties | Loop Starter | Démarreur d'idées (boucles et sons assortis, banques libres) | ⬜ | N |
| Remixer un morceau (séparation en pistes) | Remix a Song (2026) | Remix : séparation en pistes dans l'arrangement | ⬜ | L |

## 5. Rack d'instruments et séquenceur pas à pas

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Liste des canaux, sans limite | Channel Rack (plafonné) | Rack d'instruments, illimité, virtualisé | ✅ | N0, ⟂ |
| Séquenceur pas à pas | Step sequencer | Pas cliquables | ✅ (clic, clic droit) | N3 |
| Peindre au glisser, effacer | Paint steps | Peinture et effacement au glisser | 🌐 | N3 |
| Longueur du pattern, pas par mesure | Pattern length | Longueur réglable (1 à 512 pas) | 🌐 | N3 |
| Paramètres par pas : vélocité, relâchement, hauteur fine, panoramique, modulations X/Y, décalage | Graph editor | Éditeur graphique des pas | 🟡 (vélocité, web) | E |
| Swing global et par canal | Swing, channel swing | Swing | 🌐 (global) | N3, E |
| Muet, solo, volume, panoramique par canal | Mute, solo, knobs | Muet, solo, volume ; panoramique | 🟡 | N3 |
| Témoins d'activité | Activity LED | Témoins | ✅ | N0 |
| Sélection de la piste de mixage | Mixer track | Routage vers une piste | 🌐 | N5 |
| Groupes et filtres de canaux | Filter groups | Groupes de canaux | ⬜ | E |
| Couleurs, icônes, noms | Color, icon | Couleurs, noms | 🟡 | N3 |
| Cloner, déplacer, supprimer, remplacer | Clone, move, delete, replace | Mêmes actions | ⬜ | N3 |
| Remplir tous les N pas, décaler, inverser, aléatoire | Fill each N steps, rotate, randomize | Mêmes outils | ⬜ | E |
| Couches (un déclenchement joue plusieurs canaux) | Layer channel | Couches | ⬜ | E |
| Envoyer au piano roll | Send to piano roll | Ouvrir dans le piano roll | ⬜ | N4 |
| Canaux d'automatisation et de clips audio | Automation / audio clip channels | Canaux spéciaux | ⬜ | G, K |

## 6. Échantillonneur de canal (réglages d'un son)

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Lecture d'échantillon, note de base, plage | Channel Sampler | Échantillonneur | ⬜ | D |
| Inverser, normaliser, déclic, fondus, boucle (début/fin) | Precomputed effects, loop (2025.2) | Préparation du son | ⬜ | D |
| Enveloppes (volume, filtre, hauteur, panoramique) | Envelope / LFO | Enveloppes ADSR et LFO | 🟡 (synthé intégré) | D |
| Filtre (types multiples) | Filter | Filtres | 🟡 (synthé intégré) | D |
| Étirement temporel et transposition | Time stretching | Étirement (Signalsmith Stretch) | ⬜ | L |
| Arpégiateur de canal | Arpeggiator | Arpégiateur | ⬜ | F |
| Polyphonie, mono, portamento, glissé | Polyphony, porta | Polyphonie, portamento | 🟡 (glissé du synthé) | D |
| Hauteur, plage de pitch bend | Pitch, range | Hauteur et plage | ⬜ | D |
| Retard d'effet (écho intégré au canal) | Echo delay / fat mode | Écho du canal | ⬜ | I |

## 7. Piano roll

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Dessin, peinture, effacement, sélection, zoom | Draw, paint, delete, select, zoom | Outils d'édition (dessin GPU) | 🌐 | N4 |
| Couper (trancher), coller, muet, lecture d'une zone | Slice, glue, mute, playback | Mêmes outils | ⬜ | F |
| Lasso, sélection par hauteur, tout sélectionner | Select tools | Sélections | 🌐 | N4 |
| Quantification, quantification rapide, force | Quantize | Quantification avec force et swing | 🟡 (simple, web) | F |
| Notes fantômes des autres canaux | Ghost notes | Notes fantômes | ⬜ | F |
| Gammes : surlignage et aimantation | Scale highlighting, snap to scale | Gammes | ⬜ | F |
| Tampons d'accords | Stamp | Tampons d'accords | ⬜ | F |
| Outil de progression d'accords | Chord Progression tool | Progressions d'accords | ⬜ | F |
| Générateur de ligne de basse | Bassline generator (2025) | Basse automatique sur la progression | ⬜ | F |
| Détection des notes et des accords en temps réel | Note/chord detection (2026) | Nom des accords joués | ⬜ | F |
| Arpéger, égrener, flam | Arpeggiate, strum, flam | Arpège, égrené, flam | ⬜ | F |
| Legato, limiter, inverser, mise à l'échelle | Legato, limit, flip, scale levels | Mêmes outils | ⬜ | F |
| Générateur de motifs aléatoires guidés | Randomize, Riff machine | Générateur de motifs | ⬜ | F |
| Outil LFO sur les paramètres de notes | LFO tool | LFO de notes | ⬜ | F |
| Hacher, articuler | Chop, articulate | Hacher | ⬜ | F |
| Propriétés par note : vélocité, relâchement, panoramique, hauteur fine, modulations X/Y | Note properties | Propriétés par note | 🟡 (vélocité) | F |
| Notes glissées, portamento | Slide notes, porta | Glissandos | ⬜ | F |
| Couleurs de notes, canal MIDI par note | Note colors | Couleurs de notes | ⬜ | F |
| Couloirs d'événements (vélocité, contrôleurs, pitch bend) | Event editor lanes | Couloirs d'édition | 🟡 (vélocité, web) | F, K |
| Marqueurs de temps | Time markers | Marqueurs | ⬜ | F |
| Scripts d'édition | Piano roll scripts (Python) | Scripts (Lua ou Rhai) | ⬜ | F |
| Clavier vertical jouable | Keyboard | Clavier jouable | 🌐 | N4 |
| Import et export MIDI d'un pattern | Import/export MIDI | Import/export par pattern | 🌐 (projet) | N8 |

## 8. Arrangement (playlist)

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Pistes illimitées | Playlist tracks (500) | Pistes sans limite, virtualisées | 🌐 (quelques pistes) | N6, G |
| Clips de pattern | Pattern clips | Clips de pattern | 🌐 | N6 |
| Clips audio : forme d'onde, étirement, transposition, inversion, fondus | Audio clips (2025 : propriétés de clip) | Clips audio | ⬜ | G |
| Clips d'automatisation : courbes, LFO, pas | Automation clips | Clips d'automatisation | ⬜ | G, K |
| Plusieurs arrangements | Arrangements | Plusieurs arrangements | ⬜ | G |
| Outils : peindre, couper, coller, muet, glisser le contenu | Paint, slice, mute, slip | Outils | 🌐 (poser, déplacer, supprimer) | N6, G |
| Rendre unique, fusionner, scinder par canal | Make unique, merge, split by channel | Mêmes actions | ⬜ | G |
| Marqueurs (temps, boucle, signature) | Markers | Marqueurs | ⬜ | G |
| Regroupement, couleurs, icônes de pistes | Track groups, colors | Groupes de pistes | ⬜ | G |
| Rendu en audio, gel | Consolidate / render | Rendu d'une sélection en audio | ⬜ | G, N |
| Mode performance (lancement de clips) | Performance mode | Mode scène | ⬜ | O |
| Prises multiples (comping) | Takes | Prises | ⬜ | L |
| Panneau de choix (patterns, sons) | Picker panel | Panneau de choix | ⬜ | N6 |
| Aimantation à la grille | Snap | Aimantation | 🌐 | N6 |

## 9. Mixage

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Pistes de mixage sans limite | Dynamic mixer tracks (jusqu'à 500, 2025) | Pistes illimitées | ✅ (moteur) ; ⬜ (interface) | ⟂, N5 |
| Supprimer les pistes inutilisées | Trim unused tracks (2025.2) | Nettoyage des pistes | ⬜ | N5 |
| Égaliseur 3 bandes par piste | Track EQ | Égaliseur | ✅ (moteur) | A, N5 |
| Fader, panoramique, muet, solo | Fader, pan | Tranches | ✅ (moteur) | A, N5 |
| Vumètres crête et RMS | Peak meters | Vumètres | ✅ (crêtes) | A, N5 |
| 10 emplacements d'effets par piste | 10 FX slots | Chaînes d'effets sans limite | ⬜ | H |
| Envois de n'importe quelle piste vers n'importe quelle autre | Routing | Routage libre, sans boucle | 🟡 (envoi réverbération) | H |
| Sidechain | Sidechain | Sidechain | ⬜ | H |
| Séparation stéréo, inversion gauche/droite, inversion de phase | Stereo separation, swap, invert | Mêmes réglages | ⬜ | H |
| Compensation de latence | PDC | Compensation automatique | ⬜ | H |
| Entrées et sorties audio par piste, armement | Track input/output, arm | Entrées/sorties, enregistrement | ⬜ | L |
| États du mixage, préréglages de piste | Mixer states | Préréglages | ⬜ | H |
| Liens canal → piste en masse | Link channels | Liaison automatique | ⬜ | N5 |
| Désactivation automatique des effets inactifs | Smart disable | Mise en sommeil (comme les canaux) | 🟡 (canaux) | H |
| Rendu multicœur | Multithreaded | Rendu sur tous les cœurs | ✅ | ⟂ |
| Limiteur du maître | Master limiter | Limiteur | ✅ | A |

## 10. Instruments (générateurs)

Chaque instrument de FL Studio, sa fonction, et son équivalent prévu (nom à nous).

| Référence FL Studio | Ce qu'il fait | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Channel Sampler / Audio Clip | Lecture d'échantillons | Échantillonneur | ⬜ | D |
| DirectWave | Échantillonneur multi-couches | Échantillonneur multi-couches (SFZ par sfizz) | ⬜ | D |
| SoundFont Player (2026) | Banques SF2 | Lecteur SoundFont | ✅ | A |
| FPC | Pads de batterie, couches par pad (2026) | Pads de batterie | 🌐 (pads) | J |
| Fruity Slicer, Slicex, Fruity Slicer 2 (2025.2) | Découpe de boucles | Découpeur de boucles | ⬜ | L |
| FL Keys | Piano | Piano échantillonné (Salamander) | 🌐 | D |
| FLEX (refait en 2026) | Lecteur de préréglages | Lecteur de préréglages par style | ⬜ | J |
| 3x Osc | Soustractif simple | Synthé soustractif | ✅ (6 sons) | J |
| Sytrus | FM, additive, RM | Synthé FM 6 opérateurs | ⬜ | J |
| Harmor, Harmless | Additive, resynthèse | Synthé additif et resynthèse | ⬜ | J |
| Kepler | Synthé moderne (LFO redéclenché 2025.2) | Synthé à table d'ondes | ⬜ | J |
| Toxic Biohazard, Poizone, Morphine, Sawer | Soustractifs, additif | Variantes du soustractif et de l'additif | ⬜ | J |
| Transistor Bass, BooBass | Basses | Basses synthétisées (« 303 », électrique) | 🟡 (Basse ronde, Lead acide) | J |
| Sakura, Plucked! | Modélisation de cordes | Cordes physiques (Karplus-Strong) | ⬜ | J |
| GMS | Synthé polyvalent | Synthé polyvalent | ⬜ | J |
| BassDrum, Drumaxx, Drumpad, Fruit Kick | Batteries synthétisées | Batterie synthétisée (grosse caisse, caisse claire…) | ⬜ | J |
| Fruity Granulizer | Synthèse granulaire | Synthé granulaire | ⬜ | J |
| Fruity DX10, SimSynth, Wasp, MiniSynth, Ogun, Autogun | Synthés classiques | Préréglages des synthés ci-dessus | ⬜ | J |
| Fruity Envelope Controller, Keyboard Controller | Contrôleurs | Contrôleurs internes | ⬜ | K |
| Note Arpeggiator (2026) | Arpégiateur MIDI | Arpégiateur | ⬜ | F |
| Patcher, VFX (Keyboard Splitter, Color Mapper, Script) | Assemblage modulaire, scripts | Patcheur modulaire | ⬜ | M |
| MIDI Out | Pilotage d'appareils MIDI | Sortie MIDI | ⬜ | K |
| Mobile Rack (2025) | Instruments de FL Mobile | — (nos instruments) | ⛔ | — |
| Greffons VST2/VST3/CLAP/AU | Instruments tiers | Accueil CLAP, VST3 (processus séparé) | ⬜ | M |
| Orchestre | (greffons tiers) | Banques VSCO 2, VCSL, Iowa | 🌐 (VSCO allégé) | D |
| Synthé de basse « 808 » | (préréglages) | Basse 808 à glissando | ⬜ | J |

## 11. Effets

| Référence FL Studio | Ce qu'il fait | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Parametric EQ 2 | Égaliseur paramétrique 7 bandes, spectre | Égaliseur paramétrique | ⬜ | I |
| Compressor (sidechain 2025.2), Multiband Compressor, Maximus | Compression, multibande | Compresseurs | ⬜ | I |
| Limiter, Emphasis / Emphasizer (2025) | Limiteurs, limiteur multi-étages | Limiteurs | 🟡 (limiteur du maître) | I |
| Soft Clipper | Écrêtage doux | Écrêteur | ⬜ | I |
| Transient Processor, Transmitter (2026) | Transitoires, séparation transitoire/tenue | Transitoires | ⬜ | I |
| Reeverb 2, Luxeverb, Convolver | Réverbérations | Réverbération algorithmique et à convolution | 🟡 (réverbération commune) | I |
| Delay 3, Delay Bank, Multiband Delay | Délais | Délais synchronisés au tempo | ⬜ | I |
| Chorus, Flanger, Phaser, Vintage Chorus, Vintage Phaser | Modulations | Chorus, flanger, phaser | ⬜ | I |
| Blood Overdrive, Fast Dist, Squeeze, WaveShaper, Distructor | Saturations, distorsions | Saturation, distorsion, conformateur d'onde | ⬜ | I |
| Bitcrusher (dans Squeeze) | Réduction de résolution | Bitcrusher | ⬜ | I |
| Filter, Free Filter, Love Philter | Filtres, filtres modulés | Filtres | ⬜ | I |
| Stereo Shaper, Stereo Enhancer, Spreader (2024) | Image stéréo | Élargisseur stéréo | ⬜ | I |
| Low Lifter (2024) | Harmoniques des basses | Renfort des basses | ⬜ | I |
| Balance, Mute 2, Center, Phase Inverter | Utilitaires | Utilitaires | ⬜ | I |
| Frequency Shifter, Pitch Shifter, Frequency Splitter | Décalages, séparation de bandes | Décaleurs, séparateur de bandes | ⬜ | I |
| Gross Beat | Manipulations de temps et de volume | Effet de hachage et de ralenti | ⬜ | I |
| Vocodex | Vocodeur | Vocodeur | ⬜ | I |
| Pitcher, NewTone | Correction de hauteur (temps réel, éditeur) | Correction de hauteur | ⬜ | L |
| NewTime | Éditeur de temps (recalage) | Recalage rythmique | ⬜ | L |
| Scratcher | Effet de platine | Scratch | ⬜ | I |
| Effector | Multi-effet | Multi-effet | ⬜ | I |
| Spectroman, Wave Candy, dB Meter, Tuner, Big Clock | Analyseurs, accordeur | Spectre, oscilloscope, niveaux, accordeur | ⬜ | I |
| ZGameEditor Visualizer | Vidéo musicale | Visualiseur | ⬜ | O |
| Peak Controller, Formula Controller, X-Y Controller | Contrôleurs internes | Contrôleurs | ⬜ | K |
| Notebook | Notes dans le projet | Bloc-notes | ⬜ | N8 |
| Send | Envoi | Envois | 🟡 | H |
| Mastering IA | FL Cloud Mastering | Chaîne de mastering automatique locale | ⬜ | I |

## 12. Édition audio (éditeur d'échantillons)

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Enregistrer, éditer (couper, coller, fondus, normaliser, inverser) | Edison | Éditeur audio | ⬜ | L |
| Réduction de bruit | Edison denoise | Débruitage | ⬜ | L |
| Suppression de réverbération | Edison Deverb (2025) | Déréverbération | ⬜ | L |
| Séparation en pistes (voix, batterie, basse, reste) | Stem separation (IA) | Séparation de pistes (modèle local libre) | ⬜ | L |
| Détection de tempo et de temps | Beat detection | Détection du tempo | ⬜ | L |
| Découpe automatique aux attaques | Auto-slice | Découpe automatique | ⬜ | L |
| Conversion audio → notes | Convert to score | Audio vers MIDI | ⬜ | L |
| Vue spectrale | Spectrogram | Spectrogramme | ⬜ | L |
| Étirement, transposition sans changer la durée | Time stretch | Étirement de haute qualité | ⬜ | L |

## 13. Automatisation, contrôleurs, MIDI

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Automatiser n'importe quel paramètre | Create automation clip | Clic droit : automatiser | ⬜ | K |
| Courbes (lisses, en escalier, LFO) | Automation clip curves | Courbes | ⬜ | K |
| Enregistrer les mouvements | Record automation | Enregistrement | ⬜ | K |
| Éditeur d'événements | Event editor | Éditeur d'événements | ⬜ | K |
| Lier à un contrôleur, apprentissage MIDI | Link to controller, MIDI learn | Apprentissage MIDI | ⬜ | K |
| Liens multiples | Multilink | Liens multiples | ⬜ | K |
| Claviers et contrôleurs MIDI, plusieurs ports | MIDI settings | Entrées MIDI (midir) | ⬜ | K |
| MPE | MPE | MPE | ⬜ | K |
| MIDI 2.0 | (via VST3 3.8) | MIDI 2.0 | ⬜ | K |
| Scripts de contrôleurs | MIDI scripting (Python) | Scripts de contrôleurs | ⬜ | K |
| Télécommande sur téléphone | FL Studio Remote (2025) | Télécommande (réseau local) | ⬜ | O |
| Prise en charge de surfaces de contrôle | Akai Fire, FL Key… | Surfaces génériques | ⬜ | K |

## 14. Greffons (instruments et effets tiers)

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Formats | VST2, VST3, CLAP, AU | CLAP, VST3 (SDK MIT), AU sur macOS | ⬜ | M |
| Analyse des greffons installés | Plugin manager | Gestionnaire de greffons | ⬜ | M |
| Fenêtres des greffons | Plugin wrapper | Fenêtres natives | ⬜ | M |
| Isolement (un plantage ne coupe pas le son) | (pas d'isolement) | Processus séparés | ⬜ | M |
| Latence, multicœur, préréglages, état sauvegardé | Wrapper options | Mêmes réglages | ⬜ | M |
| Greffons de tiers par abonnement | FL Cloud Plugins | — | ⛔ → greffons gratuits installés par l'utilisateur | — |

## 15. Rendu, export, partage

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Export WAV | Export WAV | Rendu hors ligne | 🌐 | N8 |
| MP3, OGG, FLAC | Export formats | Mêmes formats | ⬜ | N |
| MIDI | Export MIDI | Export MIDI (16 canaux par port, sans plafond) | 🌐 | N8 |
| Pistes séparées | Split mixer tracks | Export par piste | ⬜ | N |
| Qualité de rendu (rééchantillonnage, dithering) | Render options | Options de rendu | ⬜ | N |
| Rendu d'une sélection, d'une boucle | Render selection | Rendu partiel | ⬜ | N |
| Distribution sur les plateformes | FL Cloud → DistroKid | — | ⛔ | — |

## 16. Composition assistée

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Progressions d'accords | Chord Progression tool | Progressions | ⬜ | F |
| Basse automatique | Bassline generator | Basse automatique | ⬜ | F |
| Démarreur d'idées | Loop Starter | Démarreur d'idées | ⬜ | N |
| Assistant qui agit dans le projet | Gopher (2026) | Assistant | ⬜ | O |
| Jouer avec l'orchestre (sa partie se tait, on la joue) | — (propre à song-audio) | Jouer la partie | 🌐 | N8 |
| Vues d'instruments (manche, harpe, pistons) | — (propre à song-audio) | Vues avancées | 🌐 | N7 |

## 17. Performance et robustesse

| Fonction | Référence FL Studio | Chez song-audio | État | Phase |
|---|---|---|---|---|
| Pilotes audio à faible latence | ASIO, WASAPI, CoreAudio | WASAPI, CoreAudio, ALSA/JACK ; ASIO (GPLv3) | 🟡 (WASAPI) | O |
| Fonctionnement sans carte son | — | Horloge silencieuse | ✅ | A |
| Changement d'appareil à chaud | Audio settings | Reconnexion automatique | ⬜ | O |
| Rendu multicœur | Multithreaded mixer | Rendu sur tous les cœurs | ✅ | ⟂ |
| Rien ne bloque le fil audio | — | Règle vérifiée (aucune allocation, aucun verrou) | ✅ | A, ⟂ |
| Budget de performance vérifié | — | Tests : 200 instruments dont 120 jouent | ✅ | ⟂ |
| Groupes de travail audio (Apple Silicon) | Audio Workgroups (2026) | Même API sur macOS | ⬜ | O |

## 18. Ce que song-audio fera en plus

| Fonction | Pourquoi |
|---|---|
| Aucune limite (canaux, pistes, effets) | FL Studio plafonne (500 pistes de mixage) |
| Linux | Non pris en charge par FL Studio |
| Version web gratuite, même format de projet | Ouvrir un projet partout |
| Greffons isolés | Un greffon qui plante ne coupe pas le son |
| Libre (GPL-3) et sans abonnement | Tout est inclus |
| Jouer avec l'orchestre, vues d'instruments pédagogiques | Apprendre en jouant |

## Sources

- [FL Studio 2025 : nouveautés (Image-Line)](https://www.image-line.com/fl-studio-news/fl-studio-2025-whats-new-2)
- [FL Studio 2025.2 (Image-Line)](https://www.image-line.com/news/fl-studio-2025-2)
- [FL Studio 2026 (AudioTechnology)](https://www.audiotechnology.com/by-brand/fl-studio/fl-studio-2026-released)
- [FL Studio 2026 : FLEX refait, Transmitter, Audio Logger (We Rave You)](https://weraveyou.com/2026/07/fl-studio-2026-new-features/)
- [FL Studio 2024 : FL Cloud et greffons (Attack Magazine)](https://www.attackmagazine.com/news/fl-studio-introduces-fl-studio-2024/)
- [Comparaison des éditions (Image-Line)](https://www.image-line.com/fl-studio/compare)
- [Manuel en ligne : nouveautés (Image-Line)](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_new_25.htm)
