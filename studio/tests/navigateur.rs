//! N2 : le navigateur (morceaux fournis, fichiers du disque), piloté comme par un utilisateur,
//! avec le vrai moteur.

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use studio::Studio;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1400.0, 900.0]).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    h.step();
    h
}

/// Fait tourner l'interface jusqu'à ce que la condition soit vraie (au plus 5 s).
fn attendre(h: &mut Harness<'static, Studio>, quoi: &str, condition: impl Fn(&Studio) -> bool) {
    let fin = Instant::now() + Duration::from_secs(5);
    while !condition(h.state()) {
        assert!(Instant::now() < fin, "attendu : {quoi}");
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
}

fn arreter(h: &mut Harness<'static, Studio>) {
    h.get_by_label("Stop").click();
    h.step();
    attendre(h, "l'arrêt", |s| s.etat_transport() == EtatTransport::Arret);
}

#[test]
fn le_navigateur_n_a_pas_de_recherche_en_ligne() {
    let h = studio();
    assert!(h.query_by_label_contains("RECHERCHER EN LIGNE").is_none());
    assert!(h.query_by_label("MORCEAUX").is_some());
}

#[test]
fn morceau_fourni_charge_et_joue() {
    let mut h = studio();
    assert_eq!(h.state().navigateur.morceaux.len(), 5);
    h.get_by_label("⏵ La Lettre à Élise").click();
    h.step();
    let p = &h.state().projet;
    assert_eq!((p.nom.as_str(), p.tempo, p.canaux.len()), ("La Lettre à Élise", 63.0, 2));
    attendre(&mut h, "la lecture", |s| s.etat_transport() == EtatTransport::Lecture);
    arreter(&mut h);
}

#[test]
fn ouvrir_des_fichiers_du_disque() {
    let mut h = studio();
    let dossier = std::env::temp_dir().join(format!("song-audio-fichiers-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    // un MIDI : importé sans jouer
    let midi = dossier.join("Prélude.mid");
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/morceaux/prelude-en-do.mid"), &midi).unwrap();
    h.state_mut().ouvrir_fichier(&midi);
    h.step();
    assert_eq!(h.state().projet.nom, "Prélude");
    assert_eq!(h.state().etat_transport(), EtatTransport::Arret);
    // un projet : ouvert tel quel
    let json = dossier.join("essai.song-audio.json");
    let mut p = projet::modele::creer_projet("Projet du disque");
    p.tempo = 77.0;
    std::fs::write(&json, projet::serialisation::serialiser(&p)).unwrap();
    h.state_mut().ouvrir_fichier(&json);
    h.step();
    assert_eq!((h.state().projet.nom.as_str(), h.state().projet.tempo), ("Projet du disque", 77.0));
    // un fichier abîmé : un message, le projet ne change pas
    let abime = dossier.join("abime.mid");
    std::fs::write(&abime, b"rien").unwrap();
    h.state_mut().ouvrir_fichier(&abime);
    h.step();
    assert!(h.state().message.as_deref().unwrap_or("").starts_with("Fichier MIDI illisible : abime"));
    assert_eq!(h.state().projet.nom, "Projet du disque");
    // la liste des fichiers du dossier
    h.state_mut().navigateur.changer_dossier(dossier.clone());
    let noms: Vec<String> = h.state_mut().navigateur.contenu(&dossier).into_iter().map(|e| e.nom).collect();
    assert_eq!(noms, vec!["abime.mid", "essai.song-audio.json", "Prélude.mid"]);
    std::fs::remove_dir_all(&dossier).ok();
}
