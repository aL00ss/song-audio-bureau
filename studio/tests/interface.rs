//! Tests de l'interface native : egui_kittest pilote la vraie application (clics, lecture de
//! l'écran par l'accessibilité), avec le vrai moteur (horloge silencieuse sans carte son).

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use studio::Studio;

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
    // le nouveau canal apparaît dans le rack
    h.get_by_label("Lead acide");
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
