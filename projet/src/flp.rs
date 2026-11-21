//! Lecture des projets FL Studio (.flp). Le format n'est pas documenté par son éditeur ; sa
//! structure est connue grâce au travail de rétro-ingénierie de PyFLP (demberto, GPL-3.0), dont
//! cette lecture reprend la connaissance du format (pas le code).
//!
//! Un fichier .flp : un en-tête « FLhd » (format, nombre de canaux, résolution), puis un bloc
//! « FLdt » fait d'une suite d'événements. Chaque événement a un identifiant d'un octet ; sa
//! taille en découle : 0 à 63 un octet, 64 à 127 deux octets, 128 à 191 quatre octets, 192 et
//! au-delà une longueur variable (texte ou données). Les éléments du projet (canaux, patterns,
//! pistes du mixer, arrangements) se suivent : certains événements en ouvrent un nouveau.

use std::collections::BTreeMap;

// ---------------------------------------------------------------- identifiants d'événements

const MOT: u8 = 64;
const DOUBLE: u8 = 128;
const TEXTE: u8 = 192;
const DONNEES: u8 = 208;

// projet
const TEMPO_GROSSIER: u8 = MOT + 2;
const TEMPO_FIN: u8 = MOT + 29;
const TEMPO: u8 = DOUBLE + 28;
const TITRE: u8 = TEXTE + 2;
const COMMENTAIRES: u8 = TEXTE + 3;
const ADRESSE: u8 = TEXTE + 5;
const VERSION_FL: u8 = TEXTE + 7;
const GENRE: u8 = TEXTE + 14;
const ARTISTES: u8 = TEXTE + 15;
// canaux
const CANAL_NOUVEAU: u8 = MOT;
const CANAL_ACTIF: u8 = 0;
const CANAL_TYPE: u8 = 21;
const CANAL_ROUTE: u8 = 22;
const CANAL_VOLUME_OCTET: u8 = 2;
const CANAL_PAN_OCTET: u8 = 3;
const CANAL_VOLUME_MOT: u8 = MOT + 8;
const CANAL_PAN_MOT: u8 = MOT + 9;
const CANAL_ENFANT: u8 = MOT + 30;
const CANAL_NOTE_RACINE: u8 = DOUBLE + 7;
const CANAL_NOM: u8 = TEXTE;
const CANAL_ECHANTILLON: u8 = TEXTE + 4;
const CANAL_NIVEAUX: u8 = DONNEES + 11;
const GREFFON_NOM_INTERNE: u8 = TEXTE + 9;
const GREFFON_NOM: u8 = TEXTE + 11;
const GROUPE_AFFICHAGE_NOM: u8 = TEXTE + 39;
// patterns
const PATTERN_NOUVEAU: u8 = MOT + 1;
const PATTERN_NOM: u8 = TEXTE + 1;
const PATTERN_LONGUEUR: u8 = DOUBLE + 36;
const PATTERN_NOTES: u8 = DONNEES + 16;
const PATTERN_COURANT: u8 = MOT + 3;
// arrangements
const SIGNATURE_NUMERATEUR: u8 = 17;
const SIGNATURE_TEMPS: u8 = 18;
const ARRANGEMENT_NOUVEAU: u8 = MOT + 35;
const ARRANGEMENT_COURANT: u8 = MOT + 36;
const ARRANGEMENT_NOM: u8 = TEXTE + 49;
const ARRANGEMENT_LISTE: u8 = DONNEES + 25;
const PISTE_NOM: u8 = TEXTE + 47;
const PISTE_DONNEES: u8 = DONNEES + 30;
// mixer
const MIXER_PARAMETRES: u8 = DONNEES + 17;
const INSERT_NOM: u8 = TEXTE + 12;
const INSERT_SORTIE: u8 = DOUBLE + 19;
const EMPLACEMENT_INDEX: u8 = MOT + 34;

/// Textes toujours en UTF-16 (même dans les versions où les autres sont en ASCII).
const TEXTES_UTF16: [u8; 3] = [ARRANGEMENT_NOM, GROUPE_AFFICHAGE_NOM, PISTE_NOM];

/// Base des numéros de pattern dans les éléments de l'arrangement.
const BASE_PATTERN: u16 = 20480;

/// Un événement du fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Evenement {
    pub id: u8,
    pub valeur: Valeur,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Valeur {
    Octet(u8),
    Mot(u16),
    Double(u32),
    Donnees(Vec<u8>),
}

impl Valeur {
    pub fn entier(&self) -> u32 {
        match self {
            Valeur::Octet(v) => *v as u32,
            Valeur::Mot(v) => *v as u32,
            Valeur::Double(v) => *v,
            Valeur::Donnees(_) => 0,
        }
    }

    pub fn donnees(&self) -> &[u8] {
        match self {
            Valeur::Donnees(d) => d,
            _ => &[],
        }
    }
}

/// En-tête et événements d'un fichier .flp.
#[derive(Clone, Debug, PartialEq)]
pub struct FichierFlp {
    pub format: i16,
    pub nombre_canaux: u16,
    /// Tics par noire.
    pub ppq: u16,
    pub evenements: Vec<Evenement>,
}

fn u16_a(o: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([o[i], o[i + 1]])
}
fn u32_a(o: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([o[i], o[i + 1], o[i + 2], o[i + 3]])
}
fn i32_a(o: &[u8], i: usize) -> i32 {
    u32_a(o, i) as i32
}

/// Lit l'en-tête et la suite d'événements.
pub fn lire_evenements(octets: &[u8]) -> Result<FichierFlp, String> {
    if octets.len() < 22 || &octets[0..4] != b"FLhd" {
        return Err("ce n'est pas un projet FL Studio (en-tête absent)".into());
    }
    if u32_a(octets, 4) != 6 {
        return Err("en-tête FL Studio de taille inattendue".into());
    }
    let (format, nombre_canaux, ppq) = (u16_a(octets, 8) as i16, u16_a(octets, 10), u16_a(octets, 12));
    if &octets[14..18] != b"FLdt" {
        return Err("bloc de données FL Studio absent".into());
    }
    if ppq == 0 {
        return Err("résolution (PPQ) nulle".into());
    }
    let taille = u32_a(octets, 18) as usize;
    let fin = (22 + taille).min(octets.len());
    let mut evenements = Vec::new();
    let mut i = 22;
    let tronque = || "fichier FL Studio tronqué".to_string();
    while i < fin {
        let id = octets[i];
        i += 1;
        let valeur = match id {
            0..=63 => {
                let v = *octets.get(i).ok_or_else(tronque)?;
                i += 1;
                Valeur::Octet(v)
            }
            64..=127 => {
                if i + 2 > fin {
                    return Err(tronque());
                }
                i += 2;
                Valeur::Mot(u16_a(octets, i - 2))
            }
            128..=191 => {
                if i + 4 > fin {
                    return Err(tronque());
                }
                i += 4;
                Valeur::Double(u32_a(octets, i - 4))
            }
            _ => {
                // longueur variable : 7 bits par octet, le bit de poids fort annonce la suite
                let mut n: usize = 0;
                let mut decalage = 0;
                loop {
                    let o = *octets.get(i).ok_or_else(tronque)?;
                    i += 1;
                    n |= ((o & 0x7F) as usize) << decalage;
                    decalage += 7;
                    if o & 0x80 == 0 || decalage > 28 {
                        break;
                    }
                }
                if i + n > fin {
                    return Err(tronque());
                }
                i += n;
                Valeur::Donnees(octets[i - n..i].to_vec())
            }
        };
        evenements.push(Evenement { id, valeur });
    }
    Ok(FichierFlp { format, nombre_canaux, ppq, evenements })
}

// ---------------------------------------------------------------- le projet FL Studio lu

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TypeCanalFlp {
    /// Échantillonneur intégré (un son).
    #[default]
    Echantillonneur,
    /// Générateur natif ou clip audio.
    Natif,
    Couche,
    /// Greffon (instrument VST ou natif).
    Instrument,
    Automatisation,
    Inconnu(u8),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CanalFlp {
    pub iid: u16,
    pub genre: TypeCanalFlp,
    pub nom: Option<String>,
    pub greffon: Option<String>,
    pub greffon_nom: Option<String>,
    pub echantillon: Option<String>,
    /// 0 à 12800 (10000 par défaut).
    pub volume: Option<u32>,
    /// 0 à 12800 (6400 au centre).
    pub pan: Option<i32>,
    /// Piste du mixer qui le reçoit.
    pub piste: Option<i32>,
    pub enfants: Vec<u16>,
    pub note_racine: Option<u32>,
    pub actif: bool,
}

impl CanalFlp {
    /// Nom à montrer : le nom donné, sinon celui du greffon, sinon celui du son.
    pub fn nom_affiche(&self) -> String {
        self.nom
            .clone()
            .or_else(|| self.greffon_nom.clone())
            .or_else(|| self.greffon.clone())
            .or_else(|| self.echantillon.as_ref().map(|e| e.rsplit(['\\', '/']).next().unwrap_or(e).trim_end_matches(".wav").to_string()))
            .unwrap_or_else(|| format!("Canal {}", self.iid + 1))
    }
}

/// Une note d'un pattern FL Studio (valeurs du fichier).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteFlp {
    pub position: u32,
    pub canal: u16,
    pub longueur: u32,
    pub hauteur: u16,
    /// 0 à 240 (120 : juste).
    pub hauteur_fine: u8,
    /// 0 à 128 (64 par défaut).
    pub relachement: u8,
    /// 0 à 128 (64 au centre).
    pub pan: u8,
    /// 0 à 128 (100 par défaut).
    pub velocite: u8,
    pub glisse: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PatternFlp {
    pub iid: u16,
    pub nom: Option<String>,
    pub longueur: Option<u32>,
    pub notes: Vec<NoteFlp>,
}

/// Un élément de l'arrangement : un pattern ou un canal (clip audio, automatisation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementFlp {
    Pattern(u16),
    Canal(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipFlp {
    pub position: u32,
    pub element: ElementFlp,
    pub longueur: u32,
    /// Piste de l'arrangement (0 : la première).
    pub piste: u32,
    pub debut_decale: f32,
    pub fin_decalee: f32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArrangementFlp {
    pub iid: u16,
    pub nom: Option<String>,
    pub clips: Vec<ClipFlp>,
    pub pistes: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct InsertFlp {
    pub nom: Option<String>,
    /// 0 à 16000 (12800 : 0 dB).
    pub volume: Option<i32>,
    /// -6400 à 6400.
    pub pan: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProjetFlp {
    pub ppq: u16,
    pub version: Option<String>,
    pub tempo: Option<f64>,
    pub titre: Option<String>,
    pub artistes: Option<String>,
    pub genre: Option<String>,
    pub commentaires: Option<String>,
    pub adresse: Option<String>,
    pub signature: (u8, u8),
    pub canaux: Vec<CanalFlp>,
    pub patterns: Vec<PatternFlp>,
    pub pattern_courant: Option<u16>,
    pub arrangements: Vec<ArrangementFlp>,
    pub arrangement_courant: Option<u16>,
    pub inserts: Vec<InsertFlp>,
}

/// Version de FL Studio (majeure, mineure) écrite dans le fichier.
fn version_majeure(v: &str) -> (u32, u32) {
    let mut morceaux = v.split('.').map(|m| m.trim().parse::<u32>().unwrap_or(0));
    (morceaux.next().unwrap_or(0), morceaux.next().unwrap_or(0))
}

/// Texte d'un événement : UTF-16 (FL Studio 11.5 et après, et quelques textes toujours), sinon
/// ASCII ; le zéro final est retiré.
fn texte(id: u8, d: &[u8], utf16: bool) -> String {
    if utf16 || TEXTES_UTF16.contains(&id) {
        let unites: Vec<u16> = d.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&unites).trim_end_matches('\0').to_string()
    } else {
        d.iter().map(|&o| o as char).collect::<String>().trim_end_matches('\0').to_string()
    }
}

/// Où vont les événements qui suivent.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Contexte {
    Projet,
    Canal(usize),
    Pattern(u16),
    Arrangement(usize),
    Mixer,
}

/// Regroupe les événements en éléments du projet.
pub fn lire_projet(octets: &[u8]) -> Result<ProjetFlp, String> {
    let f = lire_evenements(octets)?;
    let mut p = ProjetFlp { ppq: f.ppq, signature: (4, 4), ..Default::default() };
    // la version vient en premier : elle dit comment lire les textes
    let utf16 = f
        .evenements
        .iter()
        .find(|e| e.id == VERSION_FL)
        .map(|e| {
            let v = texte(e.id, e.valeur.donnees(), false);
            p.version = Some(v.clone());
            let (maj, min) = version_majeure(&v);
            (maj, min) >= (11, 5)
        })
        .unwrap_or(true);
    let mut patterns: BTreeMap<u16, PatternFlp> = BTreeMap::new();
    let mut contexte = Contexte::Projet;
    let mut insert = InsertFlp::default();
    let mut inserts_lus = 0usize;
    let mut tempo_grossier: Option<u32> = None;
    let mut tempo_fin: Option<u32> = None;
    for e in &f.evenements {
        let v = &e.valeur;
        let txt = || texte(e.id, v.donnees(), utf16);
        match e.id {
            // ---- projet
            TEMPO => p.tempo = Some(v.entier() as f64 / 1000.0),
            TEMPO_GROSSIER => tempo_grossier = Some(v.entier()),
            TEMPO_FIN => tempo_fin = Some(v.entier()),
            TITRE => p.titre = Some(txt()),
            COMMENTAIRES => p.commentaires = Some(txt().replace('\r', "\n")),
            ADRESSE => p.adresse = Some(txt()),
            GENRE => p.genre = Some(txt()),
            ARTISTES => p.artistes = Some(txt()),
            SIGNATURE_NUMERATEUR => p.signature.0 = v.entier() as u8,
            SIGNATURE_TEMPS => p.signature.1 = v.entier() as u8,
            // ---- canaux
            CANAL_NOUVEAU => {
                p.canaux.push(CanalFlp { iid: v.entier() as u16, actif: true, ..Default::default() });
                contexte = Contexte::Canal(p.canaux.len() - 1);
            }
            // ---- patterns
            PATTERN_NOUVEAU => {
                let iid = v.entier() as u16;
                patterns.entry(iid).or_insert_with(|| PatternFlp { iid, ..Default::default() });
                contexte = Contexte::Pattern(iid);
            }
            PATTERN_COURANT => p.pattern_courant = Some(v.entier() as u16),
            // ---- arrangements
            ARRANGEMENT_NOUVEAU => {
                p.arrangements.push(ArrangementFlp { iid: v.entier() as u16, ..Default::default() });
                contexte = Contexte::Arrangement(p.arrangements.len() - 1);
            }
            ARRANGEMENT_COURANT => p.arrangement_courant = Some(v.entier() as u16),
            // ---- mixer : chaque piste se termine par sa sortie
            MIXER_PARAMETRES => {
                contexte = Contexte::Mixer;
                lire_parametres_mixer(v.donnees(), &mut p.inserts);
            }
            EMPLACEMENT_INDEX => contexte = Contexte::Mixer,
            INSERT_NOM => {
                contexte = Contexte::Mixer;
                insert.nom = Some(txt());
            }
            INSERT_SORTIE => {
                contexte = Contexte::Mixer;
                // les réglages arrivent à part (paramètres du mixer) : on range le nom à sa place
                let fini = std::mem::take(&mut insert);
                if inserts_lus >= p.inserts.len() {
                    p.inserts.resize(inserts_lus + 1, InsertFlp::default());
                }
                p.inserts[inserts_lus].nom = fini.nom;
                inserts_lus += 1;
            }
            _ => match contexte {
                Contexte::Canal(k) => lire_evenement_canal(&mut p.canaux[k], e, utf16),
                Contexte::Pattern(iid) => {
                    let pat = patterns.get_mut(&iid).expect("pattern ouvert");
                    match e.id {
                        PATTERN_NOM => pat.nom = Some(txt()),
                        PATTERN_LONGUEUR => pat.longueur = Some(v.entier()),
                        PATTERN_NOTES => pat.notes.extend(lire_notes(v.donnees())),
                        _ => {}
                    }
                }
                Contexte::Arrangement(k) => {
                    let a = &mut p.arrangements[k];
                    match e.id {
                        ARRANGEMENT_NOM => a.nom = Some(txt()),
                        ARRANGEMENT_LISTE => a.clips.extend(lire_clips(v.donnees())),
                        PISTE_NOM => {
                            if let Some(derniere) = a.pistes.last_mut() {
                                *derniere = Some(txt());
                            }
                        }
                        PISTE_DONNEES => a.pistes.push(None),
                        _ => {}
                    }
                }
                Contexte::Projet | Contexte::Mixer => {}
            },
        }
    }
    if p.tempo.is_none() {
        if let Some(g) = tempo_grossier {
            p.tempo = Some(g as f64 + tempo_fin.unwrap_or(0) as f64 / 1000.0);
        }
    }
    // pistes de l'arrangement : rangées à l'envers dans le fichier
    for a in p.arrangements.iter_mut() {
        let total = if a.pistes.is_empty() { 500 } else { a.pistes.len() as u32 };
        for c in a.clips.iter_mut() {
            c.piste = total.saturating_sub(1).saturating_sub(c.piste);
        }
    }
    p.patterns = patterns.into_values().collect();
    Ok(p)
}

fn lire_evenement_canal(c: &mut CanalFlp, e: &Evenement, utf16: bool) {
    let v = &e.valeur;
    let txt = || texte(e.id, v.donnees(), utf16);
    match e.id {
        CANAL_ACTIF => c.actif = v.entier() != 0,
        CANAL_TYPE => {
            c.genre = match v.entier() {
                0 => TypeCanalFlp::Echantillonneur,
                2 => TypeCanalFlp::Natif,
                3 => TypeCanalFlp::Couche,
                4 => TypeCanalFlp::Instrument,
                5 => TypeCanalFlp::Automatisation,
                n => TypeCanalFlp::Inconnu(n as u8),
            }
        }
        CANAL_ROUTE => c.piste = Some(v.entier() as u8 as i8 as i32),
        CANAL_VOLUME_OCTET | CANAL_VOLUME_MOT => {
            if c.volume.is_none() {
                c.volume = Some(v.entier())
            }
        }
        CANAL_PAN_OCTET | CANAL_PAN_MOT => {
            if c.pan.is_none() {
                c.pan = Some(v.entier() as i32)
            }
        }
        CANAL_NIVEAUX => {
            let d = v.donnees();
            if d.len() >= 8 {
                c.pan = Some(i32_a(d, 0));
                c.volume = Some(u32_a(d, 4));
            }
        }
        CANAL_ENFANT => c.enfants.push(v.entier() as u16),
        CANAL_NOTE_RACINE => c.note_racine = Some(v.entier()),
        CANAL_NOM => c.nom = Some(txt()).filter(|n| !n.is_empty()),
        CANAL_ECHANTILLON => c.echantillon = Some(txt()).filter(|n| !n.is_empty()),
        GREFFON_NOM_INTERNE => c.greffon = Some(txt()).filter(|n| !n.is_empty()),
        GREFFON_NOM => c.greffon_nom = Some(txt()).filter(|n| !n.is_empty()),
        _ => {}
    }
}

/// Notes d'un pattern : 24 octets par note.
fn lire_notes(d: &[u8]) -> Vec<NoteFlp> {
    d.chunks_exact(24)
        .map(|n| NoteFlp {
            position: u32_a(n, 0),
            glisse: u16_a(n, 4) & 0x08 != 0,
            canal: u16_a(n, 6),
            longueur: u32_a(n, 8),
            hauteur: u16_a(n, 12),
            hauteur_fine: n[16],
            relachement: n[18],
            pan: n[20],
            velocite: n[21],
        })
        .collect()
}

/// Éléments de l'arrangement : 32 octets chacun (60 depuis FL Studio 21).
fn lire_clips(d: &[u8]) -> Vec<ClipFlp> {
    let taille = if !d.is_empty() && d.len() % 60 == 0 { 60 } else { 32 };
    d.chunks_exact(taille)
        .map(|c| {
            let (base, index) = (u16_a(c, 4), u16_a(c, 6));
            let element = if index > base && base == BASE_PATTERN { ElementFlp::Pattern(index - base) } else { ElementFlp::Canal(index) };
            ClipFlp {
                position: u32_a(c, 0),
                element,
                longueur: u32_a(c, 8),
                piste: u16_a(c, 12) as u32,
                debut_decale: f32::from_le_bytes([c[24], c[25], c[26], c[27]]),
                fin_decalee: f32::from_le_bytes([c[28], c[29], c[30], c[31]]),
            }
        })
        .collect()
}

/// Réglages des pistes du mixer : 12 octets par réglage (piste, paramètre, valeur).
fn lire_parametres_mixer(d: &[u8], inserts: &mut Vec<InsertFlp>) {
    for r in d.chunks_exact(12) {
        let id = r[4];
        let numero = ((u16_a(r, 6) >> 6) & 0x7F) as usize;
        let valeur = i32_a(r, 8);
        if numero >= inserts.len() {
            inserts.resize(numero + 1, InsertFlp::default());
        }
        match id {
            192 => inserts[numero].volume = Some(valeur),
            193 => inserts[numero].pan = Some(valeur),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fichier_d_essai() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/donnees/flp/PyFLP - FL 20.8.4.flp")).unwrap()
    }

    #[test]
    fn evenements_du_fichier_d_essai() {
        let f = lire_evenements(&fichier_d_essai()).unwrap();
        assert_eq!((f.format, f.nombre_canaux, f.ppq), (0, 19, 96));
        assert!(f.evenements.len() > 1000);
    }

    #[test]
    #[ignore = "inspection : cargo test -p projet inspecter_flp -- --ignored --nocapture"]
    fn inspecter_flp() {
        let p = lire_projet(&fichier_d_essai()).unwrap();
        println!("version {:?} tempo {:?} titre {:?} signature {:?}", p.version, p.tempo, p.titre, p.signature);
        for c in &p.canaux {
            println!("canal {} {:?} nom {:?} greffon {:?}/{:?} échantillon {:?} vol {:?} pan {:?} piste {:?} enfants {:?}", c.iid, c.genre, c.nom, c.greffon, c.greffon_nom, c.echantillon, c.volume, c.pan, c.piste, c.enfants);
        }
        for pat in &p.patterns {
            println!("pattern {} {:?} longueur {:?} notes {}", pat.iid, pat.nom, pat.longueur, pat.notes.len());
        }
        for a in &p.arrangements {
            println!("arrangement {} {:?} : {} clips, {} pistes nommées {:?}", a.iid, a.nom, a.clips.len(), a.pistes.len(), a.pistes.iter().flatten().take(8).collect::<Vec<_>>());
            for c in a.clips.iter().take(12) {
                println!("   {c:?}");
            }
        }
        for (k, i) in p.inserts.iter().enumerate().take(8) {
            println!("insert {k} {i:?}");
        }
        println!("{} inserts", p.inserts.len());
    }
}

