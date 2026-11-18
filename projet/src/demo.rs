//! Projet de démonstration écrit dans le code : quatre mesures en do majeur, Do – La mineur –
//! Fa – Sol, sur quatre instruments (le même que la version web).

use crate::modele::{NoteMusicale, OptionsCanal, Projet, creer_projet};
use crate::temps::{SIGNATURE_DEFAUT, TICS_PAR_NOIRE, tics_par_mesure};

const ACCORDS: [[u8; 3]; 4] = [[60, 64, 67], [57, 60, 64], [53, 57, 60], [55, 59, 62]];
const BASSES: [u8; 4] = [36, 33, 29, 31];

fn note(tic: u32, duree: u32, hauteur: u8, velocite: u8) -> NoteMusicale {
    NoteMusicale { tic, duree, hauteur, velocite }
}

pub fn creer_demo() -> Projet {
    let mesure = tics_par_mesure(SIGNATURE_DEFAUT);
    let croche = TICS_PAR_NOIRE / 2;
    let mut p = creer_projet("Démo");
    let piano = p.ajouter_canal(OptionsCanal { nom: Some("Piano".into()), programme: 0, ..Default::default() });
    let basse = p.ajouter_canal(OptionsCanal { nom: Some("Basse".into()), programme: 33, ..Default::default() });
    let cordes = p.ajouter_canal(OptionsCanal { nom: Some("Cordes".into()), programme: 48, volume: Some(0.65), ..Default::default() });
    let batterie = p.ajouter_canal(OptionsCanal { nom: Some("Batterie".into()), batterie: true, ..Default::default() });

    let (mut n_piano, mut n_basse, mut n_cordes, mut n_batterie) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (m, accord) in ACCORDS.iter().enumerate() {
        let debut = m as u32 * mesure;
        // piano : l'accord en noires
        for temps in 0..4 {
            for &h in accord {
                n_piano.push(note(debut + temps * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, h, if temps == 0 { 96 } else { 78 }));
            }
        }
        // basse : fondamentale, quinte, fondamentale à l'octave
        let f = BASSES[m];
        n_basse.push(note(debut, TICS_PAR_NOIRE * 2 - 8, f, 100));
        n_basse.push(note(debut + 2 * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, f + 7, 90));
        n_basse.push(note(debut + 3 * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, f + 12, 90));
        // cordes : l'accord tenu, une octave plus bas
        for &h in accord {
            n_cordes.push(note(debut, mesure - 4, h - 12, 70));
        }
        // batterie : grosse caisse sur 1 et 3, caisse claire sur 2 et 4, charleston en croches
        for i in 0..8 {
            n_batterie.push(note(debut + i * croche, croche / 2, 42, if i % 2 == 0 { 90 } else { 64 }));
        }
        for temps in [0, 2] {
            n_batterie.push(note(debut + temps * TICS_PAR_NOIRE, croche / 2, 36, 110));
        }
        for temps in [1, 3] {
            n_batterie.push(note(debut + temps * TICS_PAR_NOIRE, croche / 2, 38, 100));
        }
    }
    n_batterie.push(note(0, croche, 49, 100)); // crash au début

    p.tempo = 100.0;
    let pattern = p.pattern_actif_mut();
    pattern.nom = "Démo".into();
    pattern.longueur = 4 * mesure;
    for (id, mut notes) in [(piano, n_piano), (basse, n_basse), (cordes, n_cordes), (batterie, n_batterie)] {
        notes.sort_by_key(|n| (n.tic, n.hauteur));
        pattern.notes.insert(id, notes);
    }
    let id_pattern = pattern.id.clone();
    p.ajouter_clip(&id_pattern, 0, 0);
    p
}
