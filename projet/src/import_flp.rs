//! Import d'un projet FL Studio (.flp) : canaux (et couches), patterns et leurs notes, arrangement,
//! pistes du mixer, tempo, signature, infos du projet. Ce que song-audio ne peut pas reprendre tel
//! quel est remplacé au plus proche et listé dans un rapport : greffons (le nom est gardé, un son
//! General MIDI proche joue à la place), échantillons introuvables, clips d'automatisation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::flp::{CanalFlp, ElementFlp, NoteFlp, TypeCanalFlp, lire_projet};
use crate::modele::{InfosProjet, NoteMusicale, OptionsCanal, Projet, SourceCanal, creer_piste, creer_projet, creer_pattern};
use crate::pas::DUREE_PAS;
use crate::temps::{Signature, TICS_PAR_NOIRE, TICS_PAR_PAS, tics_par_mesure};

/// Le projet importé et ce qui a dû être remplacé ou laissé de côté.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportFlp {
    pub projet: Projet,
    pub rapport: Vec<String>,
}

/// Son de batterie General MIDI d'après un nom (« Kick », « Snare 2 », « Open Hat »…).
pub fn batterie_d_apres_nom(nom: &str) -> Option<u8> {
    let n = nom.to_lowercase();
    let a = |mots: &[&str]| mots.iter().any(|m| n.contains(m));
    Some(if a(&["kick", "bd ", "bass drum", "grosse caisse"]) || n.starts_with("bd") {
        36
    } else if a(&["snare", "sd ", "caisse claire"]) {
        38
    } else if a(&["clap"]) {
        39
    } else if a(&["rim"]) {
        37
    } else if a(&["open hat", "openhat", "oh ", "hat open", "open hh"]) {
        46
    } else if a(&["hat", "hh", "charleston"]) {
        42
    } else if a(&["crash"]) {
        49
    } else if a(&["ride"]) {
        51
    } else if a(&["tom"]) {
        45
    } else if a(&["cowbell"]) {
        56
    } else if a(&["shaker", "maraca"]) {
        70
    } else if a(&["tamb"]) {
        54
    } else if a(&["perc", "conga", "bongo"]) {
        63
    } else {
        return None;
    })
}

/// Instrument General MIDI le plus proche d'après des noms (canal, greffon), à défaut d'autre son.
pub fn programme_d_apres_nom(noms: &[&str]) -> u8 {
    let n = noms.join(" ").to_lowercase();
    let a = |mots: &[&str]| mots.iter().any(|m| n.contains(m));
    if a(&["808", "sub bass"]) {
        38
    } else if a(&["bass", "basse", "boobass"]) {
        33
    } else if a(&["rhodes", "e-piano", "epiano", "electric piano", "wurli"]) {
        4
    } else if a(&["piano", "keys", "fl keys"]) {
        0
    } else if a(&["organ", "orgue"]) {
        16
    } else if a(&["guitar", "guitare", "plucked"]) {
        25
    } else if a(&["cello", "violoncelle"]) {
        42
    } else if a(&["violin", "violon"]) {
        40
    } else if a(&["string", "corde"]) {
        48
    } else if a(&["choir", "vox", "voice", "voix", "vocal"]) {
        52
    } else if a(&["trumpet", "trompette"]) {
        56
    } else if a(&["brass", "cuivre"]) {
        61
    } else if a(&["sax"]) {
        65
    } else if a(&["flute", "flûte"]) {
        73
    } else if a(&["bell", "cloche"]) {
        14
    } else if a(&["pluck"]) {
        46
    } else if a(&["pad", "nappe", "harmor", "flex"]) {
        89
    } else if a(&["lead", "saw", "sytrus", "3x osc", "3xosc", "gms", "synth", "arp"]) {
        81
    } else {
        0
    }
}

/// Cherche un échantillon référencé par le projet : dans les données de FL Studio installé,
/// sinon dans le dossier du projet (et ses sous-dossiers), d'après son nom.
pub fn trouver_echantillon(chemin: &str, dossier_projet: Option<&Path>, dossiers_fl: &[PathBuf]) -> Option<PathBuf> {
    let normal = chemin.replace('\\', "/");
    if let Some(reste) = normal.strip_prefix("%FLStudioFactoryData%/") {
        for d in dossiers_fl {
            let c = d.join(reste);
            if c.exists() {
                return Some(c);
            }
        }
    } else if Path::new(&normal).exists() {
        return Some(PathBuf::from(normal));
    }
    let nom = normal.rsplit('/').next()?.to_string();
    let dossier = dossier_projet?;
    fn chercher(d: &Path, nom: &str, profondeur: usize) -> Option<PathBuf> {
        let c = d.join(nom);
        if c.exists() {
            return Some(c);
        }
        if profondeur == 0 {
            return None;
        }
        for e in std::fs::read_dir(d).ok()?.flatten() {
            if e.path().is_dir() {
                if let Some(t) = chercher(&e.path(), nom, profondeur - 1) {
                    return Some(t);
                }
            }
        }
        None
    }
    chercher(dossier, &nom, 2)
}

/// Dossiers des données d'usine de FL Studio, s'il est installé (Windows).
pub fn dossiers_fl_studio() -> Vec<PathBuf> {
    let mut r = Vec::new();
    for base in ["C:/Program Files/Image-Line", "C:/Program Files (x86)/Image-Line"] {
        if let Ok(lecture) = std::fs::read_dir(base) {
            for e in lecture.flatten() {
                if e.file_name().to_string_lossy().starts_with("FL Studio") {
                    r.push(e.path());
                }
            }
        }
    }
    r.sort();
    r.reverse(); // la version la plus récente d'abord
    r
}

/// Convertit un temps du fichier (à sa résolution) en tics du projet.
fn tics(t: u32, ppq: u16) -> u32 {
    ((t as u64 * TICS_PAR_NOIRE as u64 + ppq as u64 / 2) / ppq.max(1) as u64) as u32
}

/// Ce que devient un canal FL Studio.
enum Destin {
    /// Un canal de song-audio (son identifiant), et la note qui doit remplacer toutes les
    /// hauteurs (batterie d'un seul son), ou le décalage de hauteur (échantillon dont la note
    /// racine n'est pas le do central).
    Canal { id: String, hauteur_fixe: Option<u8>, transposition: i32 },
    Ignore,
}

/// Importe un fichier .flp. `dossier_projet` sert à retrouver les échantillons ; `dossiers_fl`
/// sont les dossiers de FL Studio installé (pour ses sons d'usine).
pub fn depuis_flp(octets: &[u8], nom_fichier: &str, dossier_projet: Option<&Path>, dossiers_fl: &[PathBuf]) -> Result<ImportFlp, String> {
    let flp = lire_projet(octets)?;
    let ppq = flp.ppq;
    let mut rapport = Vec::new();
    let nom = flp.titre.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| nom_fichier.to_string());
    let mut p = creer_projet(&nom);
    p.tempo = flp.tempo.unwrap_or(140.0).clamp(10.0, 999.0);
    let (num, temps) = flp.signature;
    p.signature = Signature { numerateur: (num as u32).clamp(1, 32), denominateur: (temps as u32).clamp(1, 32) };
    let notes = [flp.commentaires.clone(), flp.adresse.clone()].into_iter().flatten().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n\n");
    p.infos = InfosProjet { auteur: flp.artistes.clone().unwrap_or_default(), genre: flp.genre.clone().unwrap_or_default(), notes };

    // canaux joués comme des clips audio dans l'arrangement
    let arrangement = flp
        .arrangements
        .iter()
        .find(|a| Some(a.iid) == flp.arrangement_courant)
        .or_else(|| flp.arrangements.iter().find(|a| !a.clips.is_empty()))
        .or(flp.arrangements.first());
    let clips_audio: std::collections::HashSet<u16> =
        arrangement.map(|a| a.clips.iter().filter_map(|c| if let ElementFlp::Canal(i) = c.element { Some(i) } else { None }).collect()).unwrap_or_default();

    // ---- canaux
    let mut destins: HashMap<u16, Destin> = HashMap::new();
    let (mut automatisations, mut greffons, mut manquants) = (0, Vec::new(), Vec::new());
    for c in &flp.canaux {
        let destin = convertir_canal(&mut p, c, dossier_projet, dossiers_fl, clips_audio.contains(&c.iid), &mut automatisations, &mut greffons, &mut manquants);
        destins.insert(c.iid, destin);
    }
    // couches : leurs enfants, une fois tous les canaux créés
    for c in flp.canaux.iter().filter(|c| c.genre == TypeCanalFlp::Couche) {
        let Some(Destin::Canal { id, .. }) = destins.get(&c.iid) else { continue };
        let enfants: Vec<String> = c
            .enfants
            .iter()
            .filter_map(|e| match destins.get(e) {
                Some(Destin::Canal { id: enfant, .. }) if p.canal(enfant).is_some_and(|x| x.source != SourceCanal::Couche) => Some(enfant.clone()),
                _ => None,
            })
            .collect();
        if let Some(couche) = p.canal_mut(id) {
            couche.enfants = enfants;
        }
    }
    if automatisations > 0 {
        rapport.push(format!("{automatisations} clip{} d'automatisation laissé{} de côté (l'automatisation n'existe pas encore dans song-audio).", s(automatisations), s(automatisations)));
    }
    for (canal, greffon, remplacant) in &greffons {
        rapport.push(format!("« {canal} » : le greffon {greffon} est remplacé par « {remplacant} » (General MIDI)."));
    }
    for (canal, echantillon, remplacant) in &manquants {
        rapport.push(format!("« {canal} » : échantillon introuvable ({echantillon}), remplacé par « {remplacant} »."));
    }

    // ---- mixer : les pistes de FL Studio, une pour une (la 0 est le maître)
    let derniere_utile = flp
        .inserts
        .iter()
        .enumerate()
        .filter(|(i, ins)| *i == 0 || ins.nom.is_some() || ins.volume.is_some_and(|v| v != 12800) || ins.pan.is_some_and(|v| v != 0))
        .map(|(i, _)| i)
        .chain(flp.canaux.iter().filter_map(|c| c.piste.filter(|&k| k >= 0).map(|k| k as usize)))
        .max()
        .unwrap_or(0)
        .min(flp.inserts.len().saturating_sub(1));
    p.mixage = (0..=derniere_utile)
        .map(|i| {
            let ins = flp.inserts.get(i).cloned().unwrap_or_default();
            let mut piste = creer_piste(i, &ins.nom.clone().unwrap_or_else(|| if i == 0 { "Maître".into() } else { format!("Piste {i}") }));
            if let Some(v) = ins.volume {
                // FL Studio : 12800 = 0 dB ; ici le fader à 0,8 = 0 dB, le gain suit le carré
                piste.volume = (0.8 * (v.max(0) as f32 / 12800.0).sqrt()).min(1.0);
            }
            if let Some(pan) = ins.pan {
                piste.pan = (pan as f32 / 6400.0).clamp(-1.0, 1.0);
            }
            piste
        })
        .collect();
    for c in &flp.canaux {
        if let Some(Destin::Canal { id, .. }) = destins.get(&c.iid) {
            let piste = c.piste.filter(|&k| k >= 0 && (k as usize) < p.mixage.len()).unwrap_or(0) as usize;
            if let Some(x) = p.canal_mut(id) {
                x.piste_mixage = piste;
            }
        }
    }

    // ---- patterns et leurs notes
    let mesure = tics_par_mesure(p.signature);
    let mut ids_patterns: HashMap<u16, String> = HashMap::new();
    let (mut glissees, mut notes_sans_canal) = (0, 0);
    p.patterns.clear();
    for pat in &flp.patterns {
        let mut motif = creer_pattern(&pat.nom.clone().unwrap_or_else(|| format!("Pattern {}", pat.iid)), mesure);
        let mut fin_max = 0;
        for n in &pat.notes {
            glissees += usize::from(n.glisse);
            match destins.get(&n.canal) {
                Some(Destin::Canal { id, hauteur_fixe, transposition }) => {
                    let note = convertir_note(n, ppq, *hauteur_fixe, *transposition);
                    fin_max = fin_max.max(note.tic + note.duree);
                    motif.notes.entry(id.clone()).or_default().push(note);
                }
                Some(Destin::Ignore) => {}
                None => notes_sans_canal += 1,
            }
        }
        for notes in motif.notes.values_mut() {
            notes.sort_by_key(|n| (n.tic, n.hauteur));
        }
        let longueur = pat.longueur.filter(|&l| l > 0).map(|l| tics(l, ppq)).unwrap_or(0);
        motif.longueur = longueur.max(fin_max.div_ceil(mesure) * mesure).max(mesure);
        ids_patterns.insert(pat.iid, motif.id.clone());
        p.patterns.push(motif);
    }
    if glissees > 0 {
        rapport.push(format!("{glissees} note{} glissée{} jouée{} comme des notes ordinaires.", s(glissees), s(glissees), s(glissees)));
    }
    if notes_sans_canal > 0 {
        rapport.push(format!("{notes_sans_canal} note{} sur des canaux absents du fichier, ignorée{}.", s(notes_sans_canal), s(notes_sans_canal)));
    }

    // ---- arrangement : clips de pattern ; clips audio rejoués par l'échantillonneur de leur canal
    let mut clips_audio_patterns: HashMap<(u16, u32), String> = HashMap::new();
    if let Some(a) = arrangement {
        for c in &a.clips {
            let debut = tics(c.position, ppq) as i64;
            match c.element {
                ElementFlp::Pattern(iid) => {
                    if let Some(id) = ids_patterns.get(&iid) {
                        p.ajouter_clip(id, c.piste, debut as u32);
                    }
                }
                ElementFlp::Canal(iid) => {
                    let Some(Destin::Canal { id: canal, transposition, .. }) = destins.get(&iid) else { continue };
                    let longueur = tics(c.longueur, ppq).max(1);
                    // un pattern par canal et par longueur de clip : une note qui joue le son
                    let cle = (iid, longueur);
                    let pattern = clips_audio_patterns.entry(cle).or_insert_with(|| {
                        let nom = format!("Clip audio : {}", p.canal(canal).map(|x| x.nom.clone()).unwrap_or_default());
                        let mut motif = creer_pattern(&nom, longueur.div_ceil(TICS_PAR_PAS) * TICS_PAR_PAS);
                        let hauteur = (60 + *transposition).clamp(0, 127) as u8;
                        motif.notes.insert(canal.clone(), vec![NoteMusicale { tic: 0, duree: longueur, hauteur, velocite: 100, ..Default::default() }]);
                        let id = motif.id.clone();
                        p.patterns.push(motif);
                        id
                    });
                    p.ajouter_clip(&pattern.clone(), c.piste, debut as u32);
                }
            }
        }
        let autres = flp.arrangements.iter().filter(|x| x.iid != a.iid && !x.clips.is_empty()).count();
        if autres > 0 {
            rapport.push(format!("{autres} autre{} arrangement{} non repris (song-audio n'en a qu'un par projet).", s(autres), s(autres)));
        }
    }
    if p.patterns.is_empty() {
        p.patterns.push(creer_pattern("Pattern 1", mesure));
    }
    let courant = flp.pattern_courant.and_then(|i| ids_patterns.get(&i)).cloned();
    p.pattern_actif = courant.filter(|id| p.patterns.iter().any(|x| &x.id == id)).unwrap_or_else(|| p.patterns[0].id.clone());
    Ok(ImportFlp { projet: p, rapport })
}

fn s(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

#[allow(clippy::too_many_arguments)]
fn convertir_canal(
    p: &mut Projet,
    c: &CanalFlp,
    dossier_projet: Option<&Path>,
    dossiers_fl: &[PathBuf],
    joue_en_clip_audio: bool,
    automatisations: &mut usize,
    greffons: &mut Vec<(String, String, String)>,
    manquants: &mut Vec<(String, String, String)>,
) -> Destin {
    let nom = c.nom_affiche();
    let volume = c.volume.map(|v| (v as f32 / 12800.0).clamp(0.0, 1.0));
    let pan = c.pan.map(|v| ((v - 6400) as f32 / 6400.0).clamp(-1.0, 1.0)).unwrap_or(0.0);
    let options = |o: OptionsCanal| OptionsCanal { nom: Some(nom.clone()), volume, pan, ..o };
    let greffon = c.greffon.clone().unwrap_or_default();
    match c.genre {
        TypeCanalFlp::Automatisation => {
            *automatisations += 1;
            return Destin::Ignore;
        }
        TypeCanalFlp::Couche => {
            let id = p.ajouter_canal(options(OptionsCanal { source: SourceCanal::Couche, ..Default::default() }));
            return Destin::Canal { id, hauteur_fixe: None, transposition: 0 };
        }
        _ => {}
    }
    if greffon == "MIDI Out" {
        greffons.push((nom.clone(), "MIDI Out".into(), "aucun son (sortie MIDI)".into()));
        return Destin::Ignore;
    }
    // un échantillon : joué tel quel s'il est retrouvé
    if let Some(chemin) = &c.echantillon {
        if let Some(trouve) = trouver_echantillon(chemin, dossier_projet, dossiers_fl) {
            let racine = c.note_racine.map(|r| r as i32).unwrap_or(60).clamp(0, 127);
            let id = p.ajouter_canal(options(OptionsCanal { source: SourceCanal::Echantillons, instrument: Some(trouve.to_string_lossy().to_string()), ..Default::default() }));
            // l'échantillon sonne juste au do central : on décale les notes selon la note racine
            return Destin::Canal { id, hauteur_fixe: None, transposition: 60 - racine };
        }
        let court = chemin.rsplit(['\\', '/']).next().unwrap_or(chemin).to_string();
        if let Some(son) = batterie_d_apres_nom(&format!("{nom} {court}")) {
            let id = p.ajouter_canal(options(OptionsCanal { batterie: true, note_defaut: Some(son), ..Default::default() }));
            manquants.push((nom.clone(), court, crate::gm::nom_son_batterie(son)));
            return Destin::Canal { id, hauteur_fixe: Some(son), transposition: 0 };
        }
        let programme = programme_d_apres_nom(&[&nom, &court]);
        let id = p.ajouter_canal(options(OptionsCanal { programme, ..Default::default() }));
        manquants.push((nom.clone(), court, crate::gm::nom_instrument(programme).to_string()));
        return Destin::Canal { id, hauteur_fixe: None, transposition: 0 };
    }
    let _ = joue_en_clip_audio;
    // greffons de batterie : un kit (les notes gardent leur hauteur) ou un seul son
    let noms = [nom.as_str(), greffon.as_str(), c.greffon_nom.as_deref().unwrap_or("")];
    if ["FPC", "Drumaxx", "Drumpad", "Slicex", "Fruity Slicer"].iter().any(|g| greffon == *g) {
        let id = p.ajouter_canal(options(OptionsCanal { batterie: true, ..Default::default() }));
        greffons.push((nom.clone(), greffon.clone(), "batterie General MIDI".into()));
        return Destin::Canal { id, hauteur_fixe: None, transposition: 0 };
    }
    if let Some(son) = batterie_d_apres_nom(&noms.join(" ")) {
        let id = p.ajouter_canal(options(OptionsCanal { batterie: true, note_defaut: Some(son), ..Default::default() }));
        if !greffon.is_empty() {
            greffons.push((nom.clone(), greffon.clone(), crate::gm::nom_son_batterie(son)));
        }
        return Destin::Canal { id, hauteur_fixe: Some(son), transposition: 0 };
    }
    let programme = programme_d_apres_nom(&noms);
    let id = p.ajouter_canal(options(OptionsCanal { programme, ..Default::default() }));
    if !greffon.is_empty() || c.genre == TypeCanalFlp::Instrument {
        let source = if greffon.is_empty() { "de ce canal".to_string() } else { greffon.clone() };
        greffons.push((nom.clone(), source, crate::gm::nom_instrument(programme).to_string()));
    }
    Destin::Canal { id, hauteur_fixe: None, transposition: 0 }
}

fn convertir_note(n: &NoteFlp, ppq: u16, hauteur_fixe: Option<u8>, transposition: i32) -> NoteMusicale {
    let tic = tics(n.position, ppq);
    // les notes du séquenceur pas à pas ont une longueur nulle : un pas
    let duree = if n.longueur == 0 { if tic % TICS_PAR_PAS == 0 { DUREE_PAS } else { TICS_PAR_PAS / 2 } } else { tics(n.longueur, ppq).max(1) };
    let hauteur = hauteur_fixe.map(|h| h as i32).unwrap_or(n.hauteur as i32 + transposition).clamp(0, 127) as u8;
    let echelle = |v: u8| ((v as f32 * 127.0 / 128.0).round() as i32).clamp(1, 127) as u8;
    NoteMusicale {
        tic,
        duree,
        hauteur,
        velocite: echelle(n.velocite),
        pan: (((n.pan as i32 - 64) * 100) / 64).clamp(-100, 100) as i8,
        fin: ((n.hauteur_fine as i32 - 120) * 10).clamp(-1200, 1200) as i16,
        relachement: echelle(n.relachement),
        decalage: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialisation::{deserialiser, serialiser};

    fn fichier_d_essai() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/donnees/flp/PyFLP - FL 20.8.4.flp")).unwrap()
    }

    #[test]
    fn projet_d_essai_de_pyflp() {
        let i = depuis_flp(&fichier_d_essai(), "essai", None, &[]).unwrap();
        let p = &i.projet;
        // infos et tempo
        assert_eq!((p.nom.as_str(), p.tempo), ("PyFLP Test FLP", 69.42));
        assert_eq!((p.infos.auteur.as_str(), p.infos.genre.as_str()), ("demberto", "Testing..."));
        assert!(p.infos.notes.starts_with("This is a testing FLP used by PyFLP"));
        assert!(p.infos.notes.contains("https://github.com/demberto/PyFLP"));
        assert_eq!((p.signature.numerateur, p.signature.denominateur), (4, 4));
        // canaux : 19 dans le fichier, moins l'automatisation et la sortie MIDI
        assert_eq!(p.canaux.len(), 17, "{:?}", p.canaux.iter().map(|c| &c.nom).collect::<Vec<_>>());
        let canal = |nom: &str| p.canaux.iter().find(|c| c.nom == nom).unwrap_or_else(|| panic!("canal {nom}"));
        assert_eq!(canal("BooBass").programme, 33, "une basse");
        assert!(canal("Fruit Kick").batterie && canal("Fruit Kick").note_defaut == 36);
        assert_eq!(canal("Zero Volume").volume, 0.0);
        assert_eq!(canal("Full Volume").volume, 1.0);
        assert_eq!((canal("100% L").pan, canal("100% R").pan), (-1.0, 1.0));
        assert!((canal("Sampler").volume - 10000.0 / 12800.0).abs() < 1e-6);
        // échantillon d'usine absent ici : une grosse caisse General MIDI
        assert!(canal("22in Kick").batterie && canal("22in Kick").note_defaut == 36);
        // la couche et ses enfants (sans l'automatisation ni la sortie MIDI)
        let couche = canal("Layer");
        assert_eq!(couche.source, SourceCanal::Couche);
        assert_eq!(couche.enfants.len(), 15);
        assert!(couche.enfants.iter().all(|e| p.canal(e).is_some_and(|c| c.source != SourceCanal::Couche)));
        // mixer : les pistes de FL Studio, avec leurs noms et réglages, et le routage
        assert_eq!(p.mixage[0].nom, "Master");
        assert_eq!(p.mixage[4].nom, "Zero Volume");
        assert_eq!(p.mixage[4].volume, 0.0);
        assert!((p.mixage[1].volume - 0.8).abs() < 1e-6, "0 dB");
        assert_eq!((p.mixage[6].pan, p.mixage[7].pan), (-1.0, 1.0));
        assert_eq!(canal("Instrument track").piste_mixage, 2);
        // patterns : les 5, et les notes du pattern « MIDI »
        let noms: Vec<&str> = p.patterns.iter().map(|x| x.nom.as_str()).filter(|n| !n.starts_with("Clip audio")).collect();
        assert_eq!(noms, vec!["Default", "Colored", "MIDI", "Timemarkers", "Selected"]);
        let midi = p.patterns.iter().find(|x| x.nom == "MIDI").unwrap();
        // 48 notes dans le fichier, dont 8 sur le canal d'automatisation et la sortie MIDI
        assert_eq!(midi.notes.values().map(Vec::len).sum::<usize>(), 40);
        assert_eq!(p.pattern_actif().nom, "Selected", "le pattern choisi dans FL Studio");
        // arrangement : le pattern « MIDI » sur sa piste, toutes les mesures
        let clips_midi: Vec<u32> = p.playlist.iter().filter(|c| c.pattern == midi.id).map(|c| c.debut).collect();
        assert!(clips_midi.len() >= 4);
        assert!(clips_midi.iter().all(|d| d % 384 == 0));
        // les clips audio du canal 11 rejoués par son canal
        let audio: Vec<_> = p.patterns.iter().filter(|x| x.nom == "Clip audio : 22in Kick").collect();
        assert_eq!(audio.len(), 1);
        assert!(p.playlist.iter().filter(|c| c.pattern == audio[0].id).count() >= 8);
        // le rapport dit ce qui a été remplacé
        assert!(i.rapport.iter().any(|r| r.contains("automatisation")), "{:?}", i.rapport);
        assert!(i.rapport.iter().any(|r| r.contains("22in Kick") && r.contains("introuvable")));
        assert!(i.rapport.iter().any(|r| r.contains("BooBass")));
        // le projet est valide (il s'enregistre et se relit)
        assert_eq!(deserialiser(&serialiser(p)).unwrap(), *p);
    }

    #[test]
    fn echantillon_retrouve_dans_le_dossier_du_projet() {
        let d = std::env::temp_dir().join(format!("song-audio-flp-{}", std::process::id()));
        std::fs::create_dir_all(d.join("Samples")).unwrap();
        std::fs::write(d.join("Samples/22in Kick.wav"), b"RIFF").unwrap();
        let trouve = trouver_echantillon(r"%FLStudioFactoryData%\Data\Patches\Packs\Drums\Kicks\22in Kick.wav", Some(&d), &[]);
        assert_eq!(trouve, Some(d.join("Samples").join("22in Kick.wav")));
        let i = depuis_flp(&fichier_d_essai(), "essai", Some(&d), &[]).unwrap();
        let kick = i.projet.canaux.iter().find(|c| c.nom == "22in Kick").unwrap();
        assert_eq!(kick.source, SourceCanal::Echantillons);
        assert!(kick.instrument.as_deref().unwrap().ends_with("22in Kick.wav"));
        assert!(!i.rapport.iter().any(|r| r.contains("introuvable")));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn noms_vers_sons() {
        assert_eq!(batterie_d_apres_nom("Kick 03"), Some(36));
        assert_eq!(batterie_d_apres_nom("Open Hat"), Some(46));
        assert_eq!(batterie_d_apres_nom("Closed HH"), Some(42));
        assert_eq!(batterie_d_apres_nom("Snare Roll"), Some(38));
        assert_eq!(batterie_d_apres_nom("Piano"), None);
        assert_eq!(programme_d_apres_nom(&["808 Glide"]), 38);
        assert_eq!(programme_d_apres_nom(&["Lead", "Sytrus"]), 81);
        assert_eq!(programme_d_apres_nom(&["Mystère"]), 0);
    }

    #[test]
    fn fichiers_refuses() {
        assert!(depuis_flp(b"pas un flp", "x", None, &[]).is_err());
        let mut tronque = fichier_d_essai();
        tronque.truncate(5000);
        assert!(depuis_flp(&tronque, "x", None, &[]).is_err());
    }
}
