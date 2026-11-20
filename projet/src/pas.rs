//! Séquenceur pas à pas. Un pas est une note du pattern : une double croche, à la hauteur par
//! défaut du canal.

use crate::modele::{NoteMusicale, Pattern, Projet};
use crate::temps::TICS_PAR_PAS;

pub const PAS_MAX: u32 = 512;
pub const VELOCITE_PAS: u8 = 100;
/// Durée d'une note de pas : un peu moins qu'un pas, pour que deux pas voisins se détachent.
pub const DUREE_PAS: u32 = TICS_PAR_PAS - 2;

pub fn nombre_de_pas(pattern: &Pattern) -> u32 {
    pattern.longueur.div_ceil(TICS_PAR_PAS).max(1)
}

/// Vélocité de chaque pas du canal dans le pattern (0 = pas inactif).
pub fn velocites_des_pas(pattern: &Pattern, canal_id: &str, note_defaut: u8) -> Vec<u8> {
    let mut pas = vec![0u8; nombre_de_pas(pattern) as usize];
    for n in pattern.notes.get(canal_id).map(Vec::as_slice).unwrap_or(&[]) {
        if n.hauteur == note_defaut && n.tic % TICS_PAR_PAS == 0 {
            if let Some(p) = pas.get_mut((n.tic / TICS_PAR_PAS) as usize) {
                *p = n.velocite;
            }
        }
    }
    pas
}

/// Active ou désactive un pas du pattern actif (`None` : bascule).
pub fn definir_pas(projet: &mut Projet, canal_id: &str, index: u32, actif: Option<bool>) {
    let Some(note_defaut) = projet.canal(canal_id).map(|c| c.note_defaut) else { return };
    let pattern = projet.pattern_actif_mut();
    if index >= nombre_de_pas(pattern) {
        return;
    }
    let notes = pattern.notes.entry(canal_id.to_string()).or_default();
    let tic = index * TICS_PAR_PAS;
    let position = notes.iter().position(|n| n.tic == tic && n.hauteur == note_defaut);
    match (actif.unwrap_or(position.is_none()), position) {
        (true, None) => {
            notes.push(NoteMusicale { tic, duree: DUREE_PAS, hauteur: note_defaut, velocite: VELOCITE_PAS });
            notes.sort_by_key(|n| (n.tic, n.hauteur));
        }
        (false, Some(i)) => {
            notes.remove(i);
        }
        _ => {}
    }
}

/// Vrai si les notes du canal peuvent s'afficher en pas : toutes sur un début de pas et à la
/// hauteur par défaut, au plus une par pas. Un canal vide est en mode pas.
pub fn est_en_mode_pas(notes: &[NoteMusicale], note_defaut: u8) -> bool {
    let mut vus = std::collections::HashSet::new();
    notes.iter().all(|n| n.hauteur == note_defaut && n.tic % TICS_PAR_PAS == 0 && vus.insert(n.tic))
}

/// Vélocité d'un pas actif (1 à 127) ; sans effet sur un pas inactif.
pub fn definir_velocite_pas(projet: &mut Projet, canal_id: &str, index: u32, velocite: i32) {
    let Some(note_defaut) = projet.canal(canal_id).map(|c| c.note_defaut) else { return };
    let v = velocite.clamp(1, 127) as u8;
    if let Some(notes) = projet.pattern_actif_mut().notes.get_mut(canal_id) {
        if let Some(n) = notes.iter_mut().find(|n| n.tic == index * TICS_PAR_PAS && n.hauteur == note_defaut) {
            n.velocite = v;
        }
    }
}

/// Longueur du pattern actif, en pas (1 à 512). Les notes au-delà sont gardées mais ne jouent pas.
pub fn definir_nombre_de_pas(projet: &mut Projet, pas: i64) {
    let n = pas.clamp(1, PAS_MAX as i64) as u32;
    projet.pattern_actif_mut().longueur = n * TICS_PAR_PAS;
}

/// Hauteur par défaut d'un canal (le son de batterie, par exemple). Si le canal est en mode pas
/// dans un pattern, ses pas y suivent la nouvelle hauteur.
pub fn definir_note_defaut(projet: &mut Projet, canal_id: &str, hauteur: i32) {
    let h = hauteur.clamp(0, 127) as u8;
    let Some(ancienne) = projet.canal(canal_id).map(|c| c.note_defaut) else { return };
    for pattern in projet.patterns.iter_mut() {
        if let Some(notes) = pattern.notes.get_mut(canal_id) {
            if est_en_mode_pas(notes, ancienne) {
                for n in notes.iter_mut() {
                    n.hauteur = h;
                }
            }
        }
    }
    if let Some(c) = projet.canal_mut(canal_id) {
        c.note_defaut = h;
    }
}

/// Rang des pas actifs d'un canal (dans le pattern actif), s'il est en mode pas.
fn pas_actifs(projet: &Projet, canal_id: &str) -> Option<(Vec<u8>, u8)> {
    let canal = projet.canal(canal_id)?;
    let pattern = projet.pattern_actif();
    let notes = pattern.notes.get(canal_id).map(Vec::as_slice).unwrap_or(&[]);
    est_en_mode_pas(notes, canal.note_defaut).then(|| (velocites_des_pas(pattern, canal_id, canal.note_defaut), canal.note_defaut))
}

/// Réécrit les pas d'un canal d'après leurs vélocités (0 = inactif).
fn ecrire_pas(projet: &mut Projet, canal_id: &str, velocites: &[u8], note: u8) {
    let notes: Vec<NoteMusicale> = velocites
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0)
        .map(|(i, v)| NoteMusicale { tic: i as u32 * TICS_PAR_PAS, duree: DUREE_PAS, hauteur: note, velocite: *v })
        .collect();
    projet.pattern_actif_mut().notes.insert(canal_id.to_string(), notes);
}

/// Allume un pas sur `n` à partir du pas `depart` (les autres pas ne changent pas). Sans effet
/// sur un canal aux notes libres.
pub fn remplir_tous_les(projet: &mut Projet, canal_id: &str, n: u32, depart: u32) {
    let Some((mut pas, note)) = pas_actifs(projet, canal_id) else { return };
    for (i, v) in pas.iter_mut().enumerate() {
        if n > 0 && i as u32 >= depart && (i as u32 - depart) % n == 0 && *v == 0 {
            *v = VELOCITE_PAS;
        }
    }
    ecrire_pas(projet, canal_id, &pas, note);
}

/// Fait tourner les pas d'un cran (positif : vers la droite ; le dernier revient au début).
pub fn decaler_pas(projet: &mut Projet, canal_id: &str, sens: i32) {
    let Some((mut pas, note)) = pas_actifs(projet, canal_id) else { return };
    if pas.is_empty() {
        return;
    }
    let k = sens.rem_euclid(pas.len() as i32) as usize;
    pas.rotate_right(k);
    ecrire_pas(projet, canal_id, &pas, note);
}

/// Allume les pas éteints et éteint les pas allumés.
pub fn inverser_pas(projet: &mut Projet, canal_id: &str) {
    let Some((mut pas, note)) = pas_actifs(projet, canal_id) else { return };
    for v in pas.iter_mut() {
        *v = if *v > 0 { 0 } else { VELOCITE_PAS };
    }
    ecrire_pas(projet, canal_id, &pas, note);
}

/// Éteint tous les pas du canal.
pub fn effacer_pas(projet: &mut Projet, canal_id: &str) {
    let Some((pas, note)) = pas_actifs(projet, canal_id) else { return };
    ecrire_pas(projet, canal_id, &vec![0; pas.len()], note);
}

/// Pas au hasard : chaque pas s'allume avec la probabilité `densite`, à une vélocité tirée entre
/// 70 et 127. La même graine donne le même motif.
pub fn pas_aleatoires(projet: &mut Projet, canal_id: &str, densite: f32, graine: u64) {
    let Some((pas, note)) = pas_actifs(projet, canal_id) else { return };
    let mut etat = graine.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1;
    let mut tirer = || {
        etat ^= etat << 13;
        etat ^= etat >> 7;
        etat ^= etat << 17;
        (etat >> 11) as f64 / (1u64 << 53) as f64
    };
    let nouveaux: Vec<u8> = pas.iter().map(|_| if tirer() < densite as f64 { 70 + (tirer() * 57.0) as u8 } else { 0 }).collect();
    ecrire_pas(projet, canal_id, &nouveaux, note);
}

/// Swing : les notes d'un pas impair (la 2e double croche de chaque croche) sont retardées, de
/// la moitié d'un pas à 100 %.
pub fn decalage_swing(tic: u32, swing: f64) -> f64 {
    if swing <= 0.0 || tic % (2 * TICS_PAR_PAS) != TICS_PAR_PAS {
        return 0.0;
    }
    swing.min(1.0) * (TICS_PAR_PAS as f64 / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::creer_demo;
    use crate::lecture::{ModeLecture, notes_a_jouer};
    use crate::modele::{OptionsCanal, creer_projet};

    fn avec_batterie() -> (Projet, String) {
        let mut p = creer_projet("Essai");
        let id = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
        (p, id)
    }

    // ---- les tests de la version web (pas.test.ts), traduits

    #[test]
    fn pattern_neuf_16_pas_canal_vide_en_mode_pas() {
        let (p, _) = avec_batterie();
        assert_eq!(nombre_de_pas(p.pattern_actif()), 16);
        assert_eq!(p.canaux[0].note_defaut, 36);
        assert!(est_en_mode_pas(&[], p.canaux[0].note_defaut));
        assert_eq!(velocites_des_pas(p.pattern_actif(), &p.canaux[0].id, 36), vec![0; 16]);
    }

    #[test]
    fn activer_un_pas_cree_une_double_croche_rebasculer_l_enleve() {
        let (mut p, id) = avec_batterie();
        definir_pas(&mut p, &id, 4, None);
        definir_pas(&mut p, &id, 0, None);
        assert_eq!(
            p.pattern_actif().notes[&id],
            vec![
                NoteMusicale { tic: 0, duree: DUREE_PAS, hauteur: 36, velocite: VELOCITE_PAS },
                NoteMusicale { tic: 96, duree: DUREE_PAS, hauteur: 36, velocite: VELOCITE_PAS },
            ]
        );
        assert_eq!(velocites_des_pas(p.pattern_actif(), &id, 36)[..5], [100, 0, 0, 0, 100]);
        definir_pas(&mut p, &id, 4, None);
        assert_eq!(p.pattern_actif().notes[&id].len(), 1);
        definir_pas(&mut p, &id, 2, Some(true));
        definir_pas(&mut p, &id, 2, Some(true));
        assert_eq!(p.pattern_actif().notes[&id].len(), 2);
        definir_pas(&mut p, &id, 2, Some(false));
        definir_pas(&mut p, &id, 2, Some(false));
        assert_eq!(p.pattern_actif().notes[&id].len(), 1);
        let avant = p.clone();
        definir_pas(&mut p, &id, 16, None);
        assert_eq!(p, avant, "hors du pattern : sans effet");
    }

    #[test]
    fn velocite_d_un_pas_actif_entre_1_et_127() {
        let (mut p, id) = avec_batterie();
        definir_pas(&mut p, &id, 3, None);
        definir_velocite_pas(&mut p, &id, 3, 200);
        assert_eq!(velocites_des_pas(p.pattern_actif(), &id, 36)[3], 127);
        definir_velocite_pas(&mut p, &id, 3, -5);
        assert_eq!(velocites_des_pas(p.pattern_actif(), &id, 36)[3], 1);
        let avant = p.clone();
        definir_velocite_pas(&mut p, &id, 5, 50);
        assert_eq!(p, avant, "pas inactif : rien");
    }

    #[test]
    fn longueur_de_1_a_512_pas() {
        let mut p = creer_projet("Essai");
        definir_nombre_de_pas(&mut p, 32);
        assert_eq!(nombre_de_pas(p.pattern_actif()), 32);
        definir_nombre_de_pas(&mut p, 0);
        assert_eq!(nombre_de_pas(p.pattern_actif()), 1);
        definir_nombre_de_pas(&mut p, 9999);
        assert_eq!(nombre_de_pas(p.pattern_actif()), PAS_MAX);
    }

    #[test]
    fn canal_aux_notes_libres_pas_en_mode_pas() {
        let demo = creer_demo();
        let pattern = demo.pattern_actif();
        assert!(demo.canaux.iter().all(|c| !est_en_mode_pas(&pattern.notes[&c.id], c.note_defaut)));
        let n = |tic, hauteur, velocite| NoteMusicale { tic, duree: 22, hauteur, velocite };
        assert!(est_en_mode_pas(&[n(24, 60, 90)], 60));
        assert!(!est_en_mode_pas(&[n(25, 60, 90)], 60));
        assert!(!est_en_mode_pas(&[n(24, 62, 90)], 60));
        assert!(!est_en_mode_pas(&[n(24, 60, 90), n(24, 60, 50)], 60));
    }

    #[test]
    fn changer_le_son_de_batterie_deplace_les_pas() {
        let (mut p, id) = avec_batterie();
        definir_pas(&mut p, &id, 0, None);
        definir_pas(&mut p, &id, 8, None);
        definir_note_defaut(&mut p, &id, 38);
        assert_eq!(p.canaux[0].note_defaut, 38);
        assert_eq!(p.pattern_actif().notes[&id].iter().map(|n| n.hauteur).collect::<Vec<_>>(), vec![38, 38]);
        assert_eq!(velocites_des_pas(p.pattern_actif(), &id, 38)[8], 100);
    }

    #[test]
    fn swing_comme_la_version_web() {
        assert_eq!(decalage_swing(24, 0.0), 0.0);
        assert_eq!(decalage_swing(24, 1.0), 12.0);
        assert_eq!(decalage_swing(72, 0.5), 6.0);
        assert_eq!(decalage_swing(48, 1.0), 0.0);
        assert_eq!(decalage_swing(30, 1.0), 0.0);
        // le transport applique le swing aux attaques et aux relâchements
        let (mut p, id) = avec_batterie();
        definir_pas(&mut p, &id, 0, None);
        definir_pas(&mut p, &id, 1, None);
        p.swing = 1.0;
        let notes = notes_a_jouer(&p, ModeLecture::Pattern).0;
        assert_eq!(notes.iter().map(|n| (n.debut, n.fin)).collect::<Vec<_>>(), vec![(0, DUREE_PAS), (36, 36 + DUREE_PAS)]);
    }

    #[test]
    fn pas_allumes_eteints_et_vélocites() {
        let mut p = creer_projet("Essai");
        let id = p.ajouter_canal(OptionsCanal { batterie: true, ..Default::default() });
        definir_pas(&mut p, &id, 4, None);
        definir_pas(&mut p, &id, 0, Some(true));
        definir_pas(&mut p, &id, 0, Some(true)); // déjà allumé : rien ne change
        let v = velocites_des_pas(p.pattern_actif(), &id, 36);
        assert_eq!(v.len(), 16);
        assert_eq!((v[0], v[4], v[1]), (100, 100, 0));
        definir_pas(&mut p, &id, 4, None);
        assert_eq!(velocites_des_pas(p.pattern_actif(), &id, 36)[4], 0);
        definir_pas(&mut p, &id, 99, Some(true)); // hors du pattern : ignoré
        assert_eq!(p.pattern_actif().notes[&id].len(), 1);
    }

    #[test]
    fn outils_remplir_decaler_inverser_effacer_hasard() {
        let (mut p, id) = avec_batterie();
        let v = |p: &Projet| velocites_des_pas(p.pattern_actif(), &id, 36).iter().map(|v| u8::from(*v > 0)).collect::<Vec<_>>();
        remplir_tous_les(&mut p, &id, 4, 0);
        assert_eq!(v(&p), vec![1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        remplir_tous_les(&mut p, &id, 8, 2);
        assert_eq!(v(&p), vec![1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0]);
        decaler_pas(&mut p, &id, 1);
        assert_eq!(v(&p), vec![0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0]);
        decaler_pas(&mut p, &id, -2);
        assert_eq!(v(&p), vec![0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1], "les deux premiers repartent à la fin");
        inverser_pas(&mut p, &id);
        assert_eq!(v(&p)[0..4], [1, 0, 1, 0]);
        effacer_pas(&mut p, &id);
        assert_eq!(v(&p), vec![0; 16]);
        pas_aleatoires(&mut p, &id, 0.5, 42);
        let premier = p.pattern_actif().notes[&id].clone();
        assert!((3..=13).contains(&premier.len()), "{} pas", premier.len());
        assert!(premier.iter().all(|n| (70..=127).contains(&n.velocite)));
        pas_aleatoires(&mut p, &id, 0.5, 42);
        assert_eq!(p.pattern_actif().notes[&id], premier, "même graine, même motif");
        pas_aleatoires(&mut p, &id, 0.0, 7);
        assert!(p.pattern_actif().notes[&id].is_empty());
        // un canal aux notes libres n'est pas touché
        let mut demo = crate::demo::creer_demo();
        let piano = demo.canaux[0].id.clone();
        let avant = demo.clone();
        remplir_tous_les(&mut demo, &piano, 2, 0);
        inverser_pas(&mut demo, &piano);
        assert_eq!(demo, avant);
    }

    #[test]
    fn swing_par_canal() {
        let (mut p, id) = avec_batterie();
        definir_pas(&mut p, &id, 1, None);
        p.swing = 1.0;
        let debut = |p: &Projet| notes_a_jouer(p, ModeLecture::Pattern).0[0].debut;
        assert_eq!(debut(&p), 36);
        p.canal_mut(&id).unwrap().swing = 0.5;
        assert_eq!(debut(&p), 30, "la moitié du swing global");
        p.canal_mut(&id).unwrap().swing = 0.0;
        assert_eq!(debut(&p), 24);
        let json = crate::serialisation::serialiser(&p);
        assert_eq!(crate::serialisation::deserialiser(&json).unwrap(), p);
        p.canal_mut(&id).unwrap().swing = 1.0;
        assert_eq!(crate::serialisation::serialiser(&p).matches("\"swing\"").count(), 1, "valeur par défaut : seul le swing du projet est écrit");
    }

    #[test]
    fn swing_sur_les_pas_impairs() {
        assert_eq!(decalage_swing(24, 1.0), 12.0);
        assert_eq!(decalage_swing(48, 1.0), 0.0);
        assert_eq!(decalage_swing(24, 0.0), 0.0);
    }
}
