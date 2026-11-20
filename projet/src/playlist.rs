//! Patterns et arrangement : créer, dupliquer, renommer, supprimer, choisir un pattern ; poser,
//! déplacer, supprimer des clips. Fonctions pures, reprises de la version web avec ses tests.

use crate::modele::{ClipPlaylist, Projet, creer_pattern, nouvel_id};
use crate::temps::tics_par_mesure;

/// Nombre minimal de pistes affichées dans l'arrangement.
pub const PISTES_PLAYLIST_MIN: u32 = 8;

/// « Pattern 2 », « Pattern 3 »… : le premier numéro libre.
pub fn nom_pattern_suivant(projet: &Projet) -> String {
    (projet.patterns.len() + 1..)
        .map(|n| format!("Pattern {n}"))
        .find(|nom| !projet.patterns.iter().any(|p| &p.nom == nom))
        .expect("un nom libre existe toujours")
}

/// Nouveau pattern vide (une mesure), qui devient le pattern actif. Renvoie son identifiant.
pub fn nouveau_pattern(projet: &mut Projet) -> String {
    let pattern = creer_pattern(&nom_pattern_suivant(projet), tics_par_mesure(projet.signature));
    let id = pattern.id.clone();
    projet.patterns.push(pattern);
    projet.pattern_actif = id.clone();
    id
}

/// Copie d'un pattern (notes comprises), juste après lui, qui devient le pattern actif.
pub fn dupliquer_pattern(projet: &mut Projet, id: &str) -> Option<String> {
    let index = projet.patterns.iter().position(|p| p.id == id)?;
    let mut copie = projet.patterns[index].clone();
    copie.id = nouvel_id("pattern");
    copie.nom = format!("{} (copie)", copie.nom);
    let nouvel = copie.id.clone();
    projet.patterns.insert(index + 1, copie);
    projet.pattern_actif = nouvel.clone();
    Some(nouvel)
}

pub fn renommer_pattern(projet: &mut Projet, id: &str, nom: &str) -> bool {
    let nom = nom.trim();
    match projet.patterns.iter_mut().find(|p| p.id == id) {
        Some(p) if !nom.is_empty() => {
            p.nom = nom.to_string();
            true
        }
        _ => false,
    }
}

pub fn choisir_pattern(projet: &mut Projet, id: &str) -> bool {
    if !projet.patterns.iter().any(|p| p.id == id) || projet.pattern_actif == id {
        return false;
    }
    projet.pattern_actif = id.to_string();
    true
}

/// Supprime un pattern et ses clips ; le dernier pattern ne peut pas être supprimé. Si c'était
/// le pattern actif, celui qui prend sa place le devient.
pub fn supprimer_pattern(projet: &mut Projet, id: &str) -> bool {
    let Some(index) = projet.patterns.iter().position(|p| p.id == id) else { return false };
    if projet.patterns.len() <= 1 {
        return false;
    }
    projet.patterns.remove(index);
    projet.playlist.retain(|c| c.pattern != id);
    if projet.pattern_actif == id {
        projet.pattern_actif = projet.patterns[index.min(projet.patterns.len() - 1)].id.clone();
    }
    true
}

/// Pose un clip du pattern ; renvoie son identifiant (None si le pattern n'existe pas).
pub fn ajouter_clip(projet: &mut Projet, pattern: &str, piste: i64, debut: i64) -> Option<String> {
    projet.ajouter_clip(pattern, piste.max(0) as u32, debut.max(0) as u32)
}

pub fn deplacer_clip(projet: &mut Projet, id: &str, piste: i64, debut: i64) -> bool {
    match projet.playlist.iter_mut().find(|c| c.id == id) {
        Some(c) => {
            c.piste = piste.max(0) as u32;
            c.debut = debut.max(0) as u32;
            true
        }
        None => false,
    }
}

pub fn supprimer_clip(projet: &mut Projet, id: &str) -> bool {
    let avant = projet.playlist.len();
    projet.playlist.retain(|c| c.id != id);
    projet.playlist.len() != avant
}

/// Clip posé sur une piste à un instant (le plus récent au-dessus), ou None.
pub fn clip_sous(projet: &Projet, piste: u32, tic: f64) -> Option<&ClipPlaylist> {
    projet.playlist.iter().rev().find(|c| {
        c.piste == piste
            && projet.patterns.iter().find(|p| p.id == c.pattern).is_some_and(|p| tic >= c.debut as f64 && tic < (c.debut + p.longueur) as f64)
    })
}

/// Nombre de pistes à afficher : au moins 8, et une de plus que la dernière utilisée.
pub fn nombre_de_pistes(projet: &Projet) -> u32 {
    projet.playlist.iter().map(|c| c.piste + 2).max().unwrap_or(0).max(PISTES_PLAYLIST_MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lecture::{ModeLecture, longueur_du_morceau, notes_a_jouer};
    use crate::modele::{OptionsCanal, creer_projet};
    use crate::pas::definir_pas;

    /// Deux patterns d'une mesure : A (grosse caisse au 1er pas), B (caisse claire au 1er pas).
    fn deux_patterns() -> (Projet, String, String) {
        let mut p = creer_projet("Essai");
        let kick = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), note_defaut: Some(36), ..Default::default() });
        let caisse = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Caisse claire".into()), note_defaut: Some(38), ..Default::default() });
        let a = p.pattern_actif.clone();
        definir_pas(&mut p, &kick, 0, None);
        let b = nouveau_pattern(&mut p);
        definir_pas(&mut p, &caisse, 0, None);
        (p, a, b)
    }

    // ---- les tests de la version web (playlist.test.ts), traduits

    #[test]
    fn nouveau_pattern_nom_suivant_une_mesure_devient_actif() {
        let mut p = creer_projet("Essai");
        assert_eq!(nom_pattern_suivant(&p), "Pattern 2");
        let id = nouveau_pattern(&mut p);
        assert_eq!(p.patterns.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>(), vec!["Pattern 1", "Pattern 2"]);
        assert_eq!(p.pattern_actif, id);
        assert_eq!(p.pattern_actif().longueur, 384);
        // un nom déjà pris est sauté
        p.patterns[1].nom = "Pattern 3".into();
        assert_eq!(nom_pattern_suivant(&p), "Pattern 4");
    }

    #[test]
    fn dupliquer_copie_les_notes_renommer_choisir_supprimer() {
        let (mut p, a, b) = deux_patterns();
        let notes_a = p.patterns[0].notes.clone();
        let copie = dupliquer_pattern(&mut p, &a).unwrap();
        assert_eq!(p.patterns.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>(), vec!["Pattern 1", "Pattern 1 (copie)", "Pattern 2"]);
        assert_eq!(p.pattern_actif().notes, notes_a);
        // copie indépendante
        p.pattern_actif_mut().notes.clear();
        assert_eq!(p.patterns[0].notes, notes_a);
        assert!(renommer_pattern(&mut p, &copie, "  Refrain "));
        assert_eq!(p.patterns[1].nom, "Refrain");
        assert!(!renommer_pattern(&mut p, &copie, "   "));
        assert!(choisir_pattern(&mut p, &b));
        assert_eq!(p.pattern_actif, b);
        assert!(!choisir_pattern(&mut p, "inconnu"));
        // supprimer le pattern actif : celui qui prend sa place devient actif ; ses clips partent
        choisir_pattern(&mut p, &copie);
        ajouter_clip(&mut p, &copie, 0, 0);
        assert!(supprimer_pattern(&mut p, &copie));
        assert_eq!(p.patterns.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>(), vec!["Pattern 1", "Pattern 2"]);
        assert_eq!(p.pattern_actif, b);
        assert!(p.playlist.is_empty());
        supprimer_pattern(&mut p, &a);
        assert!(!supprimer_pattern(&mut p, &b), "jamais zéro pattern");
        assert_eq!(p.patterns.len(), 1);
        assert!(dupliquer_pattern(&mut p, "inconnu").is_none());
    }

    #[test]
    fn poser_deplacer_retrouver_et_supprimer_des_clips() {
        let (mut p, a, b) = deux_patterns();
        let c1 = ajouter_clip(&mut p, &a, 0, 0).unwrap();
        let c2 = ajouter_clip(&mut p, &b, 1, 384).unwrap();
        assert_eq!(clip_sous(&p, 0, 100.0).map(|c| c.id.as_str()), Some(c1.as_str()));
        assert!(clip_sous(&p, 0, 400.0).is_none());
        assert_eq!(clip_sous(&p, 1, 384.0).map(|c| c.id.as_str()), Some(c2.as_str()));
        deplacer_clip(&mut p, &c2, 3, 768);
        assert_eq!((p.playlist[1].piste, p.playlist[1].debut), (3, 768));
        assert_eq!(nombre_de_pistes(&p), 8);
        deplacer_clip(&mut p, &c2, 9, -50);
        assert_eq!((p.playlist[1].piste, p.playlist[1].debut), (9, 0));
        assert_eq!(nombre_de_pistes(&p), 11);
        assert!(supprimer_clip(&mut p, &c1));
        assert_eq!(p.playlist.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec![c2.as_str()]);
        assert!(ajouter_clip(&mut p, "inconnu", 0, 0).is_none());
        assert!(!supprimer_clip(&mut p, "inconnu"));
    }

    #[test]
    fn le_morceau_enchaine_les_clips_longueur_et_notes_decalees() {
        let (mut p, a, b) = deux_patterns();
        ajouter_clip(&mut p, &a, 0, 0);
        ajouter_clip(&mut p, &b, 1, 384);
        ajouter_clip(&mut p, &a, 0, 768);
        assert_eq!(longueur_du_morceau(&p), 3 * 384);
        let (notes, longueur) = notes_a_jouer(&p, ModeLecture::Morceau);
        assert_eq!(longueur, 3 * 384);
        assert_eq!(notes.iter().map(|n| (n.debut, n.hauteur)).collect::<Vec<_>>(), vec![(0, 36), (384, 38), (768, 36)]);
        // sans clip : une mesure de silence
        let vide = creer_projet("Vide");
        assert_eq!(notes_a_jouer(&vide, ModeLecture::Morceau), (vec![], 384));
    }
}
