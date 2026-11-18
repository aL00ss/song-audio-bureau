//! Budget de performance : le temps réel est une exigence vérifiée. Ces tests n'ont de sens que
//! compilés avec optimisations (`cargo test --release`) ; en mode débogage ils sont ignorés.

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::noyau::tests::{FE, banque, regler};
    use crate::noyau::{Configuration, Evenement, Message, Noyau, ReglageCanal, ReglagePiste};
    use crate::source::TypeSource;

    /// Rend `secondes` en blocs de 256 (comme une carte son) ; renvoie (facteur temps réel, crête).
    fn chronometrer(noyau: &mut Noyau, secondes: f64) -> (f64, f32) {
        let n = (secondes * FE as f64) as usize;
        let (mut g, mut d) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        let depart = Instant::now();
        let mut crete = 0f32;
        for _ in 0..n / 256 {
            noyau.rendre(&mut g, &mut d);
            crete = g.iter().fold(crete, |m, v| m.max(v.abs()));
        }
        (secondes / depart.elapsed().as_secs_f64(), crete)
    }

    /// `jouent` canaux sur `total` reçoivent des accords de `notes` notes, rejoués toutes les
    /// demi-secondes ; les autres sont chargés mais muets.
    fn orchestre(total: usize, jouent: usize, notes: usize, secondes: f64) -> Noyau {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut c = Configuration::avec(total, total + 1);
        let programmes = [0u8, 4, 24, 33, 40, 48, 56, 61, 73, 89];
        for canal in 0..total {
            let synthe = canal % 5 == 4;
            let source = if synthe { TypeSource::Soustractif } else { TypeSource::SoundFont };
            let programme = if synthe { (canal % 6) as u8 } else { programmes[canal % programmes.len()] };
            c.canaux[canal] = ReglageCanal { actif: true, programme, batterie: false, volume: 0.6, pan: 0.0, piste: (canal + 1) as u32, source };
            c.pistes[canal + 1] = ReglagePiste { existe: true, reverb: 0.2, eq: [2.0, -1.0, 1.5], ..Default::default() };
        }
        regler(&mut noyau, &c);
        for coup in 0..(secondes * 2.0) as u64 {
            let debut = coup * FE as u64 / 2;
            for canal in 0..jouent as u32 {
                for (k, intervalle) in [0u8, 4, 7, 12, 16, 19].iter().take(notes).enumerate() {
                    let hauteur = 40 + (canal % 12) as u8 + intervalle;
                    let e = |echantillon, message| Evenement { echantillon, canal, message };
                    noyau.programmer(e(debut + k as u64, Message::NoteOn { hauteur, velocite: 100 }));
                    noyau.programmer(e(debut + FE as u64 * 2 / 5, Message::NoteOff { hauteur }));
                }
            }
        }
        noyau
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "mesure de performance : seulement avec --release")]
    fn seize_instruments_charges_bien_plus_vite_que_le_temps_reel() {
        let mut noyau = orchestre(16, 16, 6, 10.0);
        let (facteur, crete) = chronometrer(&mut noyau, 10.0);
        eprintln!("16 instruments, 96 voix : {facteur:.1} fois le temps réel ({} fils de calcul)", noyau.fils_de_calcul());
        assert!(crete > 0.05, "le rendu n'est pas silencieux");
        assert!(facteur > 4.0, "seulement {facteur:.1} fois le temps réel (exigé : 4)");
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "mesure de performance : seulement avec --release")]
    fn deux_cents_instruments_dont_cent_vingt_jouent_ensemble_en_temps_reel() {
        let mut noyau = orchestre(200, 120, 4, 6.0);
        let (facteur, crete) = chronometrer(&mut noyau, 6.0);
        eprintln!(
            "200 instruments chargés, 120 jouent (480 voix), 200 pistes avec égaliseur : {facteur:.1} fois le temps réel ({} fils de calcul)",
            noyau.fils_de_calcul()
        );
        assert!(crete > 0.05, "le rendu n'est pas silencieux");
        // le temps réel avec une marge : les blocs ne doivent jamais arriver en retard
        assert!(facteur > 1.5, "seulement {facteur:.1} fois le temps réel (exigé : 1,5)");
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "mesure de performance : seulement avec --release")]
    fn cinq_cents_instruments_charges_au_repos_ne_coutent_presque_rien() {
        let mut noyau = orchestre(500, 0, 0, 1.0);
        let (facteur, _) = chronometrer(&mut noyau, 5.0);
        eprintln!("500 instruments chargés au repos : {facteur:.0} fois le temps réel");
        assert!(facteur > 50.0, "les canaux endormis coûtent encore : {facteur:.0} fois le temps réel");
    }
}
