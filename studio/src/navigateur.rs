//! Le navigateur : morceaux fournis, fichiers du disque, instruments.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use projet::morceaux::Morceau;

/// Un fichier ou un dossier affiché dans la liste des fichiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entree {
    pub chemin: PathBuf,
    pub nom: String,
    pub dossier: bool,
}

/// Fichiers que le studio sait ouvrir.
pub fn fichier_utile(chemin: &Path) -> bool {
    let nom = chemin.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    nom.ends_with(".mid") || nom.ends_with(".midi") || nom.ends_with(".json") || nom.ends_with(".sfz") || nom.ends_with(".flp")
}

/// Contenu d'un dossier : sous-dossiers puis fichiers utiles, par ordre alphabétique, sans les
/// éléments cachés.
pub fn lister(dossier: &Path) -> Vec<Entree> {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut entrees: Vec<Entree> = lecture
        .flatten()
        .filter_map(|e| {
            let chemin = e.path();
            let nom = e.file_name().to_string_lossy().to_string();
            if nom.starts_with('.') {
                return None;
            }
            let est_dossier = chemin.is_dir();
            (est_dossier || fichier_utile(&chemin)).then_some(Entree { chemin, nom, dossier: est_dossier })
        })
        .collect();
    entrees.sort_by_key(|e| (!e.dossier, e.nom.to_lowercase()));
    entrees
}

/// Dossier de départ de la liste des fichiers : Musique, sinon Documents, sinon le dossier
/// personnel.
pub fn dossier_de_depart() -> PathBuf {
    let maison = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    for nom in ["Music", "Musique", "Documents"] {
        let d = maison.join(nom);
        if d.is_dir() {
            return d;
        }
    }
    maison
}

pub struct Navigateur {
    pub morceaux: Vec<Morceau>,
    pub dossier: PathBuf,
    contenus: HashMap<PathBuf, Vec<Entree>>,
}

impl Navigateur {
    pub fn new(morceaux: Vec<Morceau>) -> Self {
        Navigateur { morceaux, dossier: dossier_de_depart(), contenus: HashMap::new() }
    }

    /// Contenu d'un dossier (lu une fois, gardé jusqu'à « Actualiser »).
    pub fn contenu(&mut self, dossier: &Path) -> Vec<Entree> {
        self.contenus.entry(dossier.to_path_buf()).or_insert_with(|| lister(dossier)).clone()
    }

    pub fn actualiser(&mut self) {
        self.contenus.clear();
    }

    pub fn changer_dossier(&mut self, dossier: PathBuf) {
        self.dossier = dossier;
        self.contenus.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liste_des_fichiers_utiles_dossiers_d_abord() {
        let racine = std::env::temp_dir().join(format!("song-audio-liste-{}", std::process::id()));
        std::fs::create_dir_all(racine.join("Zèbre")).unwrap();
        std::fs::create_dir_all(racine.join("albums")).unwrap();
        std::fs::create_dir_all(racine.join(".cache")).unwrap();
        for f in ["b.mid", "A.MIDI", "projet.song-audio.json", "notes.txt", ".cache.mid"] {
            std::fs::write(racine.join(f), b"x").unwrap();
        }
        let noms: Vec<(String, bool)> = lister(&racine).into_iter().map(|e| (e.nom, e.dossier)).collect();
        assert_eq!(
            noms,
            vec![
                ("albums".into(), true),
                ("Zèbre".into(), true),
                ("A.MIDI".into(), false),
                ("b.mid".into(), false),
                ("projet.song-audio.json".into(), false)
            ]
        );
        std::fs::remove_dir_all(&racine).ok();
    }
}
