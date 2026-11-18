# song-audio-bureau

Version de bureau de song-audio (Tauri 2, moteur sonore Rust). Le plan et l'avancement sont
dans [docs/PLAN.md](docs/PLAN.md) : le lire avant toute nouvelle étape.

## Conventions

- Code, noms et commentaires en **français**, comme song-audio.
- L'interface React vient de song-audio ; le son ne passe plus par SpessaSynth mais par le
  moteur Rust (commandes et événements Tauri). Garder les deux dépôts proches pour reporter
  les corrections de l'interface.
- Moteur Rust : logique pure (événements, temps en tics, mélange) testée par `cargo test` ;
  le fil audio ne doit jamais attendre (pas d'allocation ni de verrou bloquant dans le rappel).
- Banques de sons : uniquement des licences qui permettent la redistribution (CC0, CC BY,
  « sans restriction ») ; crédits affichés dans l'application. Jamais de banque ni de nom,
  logo ou habillage d'un logiciel commercial.
- Chaque étape se termine par : tests verts (`cargo test`, tests de l'interface), construction
  sans erreur, mise à jour de docs/PLAN.md, puis un commit sans aucune ligne d'attribution.
