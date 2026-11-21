//! Import d'un projet FL Studio dans l'application : le projet d'essai de PyFLP, ouvert comme
//! par un utilisateur, puis joué par le vrai moteur.

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use projet::lecture::ModeLecture;
use studio::Studio;

#[test]
fn ouvrir_un_projet_fl_studio_et_le_jouer() {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).unwrap();
    let mut h = Harness::builder().with_size([1600.0, 1000.0]).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    h.step();
    let flp = Path::new(env!("CARGO_MANIFEST_DIR")).join("../projet/tests/donnees/flp/PyFLP - FL 20.8.4.flp");
    h.state_mut().ouvrir_fichier(&flp);
    h.run_steps(3);
    let s = h.state();
    assert_eq!(s.projet.nom, "PyFLP Test FLP");
    assert_eq!(s.projet.canaux.len(), 17);
    assert_eq!(s.mode, ModeLecture::Morceau, "le projet a un arrangement");
    assert!(s.message.as_deref().unwrap_or("").starts_with("Projet FL Studio « PyFLP - FL 20.8.4 » importé : 17 canaux"), "{:?}", s.message);
    assert!(s.modifie, "pas encore enregistré comme projet song-audio");
    // la fenêtre du rapport s'ouvre et liste les remplacements
    assert!(h.query_by_label("Rapport d'import").is_some());
    assert!(h.query_all_by_label_contains("BooBass").count() >= 1);
    // il joue
    h.get_by_label("Lecture").click();
    let fin = Instant::now() + Duration::from_secs(4);
    while h.state().etat_transport() != EtatTransport::Lecture {
        assert!(Instant::now() < fin, "état {:?}, message {:?}, longueur {:?}", h.state().etat_transport(), h.state().message, h.state().moteur().map(|m| m.lecture()));
        std::thread::sleep(Duration::from_millis(15));
        h.step();
    }
    h.get_by_label("Stop").click();
    h.step();
    // un fichier abîmé : un message clair, le projet ne change pas
    let abime = std::env::temp_dir().join(format!("abime-{}.flp", std::process::id()));
    std::fs::write(&abime, b"FLhd").unwrap();
    h.state_mut().ouvrir_fichier(&abime);
    h.step();
    assert!(h.state().message.as_deref().unwrap_or("").starts_with("Projet FL Studio illisible"));
    assert_eq!(h.state().projet.nom, "PyFLP Test FLP");
    std::fs::remove_file(&abime).ok();
}
