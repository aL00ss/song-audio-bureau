//! E3 : glisser un son du navigateur vers le rack, et les couches (un canal qui en joue
//! plusieurs), pilotés comme par un utilisateur, avec le vrai moteur.

use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{self, Modifiers, PointerButton, Pos2, accesskit::Role};
use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};
use projet::SourceCanal;
use projet::modele::{OptionsCanal, creer_projet};
use studio::Studio;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1600.0, 1400.0]).with_step_dt(1.0 / 60.0).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    let mut p = creer_projet("Couches");
    p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
    p.ajouter_canal(OptionsCanal { programme: 0, nom: Some("Piano".into()), ..Default::default() });
    h.state_mut().remplacer_projet(p, None);
    h.run_steps(3);
    h
}

fn glisser(h: &mut Harness<'static, Studio>, depart: Pos2, arrivee: Pos2) {
    h.event(egui::Event::PointerMoved(depart));
    h.step();
    h.event(egui::Event::PointerButton { pos: depart, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=6 {
        h.event(egui::Event::PointerMoved(depart + (arrivee - depart) * (k as f32 / 6.0)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: arrivee, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

fn canal<'a>(h: &'a Harness<'static, Studio>, nom: &'a str) -> egui_kittest::Node<'a> {
    h.get(by().role(Role::Button).label(nom))
}

#[test]
fn glisser_un_son_sur_le_rack_puis_sur_un_canal() {
    let mut h = studio();
    // ouvrir la famille « Basses » du navigateur
    h.get_by_label("Basses").click();
    h.run_steps(2);
    let depart = h.get_by_label("+ Basse fretless").rect().center();
    // lâché dans le vide du rack, sous les canaux : un nouveau canal
    let rack = h.get_by_label("Pas 1 Piano").rect();
    let arrivee = Pos2::new(rack.center().x, rack.bottom() + 120.0);
    glisser(&mut h, depart, arrivee);
    let p = &h.state().projet;
    assert_eq!(p.canaux.len(), 3);
    assert_eq!((p.canaux[2].programme, p.canaux[2].nom.as_str()), (35, "Basse fretless"));
    // lâché sur le nom « Piano » : son remplacé (et le nom automatique suit)
    // « Synthés intégrés » est ouvert d'office
    let depart = h.get_by_label("+ Lead acide").rect().center();
    let arrivee = canal(&h, "Piano").rect().center();
    glisser(&mut h, depart, arrivee);
    let c = &h.state().projet.canaux[1];
    assert_eq!((c.source, c.programme), (SourceCanal::Soustractif, 1));
    assert_eq!(h.state().projet.canaux.len(), 3, "remplacé, pas ajouté");
    // sur une batterie : rien ne change
    let depart = h.get_by_label("+ Lead acide").rect().center();
    let arrivee = canal(&h, "Grosse caisse").rect().center();
    glisser(&mut h, depart, arrivee);
    assert!(h.state().projet.canaux[0].batterie);
    assert_eq!(h.state().projet.canaux.len(), 3);
}

#[test]
fn une_couche_joue_ses_instruments() {
    let mut h = studio();
    h.get_by_label("+ Couche").click();
    h.run_steps(2);
    let couche = h.state().projet.canaux.last().unwrap().clone();
    assert_eq!(couche.source, SourceCanal::Couche);
    // menu de la couche : cocher la grosse caisse et le piano
    // (cocher referme le menu : on le rouvre pour chaque instrument)
    for nom in ["Grosse caisse", "Piano"] {
        canal(&h, "Couche").click_secondary();
        h.run_steps(2);
        h.get_by_label("Couche ⏵").hover();
        h.run_steps(2);
        h.get(by().role(Role::CheckBox).label(nom)).click();
        h.run_steps(2);
    }
    let enfants = h.state().projet.canal(&couche.id).unwrap().enfants.clone();
    assert_eq!(enfants, vec![h.state().projet.canaux[0].id.clone(), h.state().projet.canaux[1].id.clone()]);
    // jouer la couche en direct : les deux canaux sonnent dans le moteur
    h.key_press(egui::Key::Escape);
    h.state_mut().jouer_note(60, 100, &couche.id);
    let fin = Instant::now() + Duration::from_secs(4);
    loop {
        let voix = h.state().moteur().unwrap().mesures().voix.clone();
        if voix.first().copied().unwrap_or(0) > 0 && voix.get(1).copied().unwrap_or(0) > 0 {
            assert_eq!(voix.get(2).copied().unwrap_or(0), 0, "la couche elle-même ne sonne pas");
            break;
        }
        assert!(Instant::now() < fin, "les deux enfants devraient sonner : {voix:?}");
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    h.state_mut().relacher_note(60, &couche.id);
    h.step();
    assert!(h.state().notes_en_direct(&couche.id).is_empty());
}
