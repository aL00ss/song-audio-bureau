//! Budget de performance : le temps réel est une exigence vérifiée. Ce test n'a de sens que
//! compilé avec optimisations (`cargo test --release`) ; en mode débogage il est ignoré.

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::noyau::tests::{FE, banque};
    use crate::noyau::{Configuration, Evenement, Message, Noyau, ReglageCanal, ReglagePiste};
    use crate::source::TypeSource;

    /// Marge exigée : le moteur doit rendre au moins 4 fois plus vite que le temps réel.
    const MARGE: f64 = 4.0;

    #[test]
    #[cfg_attr(debug_assertions, ignore = "mesure de performance : seulement avec --release")]
    fn seize_instruments_charges_bien_plus_vite_que_le_temps_reel() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut c = Configuration::default();
        let programmes = [0u8, 4, 24, 33, 40, 48, 56, 61, 73, 89];
        for canal in 0..16usize {
            let source = if canal < 10 { TypeSource::SoundFont } else { TypeSource::Soustractif };
            let programme = if canal < 10 { programmes[canal] } else { (canal % 6) as u8 };
            c.canaux[canal] = ReglageCanal { actif: true, programme, batterie: false, volume: 0.7, pan: 0.0, piste: (canal + 1) as u8, source };
            c.pistes[canal + 1] = ReglagePiste { existe: true, reverb: 0.2, eq: [2.0, -1.0, 1.5], ..Default::default() };
        }
        noyau.configurer(&c);

        let secondes = 10.0f64;
        // accords de six notes rejoués toutes les demi-secondes sur chaque canal
        for coup in 0..(secondes * 2.0) as u64 {
            let debut = coup * FE as u64 / 2;
            for canal in 0..16u8 {
                for (k, intervalle) in [0u8, 4, 7, 12, 16, 19].iter().enumerate() {
                    let hauteur = 40 + canal % 12 + intervalle;
                    let e = |echantillon, message| Evenement { echantillon, canal, message };
                    noyau.programmer(e(debut + k as u64, Message::NoteOn { hauteur, velocite: 100 }));
                    noyau.programmer(e(debut + FE as u64 * 2 / 5, Message::NoteOff { hauteur }));
                }
            }
        }

        let n = (secondes * FE as f64) as usize;
        let (mut g, mut d) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        let depart = Instant::now();
        let mut crete = 0f32;
        for _ in 0..n / 256 {
            noyau.rendre(&mut g, &mut d);
            crete = g.iter().fold(crete, |m, v| m.max(v.abs()));
        }
        let duree = depart.elapsed().as_secs_f64();
        let facteur = secondes / duree;
        eprintln!("16 instruments, 96 voix : {secondes} s rendues en {duree:.2} s, {facteur:.1} fois le temps réel");
        assert!(crete > 0.05, "le rendu n'est pas silencieux");
        assert!(facteur > MARGE, "seulement {facteur:.1} fois le temps réel (exigé : {MARGE})");
    }
}
