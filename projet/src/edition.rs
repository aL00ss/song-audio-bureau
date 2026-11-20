//! Édition des notes d'un canal (piano roll). Fonctions pures sur des listes de notes, reprises
//! de la version web avec ses tests ; l'ordre des notes est conservé pendant une édition, pour
//! que la sélection (des indices) reste valable.

use std::collections::BTreeSet;

use crate::modele::NoteMusicale;
use crate::temps::TICS_PAR_PAS;

/// Pas de l'aimantation, en tics (0 = aucune).
pub const GRILLES: [(&str, u32); 6] = [
    ("Mesure", TICS_PAR_PAS * 16),
    ("Temps", TICS_PAR_PAS * 4),
    ("1/2 temps", TICS_PAR_PAS * 2),
    ("Pas (1/4 temps)", TICS_PAR_PAS),
    ("1/2 pas", TICS_PAR_PAS / 2),
    ("Aucune", 0),
];

/// Sélection : indices de notes, dans l'ordre.
pub type Selection = BTreeSet<usize>;

/// Arrondi au plus proche multiple de la grille (à l'entier sans grille).
pub fn aimanter(tic: f64, grille: u32) -> i64 {
    if grille > 0 { (tic / grille as f64).round() as i64 * grille as i64 } else { tic.round() as i64 }
}

/// Aimante vers le bas (pour placer une note là où l'on clique).
pub fn aimanter_bas(tic: f64, grille: u32) -> i64 {
    if grille > 0 { (tic / grille as f64).floor() as i64 * grille as i64 } else { tic.floor() as i64 }
}

/// Indice de la note sous un point (la plus récente au-dessus), ou None.
pub fn note_sous(notes: &[NoteMusicale], tic: f64, hauteur: i32) -> Option<usize> {
    notes.iter().enumerate().rev().find(|(_, n)| n.hauteur as i32 == hauteur && tic >= n.tic as f64 && tic < (n.tic + n.duree) as f64).map(|(i, _)| i)
}

/// Note ajoutée à la fin, ramenée dans les bornes (hauteur 0 à 127, début ≥ 0, durée ≥ 1,
/// vélocité 1 à 127).
pub fn ajouter_note(notes: &[NoteMusicale], tic: i64, duree: i64, hauteur: i32, velocite: i32) -> Vec<NoteMusicale> {
    let mut resultat = notes.to_vec();
    resultat.push(NoteMusicale {
        tic: tic.clamp(0, u32::MAX as i64) as u32,
        duree: duree.clamp(1, u32::MAX as i64) as u32,
        hauteur: hauteur.clamp(0, 127) as u8,
        velocite: velocite.clamp(1, 127) as u8,
    });
    resultat
}

pub fn supprimer_notes(notes: &[NoteMusicale], indices: &Selection) -> Vec<NoteMusicale> {
    notes.iter().enumerate().filter(|(i, _)| !indices.contains(i)).map(|(_, n)| *n).collect()
}

/// Déplace les notes choisies (le groupe ne sort pas du piano : 0 à 127, temps ≥ 0).
pub fn deplacer_notes(notes: &[NoteMusicale], indices: &Selection, d_tic: i64, d_hauteur: i32) -> Vec<NoteMusicale> {
    let choisies: Vec<&NoteMusicale> = notes.iter().enumerate().filter(|(i, _)| indices.contains(i)).map(|(_, n)| n).collect();
    let (Some(tic_min), Some(basse), Some(haute)) =
        (choisies.iter().map(|n| n.tic).min(), choisies.iter().map(|n| n.hauteur).min(), choisies.iter().map(|n| n.hauteur).max())
    else {
        return notes.to_vec();
    };
    let dt = d_tic.max(-(tic_min as i64));
    let dh = d_hauteur.clamp(-(basse as i32), 127 - haute as i32);
    notes
        .iter()
        .enumerate()
        .map(|(i, n)| if indices.contains(&i) { NoteMusicale { tic: (n.tic as i64 + dt) as u32, hauteur: (n.hauteur as i32 + dh) as u8, ..*n } } else { *n })
        .collect()
}

/// Allonge ou raccourcit les notes choisies (au moins `minimum` tics).
pub fn redimensionner_notes(notes: &[NoteMusicale], indices: &Selection, d_duree: i64, minimum: u32) -> Vec<NoteMusicale> {
    notes
        .iter()
        .enumerate()
        .map(|(i, n)| if indices.contains(&i) { NoteMusicale { duree: (n.duree as i64 + d_duree).max(minimum.max(1) as i64) as u32, ..*n } } else { *n })
        .collect()
}

/// Ramène le début des notes choisies sur la grille (toutes si aucune n'est choisie).
pub fn quantifier(notes: &[NoteMusicale], indices: &Selection, grille: u32) -> Vec<NoteMusicale> {
    if grille == 0 {
        return notes.to_vec();
    }
    let toutes = indices.is_empty();
    notes
        .iter()
        .enumerate()
        .map(|(i, n)| if toutes || indices.contains(&i) { NoteMusicale { tic: aimanter(n.tic as f64, grille).max(0) as u32, ..*n } } else { *n })
        .collect()
}

pub fn definir_velocite(notes: &[NoteMusicale], indices: &Selection, velocite: f64) -> Vec<NoteMusicale> {
    let v = velocite.round().clamp(1.0, 127.0) as u8;
    notes.iter().enumerate().map(|(i, n)| if indices.contains(&i) { NoteMusicale { velocite: v, ..*n } } else { *n }).collect()
}

/// Indices des notes dont une partie est dans le rectangle [tic1, tic2[ × [hauteur1, hauteur2].
pub fn notes_dans_rectangle(notes: &[NoteMusicale], tic1: f64, tic2: f64, h1: i32, h2: i32) -> Selection {
    let (ta, tb) = if tic1 <= tic2 { (tic1, tic2) } else { (tic2, tic1) };
    let (ha, hb) = if h1 <= h2 { (h1, h2) } else { (h2, h1) };
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| (n.hauteur as i32) >= ha && (n.hauteur as i32) <= hb && (n.tic as f64) < tb && ((n.tic + n.duree) as f64) > ta)
        .map(|(i, _)| i)
        .collect()
}

/// Hauteurs utilisées (pour centrer la vue), ou None s'il n'y a aucune note.
pub fn etendue(notes: &[NoteMusicale]) -> Option<(u8, u8)> {
    Some((notes.iter().map(|n| n.hauteur).min()?, notes.iter().map(|n| n.hauteur).max()?))
}

/// Notes qui sonnent à un instant du pattern, pour chaque canal.
pub fn notes_au_tic(pattern: &crate::modele::Pattern, tic: f64) -> std::collections::BTreeMap<&str, Vec<NoteMusicale>> {
    pattern
        .notes
        .iter()
        .filter_map(|(canal, notes)| {
            let actives: Vec<NoteMusicale> = notes.iter().filter(|n| n.tic as f64 <= tic && tic < (n.tic + n.duree) as f64).copied().collect();
            (!actives.is_empty()).then_some((canal.as_str(), actives))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(tic: u32, hauteur: u8, duree: u32) -> NoteMusicale {
        NoteMusicale { tic, duree, hauteur, velocite: 100 }
    }

    fn sel(i: &[usize]) -> Selection {
        i.iter().copied().collect()
    }

    // ---- les tests de la version web (edition.test.ts), traduits

    #[test]
    fn aimantation_a_la_grille() {
        assert_eq!(aimanter(37.0, 24), 48);
        assert_eq!(aimanter(35.0, 24), 24);
        assert_eq!(aimanter_bas(47.0, 24), 24);
        assert_eq!(aimanter(37.4, 0), 37);
    }

    #[test]
    fn note_sous_le_pointeur_la_plus_recente_gagne() {
        let notes = [n(0, 60, 96), n(48, 60, 24)];
        assert_eq!(note_sous(&notes, 50.0, 60), Some(1));
        assert_eq!(note_sous(&notes, 10.0, 60), Some(0));
        assert_eq!(note_sous(&notes, 10.0, 61), None);
        assert_eq!(note_sous(&notes, 96.0, 60), None);
    }

    #[test]
    fn ajouter_et_supprimer() {
        let notes = ajouter_note(&[n(0, 60, 24)], -5, 0, 200, 0);
        assert_eq!(notes[1], NoteMusicale { tic: 0, duree: 1, hauteur: 127, velocite: 1 });
        assert_eq!(supprimer_notes(&notes, &sel(&[0])), vec![notes[1]]);
    }

    #[test]
    fn deplacer_un_groupe_sans_sortir_du_piano_en_gardant_l_ordre() {
        let notes = [n(24, 60, 24), n(48, 126, 24), n(0, 10, 24)];
        let choix = sel(&[0, 1]);
        let tic_h = |v: Vec<NoteMusicale>| v.iter().map(|x| (x.tic, x.hauteur)).collect::<Vec<_>>();
        assert_eq!(tic_h(deplacer_notes(&notes, &choix, 24, 1)), vec![(48, 61), (72, 127), (0, 10)]);
        // trop haut : le groupe s'arrête quand sa note la plus haute atteint 127
        assert_eq!(deplacer_notes(&notes, &choix, 0, 5).iter().map(|x| x.hauteur).collect::<Vec<_>>(), vec![61, 127, 10]);
        // trop tôt : le groupe s'arrête au début du pattern
        assert_eq!(deplacer_notes(&notes, &choix, -100, 0).iter().map(|x| x.tic).collect::<Vec<_>>(), vec![0, 24, 0]);
        assert_eq!(deplacer_notes(&notes, &sel(&[]), 10, 10), notes.to_vec());
    }

    #[test]
    fn redimensionner_quantifier_velocite() {
        let notes = [n(5, 60, 30), n(50, 62, 30)];
        assert_eq!(redimensionner_notes(&notes, &sel(&[0]), -100, 1).iter().map(|x| x.duree).collect::<Vec<_>>(), vec![1, 30]);
        assert_eq!(quantifier(&notes, &sel(&[]), 24).iter().map(|x| x.tic).collect::<Vec<_>>(), vec![0, 48]);
        assert_eq!(quantifier(&notes, &sel(&[1]), 24).iter().map(|x| x.tic).collect::<Vec<_>>(), vec![5, 48]);
        assert_eq!(definir_velocite(&notes, &sel(&[0, 1]), 300.0).iter().map(|x| x.velocite).collect::<Vec<_>>(), vec![127, 127]);
    }

    #[test]
    fn selection_au_lasso_et_etendue_des_hauteurs() {
        let notes = [n(0, 60, 24), n(30, 64, 24), n(100, 70, 24)];
        assert_eq!(notes_dans_rectangle(&notes, 50.0, 10.0, 66, 58), sel(&[0, 1]));
        assert_eq!(notes_dans_rectangle(&notes, 24.0, 30.0, 60, 64), sel(&[]));
        assert_eq!(etendue(&notes), Some((60, 70)));
        assert_eq!(etendue(&[]), None);
    }

    #[test]
    fn notes_qui_sonnent_a_un_instant() {
        let mut p = crate::modele::creer_pattern("P", 384);
        p.notes.insert("a".into(), vec![n(0, 60, 48), n(96, 62, 24)]);
        p.notes.insert("b".into(), vec![n(40, 36, 10)]);
        let a = notes_au_tic(&p, 45.0);
        assert_eq!(a.keys().copied().collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(notes_au_tic(&p, 50.0).len(), 0, "la fin d'une note ne sonne plus");
    }
}
