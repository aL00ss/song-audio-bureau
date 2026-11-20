//! song-audio, version de bureau native : interface egui et moteur sonore dans le même
//! programme.

pub mod app;
pub mod arrangement;
pub mod banques;
pub mod coquille;
pub mod fichiers;
pub mod instrument;
pub mod instruments;
pub mod jeu;
pub mod mixeur;
pub mod navigateur;
pub mod piano_roll;
pub mod pont;
pub mod rack;
pub mod theme;

pub use app::Studio;
