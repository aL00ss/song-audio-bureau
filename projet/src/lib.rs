//! Modèle du projet de song-audio (version de bureau) : pur, sans son ni interface, testé.
//! Les fichiers ont le même format JSON que la version web.

pub mod demo;
pub mod gm;
pub mod lecture;
pub mod modele;
pub mod pas;
pub mod serialisation;
pub mod synthes;
pub mod temps;

pub use modele::{Canal, NoteMusicale, OptionsCanal, Pattern, PisteMixage, Projet, SourceCanal};
