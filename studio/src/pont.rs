//! Le projet vu par le moteur : réglages des canaux et des pistes, et session à jouer.
//! Fonctions pures (testées) : l'interface native et le moteur vivent dans le même programme,
//! sans aucun intermédiaire.

use moteur::{CANAUX, Configuration, NoteSession, PISTES, ReglageCanal, ReglagePiste, Session, TypeSource};
use projet::lecture::{ModeLecture, notes_a_jouer};
use projet::modele::{attribuer_canaux_midi, pistes_audibles};
use projet::{Projet, SourceCanal};

/// Réglages du mixer et des canaux pour le moteur.
pub fn configuration(projet: &Projet) -> Configuration {
    let mut c = Configuration::default();
    let midi = attribuer_canaux_midi(&projet.canaux).unwrap_or_default();
    let pistes_existantes: Vec<usize> = projet.mixage.iter().map(|p| p.index).collect();
    for canal in &projet.canaux {
        let Some(&m) = midi.get(&canal.id) else { continue };
        let Some(reglage) = c.canaux.get_mut(m as usize) else { continue };
        let piste = if pistes_existantes.contains(&canal.piste_mixage) && canal.piste_mixage < PISTES { canal.piste_mixage } else { 0 };
        *reglage = ReglageCanal {
            actif: true,
            programme: canal.programme,
            batterie: canal.batterie,
            volume: canal.volume,
            pan: canal.pan,
            piste: piste as u8,
            source: match canal.source {
                SourceCanal::Soustractif if !canal.batterie => TypeSource::Soustractif,
                _ => TypeSource::SoundFont,
            },
        };
    }
    let audibles = pistes_audibles(&projet.mixage);
    for piste in &projet.mixage {
        let Some(reglage) = c.pistes.get_mut(piste.index) else { continue };
        *reglage = ReglagePiste {
            existe: true,
            volume: piste.volume,
            pan: piste.pan,
            audible: piste.index == 0 || audibles.contains(&piste.index),
            eq: [piste.eq.grave, piste.eq.medium, piste.eq.aigu],
            reverb: piste.reverb,
        };
    }
    c
}

/// Ce que le transport du moteur doit jouer.
pub fn session(projet: &Projet, mode: ModeLecture) -> Session {
    let (notes, longueur) = notes_a_jouer(projet, mode);
    Session {
        tempo: projet.tempo,
        longueur,
        temps_par_mesure: projet.signature.numerateur,
        notes: notes
            .into_iter()
            .filter(|n| (n.canal as usize) < CANAUX)
            .map(|n| NoteSession { debut: n.debut, fin: n.fin, canal: n.canal, hauteur: n.hauteur, velocite: n.velocite })
            .collect(),
        boucler: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use projet::demo::creer_demo;

    #[test]
    fn la_demo_vue_par_le_moteur() {
        let demo = creer_demo();
        let c = configuration(&demo);
        assert!(c.canaux[0].actif && c.canaux[1].actif && c.canaux[2].actif);
        assert!(c.canaux[9].actif && c.canaux[9].batterie);
        assert_eq!((c.canaux[0].piste, c.canaux[9].piste), (1, 4));
        assert!(c.pistes[0].existe && c.pistes[4].existe && !c.pistes[5].existe);
        let s = session(&demo, ModeLecture::Pattern);
        assert_eq!(s.longueur, 1536);
        assert_eq!(s.tempo, 100.0);
        assert!(!s.notes.is_empty());
    }

    #[test]
    fn solo_d_une_piste() {
        let mut demo = creer_demo();
        demo.mixage[2].solo = true;
        let c = configuration(&demo);
        assert_eq!((c.pistes[1].audible, c.pistes[2].audible, c.pistes[0].audible), (false, true, true));
    }
}
