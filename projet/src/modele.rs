//! Modèle du projet : canaux (instruments), patterns, arrangement, pistes de mixage. Mêmes
//! structures et mêmes noms de champs JSON que la version web : un projet passe de l'une à
//! l'autre.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::gm::{CANAL_BATTERIE, nom_instrument};
use crate::temps::{SIGNATURE_DEFAUT, Signature, tics_par_mesure};

pub const VERSION_PROJET: u32 = 1;
pub const NOTE_DEFAUT_MELODIQUE: u8 = 60;
pub const NOTE_DEFAUT_BATTERIE: u8 = 36;
pub const REVERB_DEFAUT: f32 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMusicale {
    /// Début, en tics depuis le début du pattern.
    pub tic: u32,
    /// Durée en tics (au moins 1).
    pub duree: u32,
    pub hauteur: u8,
    /// 1 à 127.
    pub velocite: u8,
}

/// Ce qui produit le son d'un canal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceCanal {
    /// Banque General MIDI.
    #[default]
    Soundfont,
    /// Synthé soustractif intégré ; le programme choisit le préréglage.
    Soustractif,
    /// Échantillonneur : un instrument SFZ (chemin dans `Canal::instrument`).
    Echantillons,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canal {
    pub id: String,
    pub nom: String,
    /// Programme General MIDI (0 à 127), ou préréglage du synthé intégré.
    pub programme: u8,
    pub batterie: bool,
    /// 0 à 1.
    pub volume: f32,
    /// -1 (gauche) à 1 (droite).
    pub pan: f32,
    pub muet: bool,
    pub solo: bool,
    /// Piste du mixer qui reçoit ce canal (0 = maître).
    pub piste_mixage: usize,
    pub couleur: String,
    /// Hauteur des pas du séquenceur : Do central, ou le son de batterie.
    pub note_defaut: u8,
    pub source: SourceCanal,
    /// Fichier SFZ de l'échantillonneur (absent des autres sources, et des fichiers de la
    /// version web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub nom: String,
    /// Longueur en tics.
    pub longueur: u32,
    /// Notes par identifiant de canal.
    pub notes: BTreeMap<String, Vec<NoteMusicale>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipPlaylist {
    pub id: String,
    pub pattern: String,
    pub piste: u32,
    pub debut: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Egaliseur {
    /// Gain de chaque bande, en dB.
    pub grave: f32,
    pub medium: f32,
    pub aigu: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PisteMixage {
    pub index: usize,
    pub nom: String,
    /// Position du fader, 0 à 1 (0,8 = 0 dB).
    pub volume: f32,
    pub pan: f32,
    pub muet: bool,
    pub solo: bool,
    pub eq: Egaliseur,
    /// Envoi vers la réverbération commune, 0 à 1.
    pub reverb: f32,
}

pub fn creer_piste(index: usize, nom: &str) -> PisteMixage {
    PisteMixage {
        index,
        nom: nom.to_string(),
        volume: 0.8,
        pan: 0.0,
        muet: false,
        solo: false,
        eq: Egaliseur::default(),
        reverb: if index == 0 { 0.0 } else { REVERB_DEFAUT },
    }
}

/// Fiche du projet : auteur, genre et notes libres (le titre est le nom du projet). Absente des
/// fichiers de la version web, qui l'ignore.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InfosProjet {
    pub auteur: String,
    pub genre: String,
    pub notes: String,
}

impl InfosProjet {
    pub fn est_vide(&self) -> bool {
        self.auteur.is_empty() && self.genre.is_empty() && self.notes.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Projet {
    pub version: u32,
    pub nom: String,
    /// Noires par minute.
    pub tempo: f64,
    /// Swing global, de 0 à 1.
    pub swing: f64,
    pub signature: Signature,
    pub canaux: Vec<Canal>,
    pub patterns: Vec<Pattern>,
    pub pattern_actif: String,
    pub playlist: Vec<ClipPlaylist>,
    pub mixage: Vec<PisteMixage>,
    #[serde(default, skip_serializing_if = "InfosProjet::est_vide")]
    pub infos: InfosProjet,
}

const COULEURS: [&str; 8] = ["#f5a524", "#4cc38a", "#5aa9f5", "#e5679a", "#a78bfa", "#f97362", "#2dd4bf", "#facc15"];

/// Identifiant court et unique, de la même forme que la version web.
pub fn nouvel_id(prefixe: &str) -> String {
    static COMPTEUR: AtomicU64 = AtomicU64::new(0);
    let n = COMPTEUR.fetch_add(1, Ordering::Relaxed) + 1;
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    format!("{prefixe}-{}-{}", base36(ms), base36(n))
}

fn base36(mut n: u64) -> String {
    const CHIFFRES: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut s = Vec::new();
    while n > 0 {
        s.push(CHIFFRES[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

pub fn creer_pattern(nom: &str, longueur: u32) -> Pattern {
    Pattern { id: nouvel_id("pattern"), nom: nom.to_string(), longueur, notes: BTreeMap::new() }
}

pub fn creer_projet(nom: &str) -> Projet {
    let pattern = creer_pattern("Pattern 1", tics_par_mesure(SIGNATURE_DEFAUT));
    Projet {
        version: VERSION_PROJET,
        nom: nom.to_string(),
        tempo: 120.0,
        swing: 0.0,
        signature: SIGNATURE_DEFAUT,
        canaux: Vec::new(),
        pattern_actif: pattern.id.clone(),
        patterns: vec![pattern],
        playlist: Vec::new(),
        mixage: vec![creer_piste(0, "Maître")],
        infos: InfosProjet::default(),
    }
}

/// Options d'un nouveau canal (toutes facultatives).
#[derive(Clone, Debug, Default)]
pub struct OptionsCanal {
    pub nom: Option<String>,
    pub source: SourceCanal,
    pub programme: u8,
    pub batterie: bool,
    pub volume: Option<f32>,
    pub pan: f32,
    pub note_defaut: Option<u8>,
    /// Fichier SFZ, pour un canal échantillonneur.
    pub instrument: Option<String>,
}

/// Nom du son d'un canal mélodique (instrument General MIDI ou préréglage du synthé).
pub fn nom_du_son(source: SourceCanal, programme: u8) -> String {
    match source {
        SourceCanal::Soundfont => nom_instrument(programme).to_string(),
        SourceCanal::Soustractif => crate::synthes::PREREGLAGES_SOUSTRACTIF[programme as usize % crate::synthes::PREREGLAGES_SOUSTRACTIF.len()].nom.to_string(),
        SourceCanal::Echantillons => "Échantillonneur".to_string(),
    }
}

impl Projet {
    /// Ajoute un canal et sa piste de mixage ; renvoie l'identifiant du canal.
    pub fn ajouter_canal(&mut self, options: OptionsCanal) -> String {
        let id = nouvel_id("canal");
        let index = self.mixage.len();
        let source = if options.batterie { SourceCanal::Soundfont } else { options.source };
        let nom = options.nom.unwrap_or_else(|| if options.batterie { "Batterie".into() } else { nom_du_son(source, options.programme) });
        self.mixage.push(creer_piste(index, &nom));
        self.canaux.push(Canal {
            id: id.clone(),
            nom,
            programme: options.programme,
            batterie: options.batterie,
            volume: options.volume.unwrap_or(0.8),
            pan: options.pan,
            muet: false,
            solo: false,
            piste_mixage: index,
            couleur: COULEURS[self.canaux.len() % COULEURS.len()].into(),
            note_defaut: options.note_defaut.unwrap_or(if options.batterie { NOTE_DEFAUT_BATTERIE } else { NOTE_DEFAUT_MELODIQUE }),
            source,
            instrument: options.instrument,
        });
        id
    }

    pub fn canal(&self, id: &str) -> Option<&Canal> {
        self.canaux.iter().find(|c| c.id == id)
    }

    pub fn canal_mut(&mut self, id: &str) -> Option<&mut Canal> {
        self.canaux.iter_mut().find(|c| c.id == id)
    }

    pub fn pattern_actif(&self) -> &Pattern {
        self.patterns.iter().find(|p| p.id == self.pattern_actif).unwrap_or(&self.patterns[0])
    }

    pub fn pattern_actif_mut(&mut self) -> &mut Pattern {
        let index = self.patterns.iter().position(|p| p.id == self.pattern_actif).unwrap_or(0);
        &mut self.patterns[index]
    }

    /// Ajoute un clip du pattern à l'arrangement ; renvoie son identifiant.
    pub fn ajouter_clip(&mut self, pattern: &str, piste: u32, debut: u32) -> Option<String> {
        if !self.patterns.iter().any(|p| p.id == pattern) {
            return None;
        }
        let id = nouvel_id("clip");
        self.playlist.push(ClipPlaylist { id: id.clone(), pattern: pattern.to_string(), piste, debut });
        Some(id)
    }
}

/// Canaux qui sonnent : si au moins un est en solo, les solos ; sinon tous les non muets.
pub fn canaux_audibles(canaux: &[Canal]) -> HashSet<&str> {
    let solo = canaux.iter().any(|c| c.solo);
    canaux.iter().filter(|c| if solo { c.solo } else { !c.muet }).map(|c| c.id.as_str()).collect()
}

/// Pistes de mixage qui sonnent (le maître, index 0, n'est jamais concerné par le solo).
pub fn pistes_audibles(pistes: &[PisteMixage]) -> HashSet<usize> {
    let solo = pistes.iter().any(|p| p.index != 0 && p.solo);
    pistes.iter().filter(|p| p.index != 0 && if solo { p.solo } else { !p.muet }).map(|p| p.index).collect()
}

/// Canal MIDI de chaque canal du projet, pour l'export MIDI : la première batterie sur le canal
/// 10 (indice 9), les autres sur 0, 1, 2… en sautant le 9. Au-delà de 16, les numéros continuent
/// (16 canaux par port MIDI) : aucun plafond. Le moteur, lui, adresse les canaux par leur rang.
pub fn attribuer_canaux_midi(canaux: &[Canal]) -> HashMap<String, u32> {
    let mut attribution = HashMap::new();
    let premiere_batterie = canaux.iter().position(|c| c.batterie);
    let mut suivant = 0usize;
    for (i, canal) in canaux.iter().enumerate() {
        if Some(i) == premiere_batterie {
            attribution.insert(canal.id.clone(), CANAL_BATTERIE as u32);
            continue;
        }
        if suivant % 16 == CANAL_BATTERIE as usize {
            suivant += 1;
        }
        attribution.insert(canal.id.clone(), suivant as u32);
        suivant += 1;
    }
    attribution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canaux_pistes_et_canaux_midi() {
        let mut p = creer_projet("Essai");
        let piano = p.ajouter_canal(OptionsCanal::default());
        let batterie = p.ajouter_canal(OptionsCanal { batterie: true, ..Default::default() });
        let basse = p.ajouter_canal(OptionsCanal { programme: 33, ..Default::default() });
        assert_eq!(p.mixage.len(), 4);
        assert_eq!(p.canal(&piano).unwrap().nom, "Piano à queue");
        assert_eq!(p.canal(&batterie).unwrap().note_defaut, 36);
        let midi = attribuer_canaux_midi(&p.canaux);
        assert_eq!((midi[&piano], midi[&batterie], midi[&basse]), (0, 9, 1));
        p.canal_mut(&basse).unwrap().solo = true;
        assert_eq!(canaux_audibles(&p.canaux), HashSet::from([basse.as_str()]));
    }

    #[test]
    fn identifiants_uniques() {
        let a = nouvel_id("canal");
        let b = nouvel_id("canal");
        assert_ne!(a, b);
        assert!(a.starts_with("canal-"));
    }

    #[test]
    fn synthe_integre_et_batterie() {
        let mut p = creer_projet("Essai");
        let s = p.ajouter_canal(OptionsCanal { source: SourceCanal::Soustractif, programme: 1, ..Default::default() });
        assert_eq!(p.canal(&s).unwrap().nom, "Lead acide");
        // une batterie joue toujours la banque General MIDI
        let b = p.ajouter_canal(OptionsCanal { batterie: true, source: SourceCanal::Soustractif, ..Default::default() });
        assert_eq!(p.canal(&b).unwrap().source, SourceCanal::Soundfont);
    }

    #[test]
    fn des_centaines_de_canaux_sans_plafond() {
        let mut p = creer_projet("Grand orchestre");
        for i in 0..300u32 {
            p.ajouter_canal(OptionsCanal { programme: (i % 128) as u8, batterie: i % 50 == 0, ..Default::default() });
        }
        assert_eq!((p.canaux.len(), p.mixage.len()), (300, 301));
        let midi = attribuer_canaux_midi(&p.canaux);
        assert_eq!(midi.len(), 300);
        // les numéros MIDI ne retombent jamais sur un canal 10 d'un autre port
        assert!(midi.values().filter(|&&m| m != 9).all(|m| m % 16 != 9));
    }
}
