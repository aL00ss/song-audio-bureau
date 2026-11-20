# song-audio-bureau

Studio de musique de bureau (Windows, macOS, Linux), entièrement en Rust : moteur sonore et
interface native (egui) dans le même programme. Le plan et l'avancement sont dans
[docs/PLAN.md](docs/PLAN.md), le but final dans [docs/FONCTIONS-CIBLES.md](docs/FONCTIONS-CIBLES.md) :
les lire avant toute nouvelle étape.

## Organisation

- `moteur` : le son (graphe sans limite de canaux ni de pistes, transport, rendu hors ligne).
- `projet` : le modèle du projet, pur et testé (même format JSON que la version web).
- `studio` : l'application egui ; tests d'interface par egui_kittest, avec le vrai moteur.
- `ressources` : banque General MIDI, morceaux fournis, icônes ; `ressources/sons` : banques
  plus fidèles gardées pour la phase D.

## Conventions

- Code, noms et commentaires en **français**.
- Seulement des fonctions que FL Studio a aussi (référence pour l'étendue des fonctions) ; jamais
  son nom, son logo, son habillage ni son contenu d'usine.
- Aucune limite fixe (canaux, pistes, clips) ; listes virtualisées dans l'interface.
- Moteur : le fil audio ne doit jamais attendre (pas d'allocation ni de verrou bloquant dans le
  rappel).
- Banques de sons : uniquement des licences qui permettent la redistribution (CC0, CC BY,
  « sans restriction ») ; crédits affichés dans l'application.
- Chaque étape se termine par : `cargo test --workspace --release` vert (garde-fou des
  caractères de la police compris), construction sans avertissement, installeur
  (`cargo packager -p studio --release --formats nsis`) reconstruit et vérifié, mise à jour de
  docs/PLAN.md et docs/FONCTIONS-CIBLES.md, puis un commit sans aucune ligne d'attribution.
