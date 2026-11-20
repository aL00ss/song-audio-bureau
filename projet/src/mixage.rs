//! Mixage : décibels, loi du fader, vumètres (repris de la version web avec ses tests), et
//! actions sur les pistes (ajouter, renommer, retirer les pistes inutilisées, donner à chaque
//! canal sa piste). Fonctions pures. L'index d'une piste est sa place dans la liste : le maître
//! est la piste 0.

use crate::modele::{Projet, creer_piste};

/// Position du fader (0 à 1) qui donne le gain unité (0 dB).
pub const FADER_UNITE: f32 = 0.8;

pub fn db_vers_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_vers_db(gain: f32) -> f32 {
    if gain <= 0.0 { f32::NEG_INFINITY } else { 20.0 * gain.log10() }
}

/// Loi du fader : quadratique, 0,8 = 0 dB, 1 = +3,9 dB, 0 = silence (la même que le moteur).
pub fn gain_du_fader(position: f32) -> f32 {
    (position.clamp(0.0, 1.0) / FADER_UNITE).powi(2)
}

pub fn texte_db(gain: f32) -> String {
    let db = gain_vers_db(gain);
    if !db.is_finite() || db < -60.0 {
        return "-∞ dB".into();
    }
    format!("{}{:.1} dB", if db >= 0.0 { "+" } else { "" }, db)
}

/// Part de la hauteur d'un vumètre allant de -60 à +6 dB.
pub fn hauteur_vumetre(crete: f32) -> f32 {
    let db = gain_vers_db(crete);
    if !db.is_finite() { 0.0 } else { ((db + 60.0) / 66.0).clamp(0.0, 1.0) }
}

/// Retombée du vumètre : environ 20 dB par seconde, quelle que soit la cadence d'affichage.
pub fn retombee(niveau: f32, crete: f32, secondes: f32) -> f32 {
    crete.max(niveau * 0.85f32.powf(secondes * 60.0))
}

/// Texte d'un panoramique : « centre », « 40 % gauche »…
pub fn texte_pan(pan: f32) -> String {
    if pan.abs() < 0.01 {
        "centre".into()
    } else {
        format!("{} % {}", (pan.abs() * 100.0).round(), if pan < 0.0 { "gauche" } else { "droite" })
    }
}

/// Ajoute une piste à la fin ; renvoie son index.
pub fn ajouter_piste(projet: &mut Projet, nom: Option<&str>) -> usize {
    let index = projet.mixage.len();
    let nom = nom.map(str::to_string).unwrap_or_else(|| format!("Piste {index}"));
    projet.mixage.push(creer_piste(index, &nom));
    index
}

pub fn renommer_piste(projet: &mut Projet, index: usize, nom: &str) -> bool {
    let nom = nom.trim();
    match projet.mixage.get_mut(index) {
        Some(p) if !nom.is_empty() => {
            p.nom = nom.to_string();
            true
        }
        _ => false,
    }
}

/// Pistes (hors maître) qui ne reçoivent aucun canal.
pub fn pistes_inutilisees(projet: &Projet) -> Vec<usize> {
    projet.mixage.iter().map(|p| p.index).filter(|&i| i != 0 && !projet.canaux.iter().any(|c| c.piste_mixage == i)).collect()
}

/// Retire les pistes qui ne reçoivent aucun canal ; les autres sont renumérotées dans l'ordre et
/// les canaux suivent leur piste. Renvoie le nombre de pistes retirées.
pub fn retirer_pistes_inutilisees(projet: &mut Projet) -> usize {
    let inutiles = pistes_inutilisees(projet);
    if inutiles.is_empty() {
        return 0;
    }
    let anciennes = std::mem::take(&mut projet.mixage);
    let mut nouvel_index = vec![0usize; anciennes.len()];
    for mut piste in anciennes.into_iter().filter(|p| !inutiles.contains(&p.index)) {
        nouvel_index[piste.index] = projet.mixage.len();
        piste.index = projet.mixage.len();
        projet.mixage.push(piste);
    }
    for c in projet.canaux.iter_mut() {
        c.piste_mixage = nouvel_index.get(c.piste_mixage).copied().unwrap_or(0);
    }
    inutiles.len()
}

/// Donne à chaque canal qui partage sa piste (ou va au maître) une piste à lui, nommée comme
/// lui. Renvoie le nombre de pistes créées.
pub fn une_piste_par_canal(projet: &mut Projet) -> usize {
    let mut deja = std::collections::HashSet::new();
    let mut creees = 0;
    for i in 0..projet.canaux.len() {
        let piste = projet.canaux[i].piste_mixage;
        if piste != 0 && piste < projet.mixage.len() && deja.insert(piste) {
            continue;
        }
        let nom = projet.canaux[i].nom.clone();
        let index = ajouter_piste(projet, Some(&nom));
        projet.canaux[i].piste_mixage = index;
        deja.insert(index);
        creees += 1;
    }
    creees
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::{OptionsCanal, creer_projet};
    use crate::serialisation::{deserialiser, serialiser};

    // ---- les tests de la version web (niveaux.test.ts), traduits

    #[test]
    fn decibels_et_gain() {
        assert_eq!(db_vers_gain(0.0), 1.0);
        assert!((db_vers_gain(-6.0) - 0.501).abs() < 0.001);
        assert_eq!(gain_vers_db(1.0), 0.0);
        assert_eq!(gain_vers_db(0.0), f32::NEG_INFINITY);
        assert!((gain_vers_db(db_vers_gain(-12.5)) - -12.5).abs() < 1e-4);
    }

    #[test]
    fn solo_et_muet_le_maitre_n_est_jamais_concerne() {
        use crate::modele::pistes_audibles;
        use std::collections::HashSet;
        let mut pistes: Vec<_> = ["Maître", "A", "B", "C"].iter().enumerate().map(|(i, n)| creer_piste(i, n)).collect();
        assert_eq!(pistes_audibles(&pistes), HashSet::from([1, 2, 3]));
        pistes[2].muet = true;
        assert_eq!(pistes_audibles(&pistes), HashSet::from([1, 3]));
        pistes[3].solo = true;
        assert_eq!(pistes_audibles(&pistes), HashSet::from([3]));
        pistes[2].solo = true; // une piste muette mise en solo sonne
        assert_eq!(pistes_audibles(&pistes), HashSet::from([2, 3]));
        pistes[0].solo = true; // solo du maître : sans effet
        assert_eq!(pistes_audibles(&pistes), HashSet::from([2, 3]));
    }

    #[test]
    fn loi_du_fader_et_decibels() {
        assert!((gain_du_fader(0.8) - 1.0).abs() < 1e-6);
        assert_eq!(gain_du_fader(0.0), 0.0);
        assert!((gain_vers_db(gain_du_fader(1.0)) - 3.876).abs() < 0.01);
        assert!((gain_vers_db(gain_du_fader(0.4)) - -12.04).abs() < 0.01);
        assert_eq!(gain_du_fader(2.0), gain_du_fader(1.0));
        assert!((db_vers_gain(-6.0206) - 0.5).abs() < 1e-4);
        assert_eq!(texte_db(1.0), "+0.0 dB");
        assert_eq!(texte_db(0.5), "-6.0 dB");
        assert_eq!(texte_db(0.0), "-∞ dB");
        assert_eq!(texte_db(0.0001), "-∞ dB");
    }

    #[test]
    fn vumetre_de_moins_60_a_plus_6_db() {
        assert_eq!(hauteur_vumetre(0.0), 0.0);
        assert!((hauteur_vumetre(1.0) - 60.0 / 66.0).abs() < 1e-6);
        assert_eq!(hauteur_vumetre(10.0), 1.0);
        assert_eq!(hauteur_vumetre(0.0005), 0.0);
        assert!(hauteur_vumetre(db_vers_gain(-60.0)).abs() < 1e-5);
        assert_eq!(hauteur_vumetre(4.0), 1.0);
        // retombée : 20 dB par seconde environ, une crête plus forte prend le dessus
        assert!((gain_vers_db(retombee(1.0, 0.0, 1.0)) - -84.7).abs() < 1.0);
        assert_eq!(retombee(0.1, 0.5, 0.016), 0.5);
        assert_eq!(texte_pan(0.0), "centre");
        assert_eq!(texte_pan(-0.4), "40 % gauche");
        assert_eq!(texte_pan(1.0), "100 % droite");
    }

    fn projet_de_test() -> Projet {
        let mut p = creer_projet("Essai");
        for nom in ["A", "B", "C"] {
            p.ajouter_canal(OptionsCanal { nom: Some(nom.into()), ..Default::default() });
        }
        p
    }

    #[test]
    fn ajouter_et_renommer() {
        let mut p = projet_de_test();
        assert_eq!(ajouter_piste(&mut p, None), 4);
        assert_eq!(p.mixage[4].nom, "Piste 4");
        assert!(renommer_piste(&mut p, 4, "  Bus  "));
        assert_eq!(p.mixage[4].nom, "Bus");
        assert!(!renommer_piste(&mut p, 4, " "));
        assert!(!renommer_piste(&mut p, 99, "x"));
    }

    #[test]
    fn retirer_les_pistes_inutilisees_renumerote_et_garde_le_routage() {
        let mut p = projet_de_test();
        ajouter_piste(&mut p, Some("Vide"));
        p.canaux[1].piste_mixage = 0; // B va au maître : sa piste 2 ne sert plus
        p.canaux[2].piste_mixage = 4; // C va sur « Vide » : sa piste 3 ne sert plus
        p.mixage[4].reverb = 0.5;
        assert_eq!(pistes_inutilisees(&p), vec![2, 3]);
        assert_eq!(retirer_pistes_inutilisees(&mut p), 2);
        assert_eq!(p.mixage.iter().map(|m| (m.index, m.nom.as_str())).collect::<Vec<_>>(), vec![(0, "Maître"), (1, "A"), (2, "Vide")]);
        assert_eq!(p.canaux.iter().map(|c| c.piste_mixage).collect::<Vec<_>>(), vec![1, 0, 2]);
        assert_eq!(p.mixage[2].reverb, 0.5, "les réglages suivent la piste");
        assert_eq!(retirer_pistes_inutilisees(&mut p), 0);
        assert_eq!(deserialiser(&serialiser(&p)).unwrap(), p);
    }

    #[test]
    fn une_piste_par_canal() {
        let mut p = projet_de_test();
        p.canaux[1].piste_mixage = 1; // B partage la piste de A
        p.canaux[2].piste_mixage = 0; // C va au maître
        assert_eq!(super::une_piste_par_canal(&mut p), 2);
        assert_eq!(p.canaux.iter().map(|c| c.piste_mixage).collect::<Vec<_>>(), vec![1, 4, 5]);
        assert_eq!((p.mixage[4].nom.as_str(), p.mixage[5].nom.as_str()), ("B", "C"));
        assert_eq!(super::une_piste_par_canal(&mut p), 0, "déjà fait");
    }
}
