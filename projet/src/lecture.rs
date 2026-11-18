//! Ce que le transport doit jouer : les notes du pattern actif (mode pattern) ou de tout
//! l'arrangement (mode morceau), sur leurs canaux MIDI, avec swing, muets et solos appliqués.

use crate::modele::{Pattern, Projet, attribuer_canaux_midi, canaux_audibles};
use crate::pas::decalage_swing;
use crate::temps::arrondir_a_la_mesure;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModeLecture {
    /// Le pattern actif en boucle.
    #[default]
    Pattern,
    /// Tout l'arrangement.
    Morceau,
}

/// Une note à jouer : début et fin en tics depuis le début de la boucle, canal MIDI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteJouee {
    pub debut: u32,
    pub fin: u32,
    pub canal: u8,
    pub hauteur: u8,
    pub velocite: u8,
}

/// Notes d'un pattern. Les notes qui dépassent la fin sont coupées à la fin.
pub fn notes_du_pattern(projet: &Projet, pattern: &Pattern) -> Vec<NoteJouee> {
    let midi = attribuer_canaux_midi(&projet.canaux).unwrap_or_default();
    let audibles = canaux_audibles(&projet.canaux);
    let mut notes = Vec::new();
    for canal in &projet.canaux {
        let (Some(&c), true) = (midi.get(&canal.id), audibles.contains(canal.id.as_str())) else { continue };
        for n in pattern.notes.get(&canal.id).map(Vec::as_slice).unwrap_or(&[]) {
            if n.tic >= pattern.longueur {
                continue;
            }
            let decalage = decalage_swing(n.tic, projet.swing);
            let debut = (n.tic as f64 + decalage).round() as u32;
            let fin = ((n.tic + n.duree) as f64 + decalage).round().min(pattern.longueur as f64) as u32;
            if debut < fin {
                notes.push(NoteJouee { debut, fin, canal: c, hauteur: n.hauteur, velocite: n.velocite.max(1) });
            }
        }
    }
    notes.sort_by_key(|n| (n.debut, n.canal, n.hauteur));
    notes
}

/// Fin du morceau : la fin du dernier clip, arrondie à la mesure (au moins une mesure).
pub fn longueur_du_morceau(projet: &Projet) -> u32 {
    let fin = projet
        .playlist
        .iter()
        .filter_map(|clip| projet.patterns.iter().find(|p| p.id == clip.pattern).map(|p| clip.debut + p.longueur))
        .max()
        .unwrap_or(0);
    arrondir_a_la_mesure(fin, projet.signature)
}

/// Les notes à jouer et la longueur de la boucle, selon le mode.
pub fn notes_a_jouer(projet: &Projet, mode: ModeLecture) -> (Vec<NoteJouee>, u32) {
    match mode {
        ModeLecture::Pattern => {
            let pattern = projet.pattern_actif();
            (notes_du_pattern(projet, pattern), pattern.longueur.max(1))
        }
        ModeLecture::Morceau => {
            let mut notes = Vec::new();
            for clip in &projet.playlist {
                if let Some(pattern) = projet.patterns.iter().find(|p| p.id == clip.pattern) {
                    notes.extend(notes_du_pattern(projet, pattern).into_iter().map(|n| NoteJouee { debut: n.debut + clip.debut, fin: n.fin + clip.debut, ..n }));
                }
            }
            notes.sort_by_key(|n| (n.debut, n.canal, n.hauteur));
            (notes, longueur_du_morceau(projet))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::creer_demo;
    use crate::modele::{OptionsCanal, creer_projet};
    use crate::pas::definir_pas;

    #[test]
    fn notes_du_pattern_et_du_morceau() {
        let demo = creer_demo();
        let (notes, longueur) = notes_a_jouer(&demo, ModeLecture::Pattern);
        assert_eq!(longueur, 4 * 384);
        assert!(notes.iter().any(|n| n.canal == 9 && n.hauteur == 49 && n.debut == 0), "crash de batterie sur le canal 10");
        assert!(notes.windows(2).all(|w| w[0].debut <= w[1].debut));

        let mut p = creer_projet("Essai");
        let id = p.ajouter_canal(OptionsCanal { batterie: true, ..Default::default() });
        definir_pas(&mut p, &id, 0, Some(true));
        let pattern = p.pattern_actif.clone();
        p.ajouter_clip(&pattern, 0, 0);
        p.ajouter_clip(&pattern, 1, 768);
        let (morceau, longueur) = notes_a_jouer(&p, ModeLecture::Morceau);
        assert_eq!(morceau.iter().map(|n| n.debut).collect::<Vec<_>>(), vec![0, 768]);
        assert_eq!(longueur, 1152);
    }

    #[test]
    fn muet_solo_et_swing() {
        let mut p = creer_projet("Essai");
        let a = p.ajouter_canal(OptionsCanal { batterie: true, ..Default::default() });
        let b = p.ajouter_canal(OptionsCanal { programme: 0, ..Default::default() });
        definir_pas(&mut p, &a, 1, Some(true));
        definir_pas(&mut p, &b, 0, Some(true));
        p.swing = 1.0;
        let (notes, _) = notes_a_jouer(&p, ModeLecture::Pattern);
        assert_eq!(notes.iter().find(|n| n.canal == 9).unwrap().debut, 36, "pas impair retardé d'un demi-pas");
        p.canal_mut(&b).unwrap().muet = true;
        assert_eq!(notes_a_jouer(&p, ModeLecture::Pattern).0.len(), 1);
    }
}
