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
    fn swing_sur_les_pas_impairs() {
        assert_eq!(decalage_swing(24, 1.0), 12.0);
        assert_eq!(decalage_swing(48, 1.0), 0.0);
        assert_eq!(decalage_swing(24, 0.0), 0.0);
    }
}
