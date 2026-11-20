//! Modèle du projet de song-audio (version de bureau) : pur, sans son ni interface, testé.
//! Les fichiers ont le même format JSON que la version web.

pub mod canaux;
pub mod clavier;
pub mod demo;
pub mod edition;
pub mod export_midi;
pub mod gm;
pub mod historique;
pub mod import_midi;
pub mod lecture;
pub mod mixage;
pub mod modele;
pub mod modeles;
pub mod morceaux;
pub mod pas;
pub mod playlist;
pub mod serialisation;
pub mod synthes;
pub mod temps;

pub use modele::{Canal, InfosProjet, NoteMusicale, OptionsCanal, Pattern, PisteMixage, Projet, SourceCanal};
