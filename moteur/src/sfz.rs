//! Lecture des fichiers SFZ (le format des grandes banques d'échantillons libres : Salamander,
//! VSCO, VCSL, Karoryfer…). Un fichier SFZ est du texte : des en-têtes (`<control>`, `<global>`,
//! `<master>`, `<group>`, `<region>`) suivis de réglages `nom=valeur`. Chaque région hérite des
//! réglages de son groupe, de son master et du global. Seule la lecture est ici ; le jeu est
//! dans l'échantillonneur.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Comment une région boucle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Boucle {
    /// Joue l'échantillon une fois (relâcher la touche arrête le son).
    #[default]
    Aucune,
    /// Joue jusqu'au bout, quoi qu'il arrive.
    UneFois,
    /// Boucle tant que le son dure.
    Continue,
    /// Boucle tant que la touche est tenue, puis joue la fin.
    Tenue,
}

/// Quand une région se déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Declenchement {
    #[default]
    Attaque,
    /// Au relâchement de la touche (bruit de relâchement d'un piano).
    Relachement,
    /// Seulement si aucune autre touche n'est tenue.
    Premiere,
    /// Seulement si une autre touche est tenue.
    Legato,
}

/// Une région : un échantillon, la plage de touches et de vélocités où il joue, et ses réglages.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub echantillon: PathBuf,
    pub touche_basse: u8,
    pub touche_haute: u8,
    pub velocite_basse: u8,
    pub velocite_haute: u8,
    /// Note jouée par l'échantillon sans transposition.
    pub note_racine: u8,
    /// Réglage fin, en cents, et transposition, en demi-tons.
    pub accord: f32,
    pub transposition: i32,
    /// Cents par touche (100 : suit le clavier ; 0 : même hauteur partout, pour une batterie).
    pub suivi_clavier: f32,
    pub volume_db: f32,
    /// -100 (gauche) à 100 (droite).
    pub pan: f32,
    /// Part de la vélocité dans le volume, en % (100 : une touche douce est plus douce).
    pub suivi_velocite: f32,
    pub boucle: Boucle,
    /// Début et fin de la boucle, en trames (None : celles du fichier, s'il en a).
    pub boucle_debut: Option<u64>,
    pub boucle_fin: Option<u64>,
    /// Décalage du début, en trames ; fin de l'échantillon (None : jusqu'au bout).
    pub decalage: u64,
    pub fin: Option<u64>,
    /// Enveloppe de volume, en secondes (maintien : 0 à 100 %).
    pub retard: f32,
    pub attaque: f32,
    pub tenue: f32,
    pub declin: f32,
    pub maintien: f32,
    pub relachement: f32,
    pub declenchement: Declenchement,
    /// Groupe de la région et groupe qu'elle coupe (charleston fermé qui coupe l'ouvert).
    pub groupe: i64,
    pub coupe: Option<i64>,
    /// Alternance des prises (round robin) : longueur du cycle et place de cette prise.
    pub sequence: (u32, u32),
    /// Amortissement des bruits de relâchement, en dB par seconde de touche tenue.
    pub amortissement_relachement: f32,
    /// Choix au hasard entre des prises : la région joue si le tirage (0 à 1) tombe dans
    /// [bas, haut[.
    pub alea: (f32, f32),
    /// Changement d'articulation par des touches : la région joue si la dernière touche de
    /// commutation enfoncée est `commutateur` ; touches de commutation et articulation de départ.
    pub commutateur: Option<u8>,
    pub commutation: Option<(u8, u8)>,
    pub commutateur_defaut: Option<u8>,
    /// Courbe de vélocité : (vélocité, gain de 0 à 1), points donnés par le fichier.
    pub courbe_velocite: Vec<(u8, f32)>,
    /// Amplitude, en %, et parts qui dépendent de contrôleurs MIDI (numéro, %).
    pub amplitude: f32,
    pub amplitude_cc: Vec<(u8, f32)>,
    /// Plages de contrôleurs hors desquelles la région ne joue pas (numéro, bas, haut).
    pub plages_cc: Vec<(u8, u8, u8)>,
}

impl Default for Region {
    fn default() -> Self {
        Region {
            echantillon: PathBuf::new(),
            touche_basse: 0,
            touche_haute: 127,
            velocite_basse: 1,
            velocite_haute: 127,
            note_racine: 60,
            accord: 0.0,
            transposition: 0,
            suivi_clavier: 100.0,
            volume_db: 0.0,
            pan: 0.0,
            suivi_velocite: 100.0,
            boucle: Boucle::Aucune,
            boucle_debut: None,
            boucle_fin: None,
            decalage: 0,
            fin: None,
            retard: 0.0,
            attaque: 0.0,
            tenue: 0.0,
            declin: 0.0,
            maintien: 100.0,
            relachement: 0.001,
            declenchement: Declenchement::Attaque,
            groupe: 0,
            coupe: None,
            sequence: (1, 1),
            amortissement_relachement: 0.0,
            alea: (0.0, 1.0),
            commutateur: None,
            commutation: None,
            commutateur_defaut: None,
            courbe_velocite: Vec::new(),
            amplitude: 100.0,
            amplitude_cc: Vec::new(),
            plages_cc: Vec::new(),
        }
    }
}

/// Numéro MIDI d'une touche : nombre (60) ou nom (c4, C#4, db4 ; do central = c4).
pub fn touche(valeur: &str) -> Option<u8> {
    let v = valeur.trim();
    if let Ok(n) = v.parse::<i32>() {
        return (0..=127).contains(&n).then_some(n as u8);
    }
    let mut car = v.chars();
    let lettre = car.next()?.to_ascii_lowercase();
    let base = match lettre {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let reste: String = car.collect();
    let (alteration, octave) = match reste.chars().next() {
        Some('#') | Some('♯') => (1, &reste[reste.chars().next()?.len_utf8()..]),
        Some('b') | Some('♭') if reste.len() > 1 => (-1, &reste[reste.chars().next()?.len_utf8()..]),
        _ => (0, reste.as_str()),
    };
    let octave: i32 = octave.parse().ok()?;
    let n = (octave + 1) * 12 + base + alteration;
    (0..=127).contains(&n).then_some(n as u8)
}

/// Le texte d'un fichier SFZ découpé en (en-tête ou réglage) : commentaires retirés, définitions
/// (`#define $NOM valeur`) remplacées, valeurs à espaces (noms de fichiers) gardées entières.
fn jetons(texte: &str) -> Vec<Jeton> {
    let mut definitions: Vec<(String, String)> = Vec::new();
    let mut jetons = Vec::new();
    for ligne in texte.lines() {
        // commentaire : « // » jusqu'à la fin de la ligne
        let ligne = ligne.split("//").next().unwrap_or("");
        let ligne = ligne.trim();
        if let Some(def) = ligne.strip_prefix("#define") {
            let mut morceaux = def.split_whitespace();
            if let (Some(nom), Some(valeur)) = (morceaux.next(), morceaux.next()) {
                definitions.push((nom.to_string(), valeur.to_string()));
                definitions.sort_by_key(|(n, _)| std::cmp::Reverse(n.len())); // $A ne doit pas mordre $AB
            }
            continue;
        }
        if ligne.starts_with('#') {
            continue; // #include et autres directives : non prises en charge
        }
        let mut ligne = ligne.to_string();
        for (nom, valeur) in &definitions {
            ligne = ligne.replace(nom.as_str(), valeur);
        }
        let mut reste = ligne.as_str();
        while !reste.trim().is_empty() {
            reste = reste.trim_start();
            if let Some(apres) = reste.strip_prefix('<') {
                let fin = apres.find('>').unwrap_or(apres.len());
                jetons.push(Jeton::EnTete(apres[..fin].trim().to_lowercase()));
                reste = apres.get(fin + 1..).unwrap_or("");
                continue;
            }
            let Some(egal) = reste.find('=') else { break };
            let nom = reste[..egal].trim().to_string();
            let apres = &reste[egal + 1..];
            // la valeur s'arrête au prochain « nom= » ou « < » ; « sample » peut contenir des espaces
            let fin = prochain_reglage(apres).unwrap_or(apres.len());
            let valeur = apres[..fin].trim();
            let valeur = if nom == "sample" { valeur.to_string() } else { valeur.split_whitespace().next().unwrap_or("").to_string() };
            jetons.push(Jeton::Reglage(nom, valeur));
            reste = &apres[fin..];
        }
    }
    jetons
}

/// Position du prochain « nom= » (précédé d'un espace) ou du prochain « < ».
fn prochain_reglage(texte: &str) -> Option<usize> {
    let octets = texte.as_bytes();
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'<' {
            return Some(i);
        }
        if octets[i].is_ascii_whitespace() {
            let mut j = i + 1;
            while j < octets.len() && octets[j].is_ascii_whitespace() {
                j += 1;
            }
            let debut = j;
            while j < octets.len() && (octets[j].is_ascii_alphanumeric() || octets[j] == b'_') {
                j += 1;
            }
            if j > debut && j < octets.len() && octets[j] == b'=' {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
enum Jeton {
    EnTete(String),
    Reglage(String, String),
}

/// Applique un réglage à une région ; faux s'il n'est pas connu (il est alors ignoré).
fn appliquer(r: &mut Region, nom: &str, valeur: &str, dossier: &Path, chemin_defaut: &str) -> bool {
    let nombre = || valeur.parse::<f32>().ok();
    let entier = || valeur.parse::<i64>().ok();
    match nom {
        "sample" => r.echantillon = dossier.join(chemin_defaut).join(valeur.replace('\\', "/")),
        "key" => {
            if let Some(t) = touche(valeur) {
                (r.touche_basse, r.touche_haute, r.note_racine) = (t, t, t);
            }
        }
        "lokey" => r.touche_basse = touche(valeur).unwrap_or(r.touche_basse),
        "hikey" => r.touche_haute = touche(valeur).unwrap_or(r.touche_haute),
        "lovel" => r.velocite_basse = entier().map(|v| v.clamp(0, 127) as u8).unwrap_or(r.velocite_basse),
        "hivel" => r.velocite_haute = entier().map(|v| v.clamp(0, 127) as u8).unwrap_or(r.velocite_haute),
        "pitch_keycenter" => r.note_racine = touche(valeur).unwrap_or(r.note_racine),
        "tune" => r.accord = nombre().unwrap_or(0.0),
        "transpose" => r.transposition = entier().unwrap_or(0) as i32,
        "pitch_keytrack" => r.suivi_clavier = nombre().unwrap_or(100.0),
        "volume" => r.volume_db = nombre().unwrap_or(0.0),
        "pan" => r.pan = nombre().unwrap_or(0.0).clamp(-100.0, 100.0),
        "amp_veltrack" => r.suivi_velocite = nombre().unwrap_or(100.0),
        "loop_mode" | "loopmode" => {
            r.boucle = match valeur {
                "one_shot" => Boucle::UneFois,
                "loop_continuous" => Boucle::Continue,
                "loop_sustain" => Boucle::Tenue,
                _ => Boucle::Aucune,
            }
        }
        "loop_start" | "loopstart" => r.boucle_debut = entier().map(|v| v.max(0) as u64),
        "loop_end" | "loopend" => r.boucle_fin = entier().map(|v| v.max(0) as u64),
        "offset" => r.decalage = entier().unwrap_or(0).max(0) as u64,
        "end" => r.fin = entier().map(|v| v.max(0) as u64),
        "ampeg_delay" => r.retard = nombre().unwrap_or(0.0).max(0.0),
        "ampeg_attack" => r.attaque = nombre().unwrap_or(0.0).max(0.0),
        "ampeg_hold" => r.tenue = nombre().unwrap_or(0.0).max(0.0),
        "ampeg_decay" => r.declin = nombre().unwrap_or(0.0).max(0.0),
        "ampeg_sustain" => r.maintien = nombre().unwrap_or(100.0).clamp(0.0, 100.0),
        "ampeg_release" => r.relachement = nombre().unwrap_or(0.001).max(0.001),
        "trigger" => {
            r.declenchement = match valeur {
                "release" => Declenchement::Relachement,
                "first" => Declenchement::Premiere,
                "legato" => Declenchement::Legato,
                _ => Declenchement::Attaque,
            }
        }
        "group" => r.groupe = entier().unwrap_or(0),
        "off_by" => r.coupe = entier(),
        "seq_length" => r.sequence.0 = entier().unwrap_or(1).max(1) as u32,
        "seq_position" => r.sequence.1 = entier().unwrap_or(1).max(1) as u32,
        "rt_decay" => r.amortissement_relachement = nombre().unwrap_or(0.0).max(0.0),
        "lorand" => r.alea.0 = nombre().unwrap_or(0.0),
        "hirand" => r.alea.1 = nombre().unwrap_or(1.0),
        "sw_last" => r.commutateur = touche(valeur),
        "sw_lokey" => r.commutation = touche(valeur).map(|t| (t, r.commutation.map_or(t, |c| c.1))),
        "sw_hikey" => r.commutation = touche(valeur).map(|t| (r.commutation.map_or(t, |c| c.0), t)),
        "sw_default" => r.commutateur_defaut = touche(valeur),
        "amplitude" => r.amplitude = nombre().unwrap_or(100.0).max(0.0),
        _ => {
            // réglages numérotés : amp_velcurve_N, amplitude_onccN / amplitude_ccN, loccN / hiccN
            if let Some(v) = nom.strip_prefix("amp_velcurve_").and_then(|n| n.parse::<u8>().ok()) {
                if let Some(g) = nombre() {
                    r.courbe_velocite.retain(|(x, _)| *x != v);
                    r.courbe_velocite.push((v.min(127), g.clamp(0.0, 1.0)));
                    r.courbe_velocite.sort_by_key(|(x, _)| *x);
                }
                return true;
            }
            if let Some(cc) = nom.strip_prefix("amplitude_oncc").or_else(|| nom.strip_prefix("amplitude_cc")).and_then(|n| n.parse::<u8>().ok()) {
                if let Some(pct) = nombre() {
                    r.amplitude_cc.retain(|(c, _)| *c != cc);
                    r.amplitude_cc.push((cc, pct));
                }
                return true;
            }
            for (prefixe, haut) in [("locc", false), ("hicc", true)] {
                if let Some(cc) = nom.strip_prefix(prefixe).and_then(|n| n.parse::<u8>().ok()) {
                    let v = entier().unwrap_or(if haut { 127 } else { 0 }).clamp(0, 127) as u8;
                    match r.plages_cc.iter_mut().find(|(c, _, _)| *c == cc) {
                        Some(p) => {
                            if haut {
                                p.2 = v
                            } else {
                                p.1 = v
                            }
                        }
                        None => r.plages_cc.push(if haut { (cc, 0, v) } else { (cc, v, 127) }),
                    }
                    return true;
                }
            }
            return false;
        }
    }
    true
}

/// Résultat de la lecture : les régions, les valeurs de départ des contrôleurs MIDI (`set_ccN`)
/// et les réglages ignorés (pour en informer).
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentSfz {
    pub regions: Vec<Region>,
    pub controleurs: [u8; 128],
    pub ignores: Vec<String>,
}

impl Default for InstrumentSfz {
    fn default() -> Self {
        InstrumentSfz { regions: Vec::new(), controleurs: [0; 128], ignores: Vec::new() }
    }
}

impl Region {
    /// Gain fixe de la région : amplitude, et parts liées aux contrôleurs à leur valeur de départ.
    pub fn gain_amplitude(&self, controleurs: &[u8; 128]) -> f32 {
        let mut g = self.amplitude / 100.0;
        for (cc, pct) in &self.amplitude_cc {
            g *= controleurs[*cc as usize & 127] as f32 / 127.0 * pct / 100.0;
        }
        g
    }

    /// Vrai si les contrôleurs, à leur valeur de départ, sont dans les plages de la région.
    pub fn dans_les_plages(&self, controleurs: &[u8; 128]) -> bool {
        self.plages_cc.iter().all(|(cc, bas, haut)| (*bas..=*haut).contains(&controleurs[*cc as usize & 127]))
    }

    /// Gain d'une vélocité selon la courbe du fichier (interpolée), s'il en donne une.
    pub fn gain_courbe(&self, velocite: u8) -> Option<f32> {
        if self.courbe_velocite.is_empty() {
            return None;
        }
        let mut points = vec![(0u8, 0.0f32)];
        points.extend(self.courbe_velocite.iter().copied().filter(|(v, _)| *v > 0 && *v < 127));
        points.push((127, self.courbe_velocite.iter().find(|(v, _)| *v == 127).map_or(1.0, |p| p.1)));
        let i = points.iter().rposition(|(v, _)| *v <= velocite).unwrap_or(0);
        let (a, b) = (points[i], points.get(i + 1).copied().unwrap_or(points[i]));
        if b.0 == a.0 {
            return Some(a.1);
        }
        Some(a.1 + (b.1 - a.1) * (velocite - a.0) as f32 / (b.0 - a.0) as f32)
    }
}

/// Remplace les lignes `#include "fichier"` par le contenu du fichier (chemins relatifs au
/// dossier de l'instrument, inclusions imbriquées comprises).
pub fn resoudre_inclusions(texte: &str, dossier: &Path, profondeur: usize) -> String {
    let mut sortie = String::with_capacity(texte.len());
    for ligne in texte.lines() {
        let propre = ligne.trim_start();
        if let Some(reste) = propre.strip_prefix("#include") {
            let nom = reste.trim().trim_matches('"').replace('\\', "/");
            if profondeur < 16 {
                if let Ok(contenu) = std::fs::read_to_string(dossier.join(&nom)) {
                    sortie.push_str(&resoudre_inclusions(&contenu, dossier, profondeur + 1));
                    sortie.push('\n');
                }
            }
            continue;
        }
        sortie.push_str(ligne);
        sortie.push('\n');
    }
    sortie
}

/// Fichiers inclus par un SFZ (pour ne lister que les instruments principaux d'une banque).
pub fn inclusions(texte: &str) -> Vec<String> {
    texte
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("#include").map(|r| r.trim().trim_matches('"').replace('\\', "/")))
        .collect()
}

/// Lit le texte d'un fichier SFZ ; les chemins des échantillons sont relatifs à `dossier`.
pub fn lire_sfz(texte: &str, dossier: &Path) -> InstrumentSfz {
    let texte = &resoudre_inclusions(texte, dossier, 0);
    // réglages de chaque niveau (global, master, group), puis la région en cours
    let mut niveaux: [Vec<(String, String)>; 3] = Default::default();
    let mut courant: Option<Vec<(String, String)>> = None;
    let mut chemin_defaut = String::new();
    let mut niveau_ouvert = None; // index dans `niveaux` du dernier en-tête de niveau
    let mut en_controle = false;
    let mut resultat = InstrumentSfz::default();
    let mut ignores: HashMap<String, ()> = HashMap::new();
    let mut fermer = |courant: &mut Option<Vec<(String, String)>>, niveaux: &[Vec<(String, String)>; 3], chemin_defaut: &str, resultat: &mut InstrumentSfz| {
        if let Some(propres) = courant.take() {
            let mut r = Region::default();
            for (nom, valeur) in niveaux.iter().flatten().chain(propres.iter()) {
                if !appliquer(&mut r, nom, valeur, dossier, chemin_defaut) {
                    ignores.insert(nom.clone(), ());
                }
            }
            if !r.echantillon.as_os_str().is_empty() {
                resultat.regions.push(r);
            }
        }
    };
    for jeton in jetons(texte) {
        match jeton {
            Jeton::EnTete(nom) => {
                fermer(&mut courant, &niveaux, &chemin_defaut, &mut resultat);
                en_controle = nom == "control";
                let niveau = match nom.as_str() {
                    "global" => Some(0),
                    "master" => Some(1),
                    "group" => Some(2),
                    _ => None,
                };
                if let Some(n) = niveau {
                    // un nouveau niveau efface ceux qui sont en dessous de lui
                    for k in n..3 {
                        niveaux[k].clear();
                    }
                    niveau_ouvert = Some(n);
                } else if nom == "region" {
                    courant = Some(Vec::new());
                    niveau_ouvert = None;
                } else {
                    niveau_ouvert = None;
                }
            }
            Jeton::Reglage(nom, valeur) => {
                if en_controle {
                    if nom == "default_path" {
                        chemin_defaut = valeur.replace('\\', "/");
                    } else if let (Some(cc), Ok(v)) = (nom.strip_prefix("set_cc").and_then(|n| n.parse::<u8>().ok()), valeur.parse::<f32>()) {
                        resultat.controleurs[cc as usize & 127] = v.clamp(0.0, 127.0) as u8;
                    }
                } else if let Some(c) = courant.as_mut() {
                    c.push((nom, valeur));
                } else if let Some(n) = niveau_ouvert {
                    niveaux[n].push((nom, valeur));
                }
            }
        }
    }
    fermer(&mut courant, &niveaux, &chemin_defaut, &mut resultat);
    let mut ignores: Vec<String> = ignores.into_keys().collect();
    ignores.sort();
    resultat.ignores = ignores;
    resultat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_de_touches() {
        assert_eq!(touche("60"), Some(60));
        assert_eq!(touche("c4"), Some(60));
        assert_eq!(touche("C#4"), Some(61));
        assert_eq!(touche("db4"), Some(61));
        assert_eq!(touche("a0"), Some(21));
        assert_eq!(touche("g9"), Some(127));
        assert_eq!(touche("c-1"), Some(0));
        assert_eq!(touche("h4"), None);
        assert_eq!(touche("200"), None);
    }

    #[test]
    fn regions_heritage_et_noms_a_espaces() {
        let texte = r"
            // un piano à deux couches
            <control> default_path=samples\
            #define $VEL_DOUX 64
            <global> ampeg_release=0.6 amp_veltrack=90
            <group> hivel=$VEL_DOUX
            <region> sample=C4 doux.wav key=c4 // commentaire
            <region> sample=D4 doux.wav lokey=61 hikey=63 pitch_keycenter=62 tune=-12
            <group> lovel=65 volume=-3
            <region> sample=C4 fort.flac key=60 loop_mode=loop_continuous loop_start=100 loop_end=900
            <region> sample=rel.wav key=60 trigger=release rt_decay=6 truc_inconnu=1
        ";
        let i = lire_sfz(texte, Path::new("/banque"));
        assert_eq!(i.regions.len(), 4);
        let r = &i.regions[0];
        assert_eq!(r.echantillon, Path::new("/banque/samples/C4 doux.wav"));
        assert_eq!((r.touche_basse, r.touche_haute, r.note_racine, r.velocite_haute), (60, 60, 60, 64));
        assert_eq!((r.relachement, r.suivi_velocite), (0.6, 90.0));
        let r = &i.regions[1];
        assert_eq!((r.touche_basse, r.touche_haute, r.note_racine, r.accord), (61, 63, 62, -12.0));
        // le deuxième groupe remplace le premier (plus de hivel=64), le global reste
        let r = &i.regions[2];
        assert_eq!((r.velocite_basse, r.velocite_haute, r.volume_db, r.relachement), (65, 127, -3.0, 0.6));
        assert_eq!((r.boucle, r.boucle_debut, r.boucle_fin), (Boucle::Continue, Some(100), Some(900)));
        let r = &i.regions[3];
        assert_eq!((r.declenchement, r.amortissement_relachement), (Declenchement::Relachement, 6.0));
        assert_eq!(i.ignores, vec!["truc_inconnu".to_string()]);
    }

    #[test]
    fn batterie_groupes_et_alternance() {
        let texte = "<group> pitch_keytrack=0 loop_mode=one_shot
            <region> sample=hh_ferme_1.wav key=42 group=1 seq_length=2 seq_position=1
            <region> sample=hh_ferme_2.wav key=42 group=1 seq_length=2 seq_position=2
            <region> sample=hh_ouvert.wav key=46 group=2 off_by=1
            <region>sample=grosse.wav key=36";
        let i = lire_sfz(texte, Path::new("kit"));
        assert_eq!(i.regions.len(), 4);
        assert!(i.regions.iter().all(|r| r.suivi_clavier == 0.0 && r.boucle == Boucle::UneFois));
        assert_eq!((i.regions[0].groupe, i.regions[0].sequence), (1, (2, 1)));
        assert_eq!(i.regions[1].sequence, (2, 2));
        assert_eq!((i.regions[2].groupe, i.regions[2].coupe), (2, Some(1)));
        assert_eq!(i.regions[3].echantillon, Path::new("kit/grosse.wav"));
    }

    #[test]
    fn inclusions_alea_commutation_courbe_et_controleurs() {
        let d = std::env::temp_dir().join(format!("song-audio-sfz-inclusions-{}", std::process::id()));
        std::fs::create_dir_all(d.join("legato")).unwrap();
        std::fs::write(
            d.join("legato/tenu.sfz"),
            "<group> sw_last=26 lorand=0 hirand=0.5\n<region> sample=../Samples/a.wav key=40\n<group> sw_last=26 lorand=0.5 hirand=1\n<region> sample=../Samples/b.wav key=40",
        )
        .unwrap();
        let principal = "<control> set_cc7=100 set_cc72=127 default_path=Samples/\n<global> sw_lokey=24 sw_hikey=26 sw_default=25 amplitude_oncc7=100 amp_velcurve_64=0.3 amp_velcurve_127=1\n#include \"legato/tenu.sfz\"\n<group> locc1=64 hicc1=127\n<region> sample=c.wav key=41";
        std::fs::write(d.join("i.sfz"), principal).unwrap();
        let i = lire_sfz(principal, &d);
        assert_eq!(inclusions(principal), vec!["legato/tenu.sfz".to_string()]);
        assert_eq!(i.regions.len(), 3);
        assert_eq!(i.controleurs[7], 100);
        let a = &i.regions[0];
        assert_eq!((a.alea, a.commutateur, a.commutation, a.commutateur_defaut), ((0.0, 0.5), Some(26), Some((24, 26)), Some(25)));
        assert_eq!(i.regions[1].alea, (0.5, 1.0));
        assert!((a.gain_amplitude(&i.controleurs) - 100.0 / 127.0).abs() < 1e-5);
        assert_eq!(a.gain_courbe(64), Some(0.3));
        assert!((a.gain_courbe(32).unwrap() - 0.15).abs() < 1e-5, "interpolée entre (0, 0) et (64, 0,3)");
        assert_eq!(a.gain_courbe(127), Some(1.0));
        // cc1 vaut 0 au départ : la région « locc1=64 » ne joue pas
        assert_eq!(i.regions[2].plages_cc, vec![(1, 64, 127)]);
        assert!(!i.regions[2].dans_les_plages(&i.controleurs));
        assert!(a.dans_les_plages(&i.controleurs));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn regions_sans_echantillon_ignorees() {
        let i = lire_sfz("<region> key=60 <region> sample=a.wav", Path::new("."));
        assert_eq!(i.regions.len(), 1);
        assert!(lire_sfz("", Path::new(".")).regions.is_empty());
    }
}
