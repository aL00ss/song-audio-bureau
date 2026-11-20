//! N5 : le mixer, piloté comme par un utilisateur, avec le vrai moteur (les vumètres lisent ses
//! crêtes).

use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{self, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use studio::Studio;
use studio::coquille::Panneau;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1600.0, 1100.0]).with_step_dt(1.0 / 60.0).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    h.state_mut().montrer_panneau(Panneau::Mixage);
    h.run_steps(3);
    h
}

fn attendre(h: &mut Harness<'static, Studio>, quoi: &str, condition: impl Fn(&Studio) -> bool) {
    let fin = Instant::now() + Duration::from_secs(4);
    while !condition(h.state()) {
        assert!(Instant::now() < fin, "attendu : {quoi}");
        std::thread::sleep(Duration::from_millis(15));
        h.step();
    }
}

fn bouton(h: &mut Harness<'static, Studio>, pos: Pos2, appuye: bool) {
    h.event(egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: appuye, modifiers: Modifiers::NONE });
    h.step();
}

fn bouger(h: &mut Harness<'static, Studio>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
}

fn double_clic(h: &mut Harness<'static, Studio>, pos: Pos2) {
    bouger(h, pos);
    for appuye in [true, false, true, false] {
        bouton(h, pos, appuye);
    }
}

#[test]
fn vumetres_maitre_coupe_et_solo_pendant_la_lecture() {
    let mut h = studio();
    assert_eq!(h.state().projet.mixage.len(), 5);
    h.get_by_label("Lecture").click();
    attendre(&mut h, "le maître et la piste du piano bougent", |s| s.niveaux.first().is_some_and(|n| n[0] > 0.001) && s.niveaux.get(1).is_some_and(|n| n[0] > 0.001));
    // couper le maître : plus rien ne sort, le vumètre retombe
    h.get_by_label("Activer Maître").click();
    h.step();
    assert!(h.state().projet.mixage[0].muet);
    attendre(&mut h, "le maître silencieux", |s| s.niveaux[0][0] < 1e-4);
    h.get_by_label("Activer Maître").click();
    h.step();
    // solo de la batterie : les autres pistes se taisent, elle continue
    h.get_by_label("Solo Batterie").click();
    h.step();
    assert!(h.state().projet.mixage[4].solo);
    attendre(&mut h, "le piano tu, la batterie seule", |s| s.niveaux[1][0] < 1e-4 && s.niveaux[0][0] > 0.001);
    h.get_by_label("Stop").click();
    h.step();
    attendre(&mut h, "l'arrêt", |s| s.etat_transport() == EtatTransport::Arret);
}

#[test]
fn boutons_rotatifs_et_fader() {
    let mut h = studio();
    // glisser vers le haut : l'égaliseur monte ; double clic : retour à 0 dB
    let grave = h.get_by_label("Égaliseur Grave Piano").rect().center();
    bouger(&mut h, grave);
    bouton(&mut h, grave, true);
    for k in 1..=5 {
        bouger(&mut h, grave - egui::vec2(0.0, 8.0 * k as f32));
    }
    bouton(&mut h, grave - egui::vec2(0.0, 40.0), false);
    let g = h.state().projet.mixage[1].eq.grave;
    assert!((5.0..=7.5).contains(&g), "40 px vers le haut : environ +6,4 dB, obtenu {g}");
    double_clic(&mut h, grave);
    assert_eq!(h.state().projet.mixage[1].eq.grave, 0.0);
    // au clavier : flèche haut sur la réverbération
    let avant = h.state().projet.mixage[2].reverb;
    h.get_by_label("Réverbération Basse").focus();
    h.step();
    h.key_press(egui::Key::ArrowUp);
    h.step();
    assert!((h.state().projet.mixage[2].reverb - (avant + 0.02)).abs() < 1e-4);
    // le fader : double clic = 0 dB
    h.state_mut().modifier(|p| p.mixage[3].volume = 0.3);
    h.run_steps(2);
    let fader = h.get_by_label("Volume piste Cordes").rect().center();
    double_clic(&mut h, fader);
    assert_eq!(h.state().projet.mixage[3].volume, projet::mixage::FADER_UNITE);
}

#[test]
fn ajouter_renommer_retirer_et_une_piste_par_canal() {
    let mut h = studio();
    h.get_by_label("+ Piste").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.mixage.len(), 6);
    // renommer au double clic
    let nom = h.get_by_label("Piste 5").rect().center();
    double_clic(&mut h, nom);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.get(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput).value("Piste 5")).type_text("Bus");
    h.step();
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().projet.mixage[5].nom, "Bus");
    // la piste « Bus » ne reçoit rien : on peut la retirer
    h.get_by_label("Retirer les pistes inutilisées (1)").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.mixage.len(), 5);
    // deux canaux sur la même piste : « Une piste par canal » sépare
    h.state_mut().modifier(|p| p.canaux[1].piste_mixage = 1);
    h.run_steps(2);
    h.get_by_label("Une piste par canal").click();
    h.run_steps(2);
    let p = &h.state().projet;
    assert_eq!(p.canaux.iter().map(|c| c.piste_mixage).collect::<Vec<_>>(), vec![1, 5, 3, 4]);
    assert_eq!(p.mixage[5].nom, "Basse");
}

#[test]
fn trois_cents_pistes_seules_les_tranches_visibles_sont_dessinees() {
    let mut h = studio();
    h.state_mut().modifier(|p| {
        for _ in 0..300 {
            projet::mixage::ajouter_piste(p, None);
        }
    });
    h.run_steps(3);
    let dessinees = h.query_all_by_label_contains("Activer ").count();
    assert!((3..40).contains(&dessinees), "{dessinees} tranches dessinées sur 305");
    let debut = Instant::now();
    for _ in 0..30 {
        h.step();
    }
    let par_image = debut.elapsed() / 30;
    eprintln!("mixer, 305 pistes : {par_image:?} par image");
    assert!(par_image < Duration::from_millis(30));
    // le moteur a toutes les pistes
    attendre(&mut h, "le graphe à 305 pistes", |s| s.moteur().unwrap().taille_du_graphe().1 >= 305);
}
