//! Le navigateur : recherche de fichiers MIDI en ligne (BitMidi), morceaux fournis, fichiers du
//! disque, instruments. Les accès au réseau se font dans des fils à part ; leurs résultats
//! reviennent par un canal que l'interface relève à chaque image.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use projet::morceaux::Morceau;
use projet::recherche::{ResultatRecherche, adresse_recherche, lire_resultats};

/// Délai après la dernière frappe avant de chercher (comme la version web).
pub const DELAI_FRAPPE: Duration = Duration::from_millis(350);
/// Résultats affichés au plus.
pub const RESULTATS_MAX: usize = 30;
/// Taille maximale d'un fichier MIDI téléchargé.
const TAILLE_MAX: u64 = 8 * 1024 * 1024;

/// Accès au réseau (remplaçable dans les tests).
pub trait Reseau: Send + Sync {
    fn chercher(&self, requete: &str) -> Result<Vec<ResultatRecherche>, String>;
    fn telecharger(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// BitMidi, par HTTPS.
pub struct BitMidi {
    agent: ureq::Agent,
}

impl Default for BitMidi {
    fn default() -> Self {
        let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
        BitMidi { agent: ureq::Agent::new_with_config(config) }
    }
}

impl Reseau for BitMidi {
    fn chercher(&self, requete: &str) -> Result<Vec<ResultatRecherche>, String> {
        let mut reponse = self.agent.get(&adresse_recherche(requete, 0)).header("User-Agent", "song-audio").call().map_err(|e| e.to_string())?;
        let texte = reponse.body_mut().with_config().limit(TAILLE_MAX).read_to_string().map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&texte).map_err(|e| e.to_string())?;
        Ok(lire_resultats(&json))
    }

    fn telecharger(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut reponse = self.agent.get(url).header("User-Agent", "song-audio").call().map_err(|e| e.to_string())?;
        reponse.body_mut().with_config().limit(TAILLE_MAX).read_to_vec().map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EtatRecherche {
    Repos,
    EnCours,
    Resultats(Vec<ResultatRecherche>),
    Erreur,
}

/// Ce qu'un fil d'arrière-plan rapporte.
pub enum Rapport {
    Resultats { generation: u64, resultat: Result<Vec<ResultatRecherche>, String> },
    Fichier { nom: String, resultat: Result<Vec<u8>, String> },
}

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
    nom.ends_with(".mid") || nom.ends_with(".midi") || nom.ends_with(".json")
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
    reseau: Arc<dyn Reseau>,
    envoi: Sender<Rapport>,
    reception: Receiver<Rapport>,
    pub requete: String,
    derniere_frappe: Option<Instant>,
    /// Numéro de la dernière recherche lancée : les réponses plus anciennes sont ignorées.
    generation: u64,
    pub recherche: EtatRecherche,
    /// Morceau en cours de téléchargement.
    pub en_chargement: Option<String>,
    pub morceaux: Vec<Morceau>,
    pub dossier: PathBuf,
    contenus: HashMap<PathBuf, Vec<Entree>>,
}

impl Navigateur {
    pub fn new(reseau: Arc<dyn Reseau>, morceaux: Vec<Morceau>) -> Self {
        let (envoi, reception) = channel();
        Navigateur {
            reseau,
            envoi,
            reception,
            requete: String::new(),
            derniere_frappe: None,
            generation: 0,
            recherche: EtatRecherche::Repos,
            en_chargement: None,
            morceaux,
            dossier: dossier_de_depart(),
            contenus: HashMap::new(),
        }
    }

    /// La requête a changé (frappe) : la recherche partira après le délai.
    pub fn requete_changee(&mut self) {
        self.derniere_frappe = Some(Instant::now());
        if self.requete.trim().chars().count() < 2 {
            self.recherche = EtatRecherche::Repos;
            self.derniere_frappe = None;
            self.generation += 1; // une réponse encore en route sera ignorée
        }
    }

    /// Lance la recherche si le délai de frappe est écoulé. Renvoie vrai s'il faut revenir plus
    /// tard (une recherche est en attente du délai).
    pub fn avancer(&mut self, contexte: Option<&eframe::egui::Context>) -> bool {
        let Some(frappe) = self.derniere_frappe else { return false };
        if frappe.elapsed() < DELAI_FRAPPE {
            return true;
        }
        self.derniere_frappe = None;
        self.generation += 1;
        self.recherche = EtatRecherche::EnCours;
        let (generation, requete, reseau, envoi) = (self.generation, self.requete.trim().to_string(), self.reseau.clone(), self.envoi.clone());
        let contexte = contexte.cloned();
        std::thread::spawn(move || {
            let resultat = reseau.chercher(&requete);
            let _ = envoi.send(Rapport::Resultats { generation, resultat });
            if let Some(c) = contexte {
                c.request_repaint();
            }
        });
        false
    }

    /// Télécharge un résultat dans un fil à part.
    pub fn telecharger(&mut self, resultat: &ResultatRecherche, contexte: Option<&eframe::egui::Context>) {
        if self.en_chargement.is_some() {
            return;
        }
        self.en_chargement = Some(resultat.titre.clone());
        let (nom, url, reseau, envoi) = (resultat.titre.clone(), resultat.url.clone(), self.reseau.clone(), self.envoi.clone());
        let contexte = contexte.cloned();
        std::thread::spawn(move || {
            let resultat = reseau.telecharger(&url);
            let _ = envoi.send(Rapport::Fichier { nom, resultat });
            if let Some(c) = contexte {
                c.request_repaint();
            }
        });
    }

    /// Relève ce que les fils ont rapporté ; renvoie les fichiers téléchargés (nom, contenu ou
    /// erreur) à ouvrir.
    pub fn relever(&mut self) -> Vec<(String, Result<Vec<u8>, String>)> {
        let mut fichiers = Vec::new();
        while let Ok(rapport) = self.reception.try_recv() {
            match rapport {
                Rapport::Resultats { generation, resultat } if generation == self.generation => {
                    self.recherche = match resultat {
                        Ok(mut r) => {
                            r.truncate(RESULTATS_MAX);
                            EtatRecherche::Resultats(r)
                        }
                        Err(_) => EtatRecherche::Erreur,
                    };
                }
                Rapport::Resultats { .. } => {}
                Rapport::Fichier { nom, resultat } => {
                    if self.en_chargement.as_deref() == Some(nom.as_str()) {
                        self.en_chargement = None;
                    }
                    fichiers.push((nom, resultat));
                }
            }
        }
        fichiers
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

    #[test]
    #[ignore = "réseau : lancer avec RESEAU=1 cargo test -- --ignored"]
    fn vraie_recherche_bitmidi() {
        if std::env::var("RESEAU").is_err() {
            return;
        }
        let b = BitMidi::default();
        let r = b.chercher("chopin").unwrap();
        assert!(!r.is_empty());
        let octets = b.telecharger(&r[0].url).unwrap();
        assert!(projet::import_midi::depuis_midi(&octets, &r[0].titre).is_ok());
    }
}
