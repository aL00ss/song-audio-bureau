//! Catalogue des morceaux fournis (domaine public), le même fichier que la version web.

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Morceau {
    pub id: String,
    pub titre: String,
    pub compositeur: String,
    pub oeuvre: String,
    pub annee: String,
    /// Durée en secondes.
    pub duree: f64,
    pub credit: String,
    pub licence: String,
    /// Chemin relatif au dossier des ressources (« morceaux/… .mid »).
    pub fichier: String,
}

pub fn lire_catalogue(json: &str) -> Result<Vec<Morceau>, String> {
    serde_json::from_str(json).map_err(|e| format!("Catalogue des morceaux illisible : {e}"))
}

/// Nom de famille du compositeur (« Ludwig van Beethoven » -> « Beethoven »).
pub fn nom_court(compositeur: &str) -> &str {
    compositeur.rsplit(' ').next().unwrap_or(compositeur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_fourni_lisible_et_fichiers_presents() {
        let dossier = concat!(env!("CARGO_MANIFEST_DIR"), "/../ressources/");
        let catalogue = lire_catalogue(&std::fs::read_to_string(format!("{dossier}morceaux/catalogue.json")).unwrap()).unwrap();
        assert_eq!(catalogue.len(), 5);
        for m in &catalogue {
            assert!(std::path::Path::new(&format!("{dossier}{}", m.fichier)).exists(), "{} manque", m.fichier);
            assert_eq!(m.licence, "Domaine public");
        }
        assert_eq!(nom_court(&catalogue[0].compositeur), "Beethoven");
    }
}
