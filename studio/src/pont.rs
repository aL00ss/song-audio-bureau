//! Le projet vu par le moteur : réglages des canaux et des pistes, et session à jouer.
//! Fonctions pures (testées). Le numéro d'un canal dans le moteur est son rang dans le projet,
//! celui d'une piste son index de mixage : aucune limite de nombre.

use moteur::{Configuration, NoteSession, ReglageCanal, ReglagePiste, Session, TypeSource};
use projet::lecture::{ModeLecture, notes_a_jouer};
use projet::modele::pistes_audibles;
use projet::{Projet, SourceCanal};

/// Réglages du mixer et des canaux pour le moteur.
pub fn configuration(projet: &Projet) -> Configuration {
    let nb_pistes = projet.mixage.iter().map(|p| p.index + 1).max().unwrap_or(1);
    let mut c = Configuration::avec(projet.canaux.len(), nb_pistes);
    let audibles = pistes_audibles(&projet.mixage);
    for piste in &projet.mixage {
        c.pistes[piste.index] = ReglagePiste {
            existe: true,
            volume: piste.volume,
            pan: piste.pan,
            audible: piste.index == 0 || audibles.contains(&piste.index),
            eq: [piste.eq.grave, piste.eq.medium, piste.eq.aigu],
            reverb: piste.reverb,
        };
    }
    for (rang, canal) in projet.canaux.iter().enumerate() {
        let piste = if c.pistes.get(canal.piste_mixage).is_some_and(|p| p.existe) { canal.piste_mixage } else { 0 };
        c.canaux[rang] = ReglageCanal {
            actif: true,
            programme: canal.programme,
            batterie: canal.batterie,
            volume: canal.volume,
            pan: canal.pan,
            piste: piste as u32,
            source: match canal.source {
                SourceCanal::Soustractif if !canal.batterie => TypeSource::Soustractif,
                _ => TypeSource::SoundFont,
            },
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
            .map(|n| NoteSession { debut: n.debut, fin: n.fin, canal: n.canal, hauteur: n.hauteur, velocite: n.velocite })
            .collect(),
        boucler: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use projet::demo::creer_demo;
    use projet::modele::OptionsCanal;

    #[test]
    fn la_demo_vue_par_le_moteur() {
        let demo = creer_demo();
        let c = configuration(&demo);
        assert_eq!(c.canaux.len(), 4);
        assert!(c.canaux.iter().all(|r| r.actif));
        assert!(c.canaux[3].batterie);
        assert_eq!((c.canaux[0].piste, c.canaux[3].piste), (1, 4));
        assert_eq!(c.pistes.len(), 5);
        assert!(c.pistes.iter().all(|p| p.existe));
        let s = session(&demo, ModeLecture::Pattern);
        assert_eq!(s.longueur, 1536);
        assert_eq!(s.tempo, 100.0);
        assert!(s.notes.iter().any(|n| n.canal == 3));
    }

    #[test]
    fn solo_d_une_piste() {
        let mut demo = creer_demo();
        demo.mixage[2].solo = true;
        let c = configuration(&demo);
        assert_eq!((c.pistes[1].audible, c.pistes[2].audible, c.pistes[0].audible), (false, true, true));
    }

    #[test]
    fn trois_cents_instruments() {
        let mut p = projet::modele::creer_projet("Grand orchestre");
        for i in 0..300u32 {
            p.ajouter_canal(OptionsCanal { programme: (i % 128) as u8, ..Default::default() });
        }
        let c = configuration(&p);
        assert_eq!((c.canaux.len(), c.pistes.len()), (300, 301));
        assert_eq!(c.canaux[299].piste, 300);
    }
}
