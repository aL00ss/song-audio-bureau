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
    use crate::modele::{OptionsCanal, creer_projet};

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
