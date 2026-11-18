# Rapport : les bons langages pour un concurrent sérieux de FL Studio ?

Date : 2026-11-18. Projet : song-audio-bureau (version de bureau de song-audio).

## 1. La question

Le studio est construit avec une interface **TypeScript / React** affichée dans une fenêtre
**Tauri** (WebView2 sous Windows) et un moteur sonore en **Rust**. Est-ce le meilleur choix pour
viser toutes les fonctions de FL Studio, face à des concurrents installés depuis vingt ans
(FL Studio, Ableton Live, Bitwig Studio, Cubase, Studio One, Reaper) ?

## 2. Ce qu'exige un studio professionnel

| Exigence | Pourquoi c'est dur |
|---|---|
| Temps réel strict | Le fil audio rend un bloc toutes les 1 à 10 ms ; une allocation, un verrou ou un ramasse-miettes au mauvais moment = un craquement |
| Puissance de calcul | Des dizaines de pistes, d'instruments et d'effets ; il faut du code natif, vectorisé, réparti sur plusieurs cœurs |
| Greffons VST3 et CLAP | L'essentiel des sons des musiciens vient de greffons tiers ; il faut les accueillir, leurs fenêtres comprises |
| Stabilité | Un greffon qui plante ne doit pas emporter le projet |
| Interface très dense | Arrangement de milliers de clips, piano roll, formes d'onde, vumètres à 60 images/s |
| Trois systèmes | Windows, macOS, Linux |

## 3. Comment font les autres

| Logiciel | Langages | Remarque |
|---|---|---|
| FL Studio | Delphi (Object Pascal) et assembleur | Ni C++ ni « langage audio » : la réussite tient au produit, pas au langage |
| Bitwig Studio | C++ et Java (selon Wikipédia, non sourcé) | Greffons isolés dans des processus séparés : un plantage ne coupe pas le son |
| Ableton Live, Cubase, Reaper, Studio One | C++ | Bases de code de plusieurs millions de lignes, 15 à 25 ans d'âge |
| Ardour, LMMS, Zrythm | C++ ou C | Logiciels libres, interfaces natives (GTK, Qt) |

Leçon : **le langage n'a pas fait la différence**. FL Studio prouve qu'un langage hors des sentiers
battus suffit ; Bitwig prouve qu'un moteur natif peut cohabiter avec une interface dans un autre
langage. Ce qui compte : l'architecture (temps réel, isolement des greffons) et l'étendue du
produit (instruments, effets, contenu, années de finitions).

## 4. Le moteur : Rust est un excellent choix

- **Performances** : au niveau du C et du C++ (compilé, sans ramasse-miettes, SIMD).
- **Sûreté** : le compilateur interdit les accès concurrents mal protégés ; les règles du temps
  réel (pas d'allocation, pas de verrou bloquant dans le fil audio) restent à tenir, mais les
  bugs de mémoire qui font planter les studios en C++ disparaissent.
- **Écosystème mûr pour ce qu'il faut** : sortie audio (cpal), SoundFont (rustysynth),
  synthèse et effets (fundsp), accueil des greffons CLAP (**clack-host 0.2**, complet en
  fonctions, API encore mouvante) et VST3 (liaisons **vst3**, et plusieurs bibliothèques
  d'accueil).
- **Le verrou des licences a sauté** : depuis octobre 2025, le SDK **VST3 est sous licence MIT**
  (auparavant : contrat avec Steinberg obligatoire) et **ASIO est disponible sous GPLv3**,
  compatible avec notre GPL-3 : faible latence sous Windows possible.
- **Quand une bibliothèque C ou C++ est la meilleure** (sfizz pour le SFZ, Signalsmith Stretch
  pour l'étirement), Rust l'appelle directement (FFI) : on ne s'interdit rien.

Alternative examinée : tout en **C++ avec JUCE**. C'est la voie la plus éprouvée (la plupart des
greffons du marché), mais JUCE 8 est sous AGPLv3 ou payant (de 800 à 3 500 $ par développeur et
par version pour un usage commercial), et on perdrait la sûreté de Rust et le moteur déjà testé.
**Pas de raison d'en changer.**

## 5. L'interface : le vrai point faible, à condition de le traiter

Une interface web dans une WebView a des avantages réels : développement rapide, tests
automatisés riches (déjà 69 tests navigateur), même code pour le site web et le bureau. Ses
risques, pour un studio de ce niveau :

| Risque | Gravité | Réponse |
|---|---|---|
| Interroger le moteur par des appels JSON répétés (aujourd'hui toutes les 40 ms) : coût et retard | Moyenne | **Le moteur pousse ses mesures** sur un canal continu (Tauri `Channel`), à la cadence de l'affichage |
| Le DOM s'effondre avec des milliers d'éléments (clips, notes, formes d'onde) | Forte | Les vues lourdes **dessinées en Canvas/WebGL** (déjà le cas du piano roll) ; le DOM pour les réglages seulement |
| Les fenêtres des greffons (VST3, CLAP) ne s'insèrent pas dans une page web | Forte | Elles s'ouvrent dans **des fenêtres natives à part** (comme dans FL Studio, où elles flottent déjà) |
| WebKitGTK (Linux) plus lent que WebView2 | Moyenne | Mesurer ; si besoin, une vue critique pourra passer en natif |
| Logique métier dans l'interface | Moyenne | Le moteur détient tout ce qui sonne ; l'interface ne fait qu'éditer le projet et l'envoyer |

Alternatives examinées : interfaces natives Rust (egui, iced, Slint, Vizia) ; elles évoluent
vite mais aucune n'a encore porté une application de cette densité, et tout refaire coûterait
les neuf étapes d'interface déjà construites et testées. **On garde l'interface web, avec des
règles strictes** (tableau ci-dessus).

## 5 bis. Décision du 2026-11-18 : interface native en Rust

Après lecture du tableau ci-dessus, la décision est prise de **ne pas garder l'interface web
sur le bureau** : les deux risques graves (le DOM face à des milliers d'éléments, les fenêtres
de greffons impossibles à insérer) et la latence entre deux processus disparaissent avec une
interface native, dans le même programme que le moteur. Le prix est connu et accepté : les neuf
étapes d'interface sont à refaire.

Choix de la bibliothèque d'interface :

| Bibliothèque | Pour | Contre | Verdict |
|---|---|---|---|
| **egui** (eframe 0.37) | Mode immédiat : dessiner un piano roll, un arrangement, des formes d'onde est naturel ; très rapide (GPU) ; panneaux ancrables (egui_dock 0.21) ; **tests automatiques de l'interface** (egui_kittest, par l'accessibilité) ; MIT/Apache ; très actif | Look à travailler soi-même ; tout est redessiné à chaque image (sans coût notable sur GPU) | **Retenu** |
| Slint 1.19 | Déclaratif, beau par défaut | Le dessin libre (piano roll, arrangement) est moins direct | Écarté |
| iced 0.14 | Architecture claire (Elm) | Pas de panneaux ancrables officiels ; dessin libre plus lourd | Écarté |
| Vizia 0.4 | Pensé pour l'audio | Plus jeune, communauté plus petite | Écarté |

Conséquences :

- **Un seul programme** : interface et moteur dans le même processus. Plus d'appels entre
  processus : l'interface lit l'horloge, la position et les vumètres directement, et envoie ses
  ordres au fil audio par la même file sans verrou.
- **Le modèle du projet passe en Rust** (crate `projet`, pure et testée) ; le format des
  fichiers reste **le même JSON** que la version web : un projet passe de l'un à l'autre.
- **Les tests d'interface** se font avec egui_kittest (clics, saisie, lecture de l'écran par
  l'accessibilité), sans navigateur.
- **Le site web reste** dans le dépôt song-audio, avec son interface TypeScript.
- **Transition sans trou** : l'application Tauri actuelle reste construite tant que
  l'interface native n'a pas rattrapé ses fonctions ; ensuite, elle est retirée.

## 6. Le vrai enjeu : l'architecture du moteur

Le moteur actuel est un bon socle mais il est **figé** : chaque canal est forcément un lecteur
SoundFont. Or toutes les fonctions visées (synthétiseurs, échantillonneur, greffons, chaînes
d'effets, automatisation) demandent **un graphe audio ouvert** :

```text
canal ── source (SoundFont | synthé | échantillonneur | greffon) ── effets ──► piste ──► maître
```

Et un plantage de greffon tiers ne doit pas couper le son : **les greffons tiers tourneront dans
un processus séparé** (comme Bitwig), le moteur restant dans le sien.

## 7. Conclusion

1. **Rust pour le moteur : on garde.** C'est aujourd'hui l'un des deux meilleurs choix possibles
   (avec le C++), et le meilleur pour un petit nombre de développeurs, grâce à sa sûreté.
2. **Interface : analyse initiale « garder TypeScript / React dans Tauri, avec des règles
   strictes » ; décision finale (section 5 bis) : interface native en Rust avec egui**, dans le
   même programme que le moteur. Les neuf étapes d'interface sont refaites.
3. **Le langage n'est pas le facteur limitant.** Ce qui sépare un projet d'un concurrent
   sérieux, c'est l'architecture (graphe audio ouvert, greffons isolés, temps réel tenu) puis
   des années de fonctions et de contenu. FL Studio, écrit en Delphi, le prouve.
4. Être honnête sur l'objectif : la parité complète avec FL Studio représente des années de
   travail. La stratégie réaliste : un moteur irréprochable, l'accueil des greffons tôt (ce qui
   apporte d'un coup des milliers d'instruments et d'effets), puis les fonctions par ordre
   d'usage.

## 8. Modifications décidées, appliquées dans cette version

| # | Modification | Pourquoi |
|---|---|---|
| M1 | **Graphe audio ouvert** : chaque canal a une *source* interchangeable (trait `Source`) ; le lecteur SoundFont devient une source parmi d'autres | Condition de tout le reste (synthés, échantillonneur, greffons) |
| M2 | **Premier synthétiseur natif** (soustractif : 2 oscillateurs, filtre résonant, enveloppes), choisi dans le navigateur d'instruments | Prouve le graphe ouvert ; premier son « studio » sans banque |
| M3 | **Le moteur pousse ses mesures** (canal Tauri) au lieu d'être interrogé | Moins de coût, moins de retard, vumètres et position fluides |
| M4 | **Budget de performance testé** : un test exige que le moteur rende 16 instruments chargés plus vite que le temps réel, avec une marge | Le temps réel devient une exigence vérifiée, pas un espoir |
| M5 | Isolement des greffons dans un processus séparé, ASIO | Inscrits au plan (phases M, O) |
| M6 | **Interface native en Rust (egui)**, modèle du projet en Rust, même format de fichier | Décision du 2026-11-18 (section 5 bis) ; reconstruction en phases N0 à N9 |

## Sources

- [egui](https://github.com/emilk/egui), [egui_dock](https://github.com/Adanos020/egui_dock), [egui_kittest](https://github.com/emilk/egui/tree/master/crates/egui_kittest)

- [VST 3.8 sous licence MIT, ASIO sous GPLv3 (KVR Audio)](https://www.kvraudio.com/news/steinberg-moves-vst-3-sdk-to-mit-open-source-license-asio-now-gplv3-65179)
- [Communiqué de Steinberg, 29 octobre 2025](https://ocl-steinberg-live.steinberg.net/_storage/asset/819253/storage/master/Press%20Release%20-%202025-10-29%20-%20VST%203.8%20-%20EN.pdf)
- [Licence de JUCE 8 (AGPLv3 ou commerciale)](https://juce.com/legal/juce-8-licence/)
- [clack : accueil de greffons CLAP en Rust](https://github.com/prokopyl/clack)
- [Bitwig : accueil des greffons et protection contre les plantages](https://www.bitwig.com/learnings/plug-in-hosting-crash-protection-in-bitwig-studio-20/)
- [FL Studio (Wikipédia) : écrit en Delphi et assembleur](https://en.wikipedia.org/wiki/FL_Studio)
- [Bitwig Studio (Wikipédia)](https://en.wikipedia.org/wiki/Bitwig_Studio)
