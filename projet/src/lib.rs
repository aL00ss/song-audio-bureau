//! Modèle du projet de song-audio (version de bureau) : pur, sans son ni interface, testé.
//! Les fichiers ont le même format JSON que la version web.

pub mod canaux;
pub mod demo;
pub mod gm;
pub mod import_midi;
pub mod lecture;
pub mod modele;
pub mod morceaux;
pub mod pas;
pub mod recherche;
pub mod serialisation;
pub mod synthes;
pub mod temps;

pub use modele::{Canal, NoteMusicale, OptionsCanal, Pattern, PisteMixage, Projet, SourceCanal};
