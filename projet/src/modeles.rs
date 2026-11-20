//! Modèles de projet : des points de départ prêts à jouer (instruments, tempo, un rythme en pas).
//! Le modèle « Vide » est le nouveau projet par défaut.

use crate::demo::creer_demo;
use crate::modele::{OptionsCanal, Projet, SourceCanal, creer_projet};
use crate::pas::definir_pas;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modele {
    Vide,
    HipHop,
    Electro,
    Orchestre,
    PianoSeul,
    Demo,
}

impl Modele {
    pub const TOUS: [Modele; 6] = [Modele::Vide, Modele::HipHop, Modele::Electro, Modele::Orchestre, Modele::PianoSeul, Modele::Demo];

    pub fn nom(self) -> &'static str {
        match self {
            Modele::Vide => "Vide",
            Modele::HipHop => "Hip-hop",
            Modele::Electro => "Électro",
            Modele::Orchestre => "Orchestre",
            Modele::PianoSeul => "Piano seul",
            Modele::Demo => "Démo",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Modele::Vide => "Quatre lignes de batterie, 120 BPM",
            Modele::HipHop => "Batterie, basse et piano électrique, 90 BPM, un peu de swing",
            Modele::Electro => "Grosse caisse à chaque temps, basse acide et nappe, 128 BPM",
            Modele::Orchestre => "Cordes, bois, cuivres, timbales, 80 BPM",
            Modele::PianoSeul => "Un piano, 100 BPM",
            Modele::Demo => "Quatre mesures en do majeur sur quatre instruments",
        }
    }

    pub fn creer(self) -> Projet {
        match self {
            Modele::Demo => creer_demo(),
            Modele::Vide => batterie("Nouveau projet", 120.0, &[("Grosse caisse", 36, ""), ("Clap", 39, ""), ("Charleston", 42, ""), ("Caisse claire", 38, "")], &[]),
            Modele::HipHop => {
                let mut p = batterie(
                    "Hip-hop",
                    90.0,
                    &[("Grosse caisse", 36, "x.....x...x....."), ("Caisse claire", 38, "....x.......x..."), ("Charleston", 42, "x.x.x.x.x.x.x.xx")],
                    &[("Basse", 33, SourceCanal::Soundfont), ("Piano électrique", 4, SourceCanal::Soundfont)],
                );
                p.swing = 0.25;
                p
            }
            Modele::Electro => batterie(
                "Électro",
                128.0,
                &[("Grosse caisse", 36, "x...x...x...x..."), ("Clap", 39, "....x.......x..."), ("Charleston ouvert", 46, "..x...x...x...x.")],
                // préréglages du synthé intégré : 1 = lead acide, 2 = nappe chaude
                &[("Basse acide", 1, SourceCanal::Soustractif), ("Nappe", 2, SourceCanal::Soustractif)],
            ),
            Modele::Orchestre => batterie(
                "Orchestre",
                80.0,
                &[],
                &[
                    ("Violons", 40, SourceCanal::Soundfont),
                    ("Altos", 41, SourceCanal::Soundfont),
                    ("Violoncelles", 42, SourceCanal::Soundfont),
                    ("Contrebasses", 43, SourceCanal::Soundfont),
                    ("Flûte", 73, SourceCanal::Soundfont),
                    ("Hautbois", 68, SourceCanal::Soundfont),
                    ("Clarinette", 71, SourceCanal::Soundfont),
                    ("Basson", 70, SourceCanal::Soundfont),
                    ("Cors", 60, SourceCanal::Soundfont),
                    ("Trompettes", 56, SourceCanal::Soundfont),
                    ("Trombones", 57, SourceCanal::Soundfont),
                    ("Timbales", 47, SourceCanal::Soundfont),
                ],
            ),
            Modele::PianoSeul => batterie("Piano seul", 100.0, &[], &[("Piano", 0, SourceCanal::Soundfont)]),
        }
    }
}

/// Projet avec des lignes de batterie (nom, son, motif en pas « x » / « . ») puis des instruments
/// mélodiques (nom, programme ou préréglage, source) ; un clip du pattern dans l'arrangement.
fn batterie(nom: &str, tempo: f64, lignes: &[(&str, u8, &str)], instruments: &[(&str, u8, SourceCanal)]) -> Projet {
    let mut p = creer_projet(nom);
    p.tempo = tempo;
    for (nom, son, motif) in lignes {
        let id = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some((*nom).into()), note_defaut: Some(*son), ..Default::default() });
        for (i, c) in motif.chars().enumerate() {
            if c == 'x' {
                definir_pas(&mut p, &id, i as u32, Some(true));
            }
        }
    }
    for (nom, programme, source) in instruments {
        p.ajouter_canal(OptionsCanal { nom: Some((*nom).into()), programme: *programme, source: *source, ..Default::default() });
    }
    let id = p.pattern_actif.clone();
    p.ajouter_clip(&id, 0, 0);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lecture::{ModeLecture, notes_a_jouer};
    use crate::serialisation::{deserialiser, serialiser};
    use crate::synthes::PREREGLAGES_SOUSTRACTIF;

    #[test]
    fn chaque_modele_est_un_projet_valide() {
        for m in Modele::TOUS {
            let p = m.creer();
            assert_eq!(deserialiser(&serialiser(&p)).unwrap(), p, "{}", m.nom());
            assert!(!p.canaux.is_empty(), "{}", m.nom());
            assert_eq!(p.playlist.len(), 1, "{} : un clip dans l'arrangement", m.nom());
            assert_eq!(p.mixage.len(), p.canaux.len() + 1, "{} : une piste par canal", m.nom());
        }
    }

    #[test]
    fn rythmes_tempos_et_sons() {
        let hip = Modele::HipHop.creer();
        assert_eq!((hip.tempo, hip.swing), (90.0, 0.25));
        assert_eq!(notes_a_jouer(&hip, ModeLecture::Pattern).0.len(), 3 + 2 + 9);
        let electro = Modele::Electro.creer();
        assert_eq!(electro.tempo, 128.0);
        let acide = electro.canaux.iter().find(|c| c.nom == "Basse acide").unwrap();
        assert_eq!(acide.source, SourceCanal::Soustractif);
        assert_eq!(PREREGLAGES_SOUSTRACTIF[acide.programme as usize].nom, "Lead acide");
        assert_eq!(PREREGLAGES_SOUSTRACTIF[2].nom, "Nappe chaude");
        assert_eq!(Modele::Orchestre.creer().canaux.len(), 12);
        let vide = Modele::Vide.creer();
        assert_eq!((vide.nom.as_str(), vide.canaux.len()), ("Nouveau projet", 4));
        assert!(notes_a_jouer(&vide, ModeLecture::Pattern).0.is_empty());
    }
}
