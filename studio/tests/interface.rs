//! Tests de l'interface native : egui_kittest pilote la vraie application (clics, lecture de
//! l'écran par l'accessibilité), avec le vrai moteur (horloge silencieuse sans carte son).

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use studio::Studio;
use studio::coquille::Panneau;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1400.0, 800.0]).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    h.step();
    h
}

/// Attend (au plus 3 s) que le moteur ait atteint l'état voulu, en faisant tourner l'interface.
fn attendre_etat(h: &mut Harness<'static, Studio>, voulu: EtatTransport) {
    let fin = Instant::now() + Duration::from_secs(3);
    while h.state().etat_transport() != voulu {
        assert!(Instant::now() < fin, "état {:?} attendu, {:?} obtenu", voulu, h.state().etat_transport());
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
}

#[test]
fn lecture_pause_arret_par_les_boutons() {
    let mut h = studio();
    assert!(h.state().moteur().is_some(), "le moteur a démarré");
    h.get_by_label("Lecture").click();
    h.step();
    attendre_etat(&mut h, EtatTransport::Lecture);
    // la position avance pendant la lecture
    std::thread::sleep(Duration::from_millis(300));
    h.step();
    assert!(h.state().position() > 10.0, "position {}", h.state().position());
    h.get_by_label("Pause").click();
    h.step();
    attendre_etat(&mut h, EtatTransport::Pause);
    h.get_by_label("Stop").click();
    h.step();
    attendre_etat(&mut h, EtatTransport::Arret);
    assert_eq!(h.state().position(), 0.0);
}

#[test]
fn ajouter_des_instruments_et_le_titre_signale_les_modifications() {
    let mut h = studio();
    let avant = h.state().projet.canaux.len();
    assert_eq!(h.state().titre(), "Démo — song-audio");
    h.get_by_label("+ Batterie").click();
    h.step();
    h.get_by_label("+ Lead acide").click();
    h.step();
    let p = &h.state().projet;
    assert_eq!(p.canaux.len(), avant + 2);
    assert_eq!(p.canaux.last().unwrap().nom, "Lead acide");
    assert_eq!(h.state().titre(), "• Démo — song-audio");
    // le nouveau canal apparaît dans le rack (sa grille de pas), et il est choisi
    h.get_by_label("Pas 1 Lead acide");
    assert_eq!(h.state().canal_selectionne.as_deref(), Some(h.state().projet.canaux.last().unwrap().id.as_str()));
}

#[test]
fn enregistrer_puis_rouvrir_a_l_identique() {
    let mut h = studio();
    let chemin = std::env::temp_dir().join(format!("essai-natif-{}.song-audio.json", std::process::id()));
    h.state_mut().modifier(|p| p.tempo = 111.0);
    h.state_mut().enregistrer_dans(&chemin).unwrap();
    assert!(!h.state().modifie);
    let projet = h.state().projet.clone();
    h.state_mut().remplacer_projet(projet::modele::creer_projet("Autre"), None);
    h.state_mut().ouvrir(&chemin);
    h.step();
    assert_eq!(h.state().projet, projet);
    assert!(h.state().titre().starts_with("essai-natif-"));
    std::fs::remove_file(&chemin).ok();
}

#[test]
fn les_six_panneaux_sont_la_et_f6_ferme_puis_rouvre_le_rack() {
    let mut h = studio();
    for titre in ["Navigateur", "Rack d'instruments", "Arrangement", "Piano roll", "Instrument", "Mixage"] {
        assert!(h.query_all_by_label(titre).next().is_some(), "onglet {titre}");
    }
    assert_eq!(h.state().panneaux_ouverts().len(), 6);
    h.key_press(eframe::egui::Key::F6);
    h.step();
    assert!(!h.state().panneaux_ouverts().contains(&Panneau::Rack));
    h.key_press(eframe::egui::Key::F6);
    h.step();
    assert!(h.state().panneaux_ouverts().contains(&Panneau::Rack));
    // fermé par le bouton de la barre d'état, rétabli par « Disposition par défaut »
    h.state_mut().basculer_panneau(Panneau::Arrangement);
    h.step();
    assert_eq!(h.state().panneaux_ouverts().len(), 5);
    h.get_by_label("Disposition par défaut").click();
    h.step();
    assert_eq!(h.state().panneaux_ouverts().len(), 6);
}

#[test]
fn deux_cent_cinquante_instruments_qui_jouent_ensemble() {
    use projet::modele::{OptionsCanal, creer_projet};
    use projet::pas::definir_pas;
    let mut h = studio();
    // un projet de 250 instruments, chacun avec un pas au début de chaque temps
    let mut p = creer_projet("Grand orchestre");
    for i in 0..250u32 {
        let id = p.ajouter_canal(OptionsCanal { programme: (i % 120) as u8, volume: Some(0.3), ..Default::default() });
        for pas in [0, 4, 8, 12] {
            definir_pas(&mut p, &id, pas, Some(true));
        }
    }
    h.state_mut().remplacer_projet(p, None);
    let depart = Instant::now();
    h.step();
    h.get_by_label("Lecture").click();
    h.step();
    attendre_etat(&mut h, EtatTransport::Lecture);
    // le graphe du moteur a grandi jusqu'à 250 canaux et 251 pistes
    assert_eq!(h.state().moteur().unwrap().taille_du_graphe(), (250, 251));
    // le dernier instrument sonne vraiment
    let fin = Instant::now() + Duration::from_secs(5);
    loop {
        h.step();
        let voix = h.state().moteur().unwrap().mesures().voix;
        if voix.get(249).copied().unwrap_or(0) > 0 && voix.iter().filter(|&&v| v > 0).count() > 100 {
            break;
        }
        assert!(Instant::now() < fin, "le 250e instrument ne joue pas");
        std::thread::sleep(Duration::from_millis(20));
    }
    // et l'interface reste fluide : une image prend bien moins qu'une trame de 60 Hz
    let image = Instant::now();
    for _ in 0..30 {
        h.step();
    }
    let par_image = image.elapsed() / 30;
    eprintln!("250 instruments : mis en place en {:?}, {:?} par image", depart.elapsed(), par_image);
    assert!(par_image < Duration::from_millis(50), "image trop lente : {par_image:?}");
    h.get_by_label("Stop").click();
    h.step();
}
