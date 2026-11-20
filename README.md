# song-audio-bureau

Studio de musique de bureau (Windows, macOS, Linux), écrit en Rust : séquenceur pas à pas,
piano roll, mixer, arrangement, sans limite de canaux ni de pistes. Interface native (egui) et
moteur sonore dans le même programme.

La version web reste en ligne : https://al00ss.github.io/song-audio/ (même format de projet).

## Installer

Les installeurs (Windows, macOS, Linux) sont produits par la CI à chaque version
(`cargo packager -p studio --release`).

## Développer

```sh
cargo test --workspace --release   # moteur, modèle du projet, interface
cargo run -p studio --release      # lance l'application
```

## État

Voir [docs/PLAN.md](docs/PLAN.md) et le but final, [docs/FONCTIONS-CIBLES.md](docs/FONCTIONS-CIBLES.md).

## Licence

GPL-3.0 (voir [LICENSE](LICENSE)). Les banques de sons gardent leur propre licence (CC0,
CC BY…) ; elles sont créditées dans l'application.
