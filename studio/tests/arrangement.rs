//! N6 : l'arrangement (patterns, clips, mode morceau), piloté comme par un utilisateur, avec le
//! vrai moteur.

use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{self, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use projet::lecture::ModeLecture;
use projet::modele::{OptionsCanal, creer_projet};
use studio::Studio;

const MESURE: u32 = 384;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1600.0, 1000.0]).with_step_dt(1.0 / 60.0).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    let mut p = creer_projet("Essai");
    p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), note_defaut: Some(36), ..Default::default() });
    h.state_mut().remplacer_projet(p, None);
    h.run_steps(3);
    h
}

fn ecran(h: &Harness<'static, Studio>, piste: u32, tic: f64) -> Pos2 {
    h.state().arrangement.vers_ecran(piste, tic, MESURE)
}

fn bouger(h: &mut Harness<'static, Studio>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
}

fn bouton(h: &mut Harness<'static, Studio>, pos: Pos2, b: PointerButton, appuye: bool) {
    h.event(egui::Event::PointerButton { pos, button: b, pressed: appuye, modifiers: Modifiers::NONE });
    h.step();
}

fn glisser(h: &mut Harness<'static, Studio>, a: Pos2, b: Pos2, bt: PointerButton) {
    bouger(h, a);
    bouton(h, a, bt, true);
    for k in 1..=4 {
        bouger(h, a + (b - a) * (k as f32 / 4.0));
    }
    bouton(h, b, bt, false);
}

fn clips(h: &Harness<'static, Studio>) -> Vec<(u32, u32)> {
    h.state().projet.playlist.iter().map(|c| (c.piste, c.debut)).collect()
}

#[test]
fn poser_deplacer_supprimer_des_clips_aimantes_a_la_mesure() {
    let mut h = studio();
    // projet sans clip
    assert!(clips(&h).is_empty());
    // clic au milieu de la 3e mesure de la piste 2 : un clip au début de la mesure
    let p = ecran(&h, 1, 2.5 * MESURE as f64);
    glisser(&mut h, p, p, PointerButton::Primary);
    assert_eq!(clips(&h), vec![(1, 2 * MESURE)]);
    assert!(h.query_by_label("Clip Pattern 1 piste 2 mesure 3").is_some());
    // glisser : une mesure plus loin et une piste plus bas
    let (a, b) = (ecran(&h, 1, 2.2 * MESURE as f64), ecran(&h, 2, 3.2 * MESURE as f64));
    glisser(&mut h, a, b, PointerButton::Primary);
    assert_eq!(clips(&h), vec![(2, 3 * MESURE)]);
    // un deuxième pattern : il devient actif, et c'est lui qu'on pose
    h.get_by_label("+ Nouveau").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.patterns.len(), 2);
    let p = ecran(&h, 0, 0.1 * MESURE as f64);
    glisser(&mut h, p, p, PointerButton::Primary);
    let pattern_2 = h.state().projet.pattern_actif.clone();
    assert_eq!(h.state().projet.playlist[1].pattern, pattern_2);
    // double clic sur le premier clip : son pattern redevient actif
    let p = ecran(&h, 2, 3.5 * MESURE as f64);
    bouger(&mut h, p);
    for appuye in [true, false, true, false] {
        bouton(&mut h, p, PointerButton::Primary, appuye);
    }
    assert_eq!(h.state().projet.pattern_actif, h.state().projet.patterns[0].id);
    assert_eq!(clips(&h), vec![(2, 3 * MESURE), (0, 0)], "le double clic ne déplace rien");
    // clic droit glissé : efface les clips survolés
    let (a, b) = (ecran(&h, 0, 0.5 * MESURE as f64), ecran(&h, 2, 3.5 * MESURE as f64));
    bouger(&mut h, a);
    bouton(&mut h, a, PointerButton::Secondary, true);
    let milieu = ecran(&h, 1, 2.0 * MESURE as f64);
    bouger(&mut h, milieu);
    bouger(&mut h, b);
    bouton(&mut h, b, PointerButton::Secondary, false);
    assert!(clips(&h).is_empty());
}

#[test]
fn patterns_renommer_dupliquer_supprimer_et_choisir_dans_le_rack() {
    let mut h = studio();
    let id = h.state().projet.canaux[0].id.clone();
    h.state_mut().modifier(|p| projet::pas::definir_pas(p, &id, 0, None));
    h.run_steps(2);
    // dupliquer par le menu : la copie a les notes et devient active
    h.get_by_label("Pattern 1").click_secondary();
    h.run_steps(2);
    h.get_by_label("Dupliquer").click();
    h.run_steps(2);
    let p = &h.state().projet;
    assert_eq!(p.patterns.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>(), vec!["Pattern 1", "Pattern 1 (copie)"]);
    assert_eq!(p.pattern_actif, p.patterns[1].id);
    assert_eq!(p.patterns[1].notes, p.patterns[0].notes);
    // renommer par le menu
    h.get_by_label("Pattern 1 (copie)").click_secondary();
    h.run_steps(2);
    h.get_by_label("Renommer…").click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.get(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput).value("Pattern 1 (copie)")).type_text("Refrain");
    h.step();
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().projet.patterns[1].nom, "Refrain");
    // choisir dans la liste
    h.get_by_label("Pattern 1").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.pattern_actif, h.state().projet.patterns[0].id);
    // supprimer « Refrain » par le menu
    h.get_by_label("Refrain").click_secondary();
    h.run_steps(2);
    h.get_by_label("Supprimer").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.patterns.len(), 1);
    // « + Pattern » du rack : un nouveau pattern
    h.get_by_label("+ Pattern").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.patterns.len(), 2);
    assert_eq!(h.state().projet.pattern_actif().nom, "Pattern 2");
}

#[test]
fn la_regle_joue_le_morceau_depuis_la_mesure_cliquee() {
    let mut h = studio();
    let id = h.state().projet.canaux[0].id.clone();
    let pattern = h.state().projet.pattern_actif.clone();
    h.state_mut().modifier(|p| {
        projet::pas::definir_pas(p, &id, 0, None);
        for m in 0..4 {
            projet::playlist::ajouter_clip(p, &pattern, 0, (m * MESURE) as i64);
        }
    });
    h.run_steps(2);
    assert_eq!(h.state().mode, ModeLecture::Pattern);
    let zone = h.state().arrangement.zone;
    let x = ecran(&h, 0, 2.5 * MESURE as f64).x;
    let regle = Pos2::new(x, zone.top() - 10.0);
    bouger(&mut h, regle);
    bouton(&mut h, regle, PointerButton::Primary, true);
    bouton(&mut h, regle, PointerButton::Primary, false);
    assert_eq!(h.state().mode, ModeLecture::Morceau);
    h.get_by_label("Lecture").click();
    let fin = Instant::now() + Duration::from_secs(3);
    while h.state().etat_transport() != EtatTransport::Lecture {
        assert!(Instant::now() < fin);
        std::thread::sleep(Duration::from_millis(10));
        h.step();
    }
    let position = h.state().position();
    assert!((2.0 * MESURE as f64..2.6 * MESURE as f64).contains(&position), "lecture depuis la mesure 3 : {position}");
    h.get_by_label("Stop").click();
    h.step();
}

#[test]
fn des_milliers_de_clips_restent_fluides() {
    let mut h = studio();
    let pattern = h.state().projet.pattern_actif.clone();
    h.state_mut().modifier(|p| {
        for i in 0..5000u32 {
            projet::playlist::ajouter_clip(p, &pattern, (i % 500) as i64, ((i / 500) * MESURE) as i64);
        }
    });
    h.run_steps(3);
    assert_eq!(projet::playlist::nombre_de_pistes(&h.state().projet), 501);
    let debut = Instant::now();
    for _ in 0..30 {
        h.step();
    }
    let par_image = debut.elapsed() / 30;
    eprintln!("arrangement, 5 000 clips sur 500 pistes : {par_image:?} par image");
    assert!(par_image < Duration::from_millis(30));
}
