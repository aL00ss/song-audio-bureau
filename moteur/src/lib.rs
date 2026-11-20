//! Moteur sonore de song-audio (version de bureau) : synthétiseurs, mixer, sortie audio et
//! rendu hors ligne. Bibliothèque indépendante de l'interface, testée par `cargo test`.

pub mod ateliers;
pub mod banques;
pub mod effets;
pub mod noyau;
mod performance;
pub mod rendu;
pub mod sortie;
pub mod source;
pub mod soustractif;
pub mod transport;

pub use noyau::{Canal, Configuration, Evenement, Mesures, Message, Noyau, Piste, ReglageCanal, ReglagePiste, Stockage};
pub use rustysynth::SoundFont;
pub use sortie::Moteur;
pub use source::{Source, TypeSource};
pub use sortie::EtatLecture;
pub use transport::{ActionTransport, EtatTransport, NoteSession, Session, SessionPreparee, Transport};

use std::{fs::File, io::BufReader, path::Path, sync::Arc};

/// Charge une banque SoundFont 2 depuis le disque.
pub fn charger_banque(chemin: &Path) -> Result<Arc<SoundFont>, String> {
    let fichier = File::open(chemin).map_err(|e| format!("banque {} : {e}", chemin.display()))?;
    SoundFont::new(&mut BufReader::new(fichier))
        .map(Arc::new)
        .map_err(|e| format!("banque {} illisible : {e:?}", chemin.display()))
}
