//! N4 : piano roll, vue d'instrument et jeu en direct, pilotés comme par un utilisateur
//! (pointeur, clavier de l'ordinateur), avec le vrai moteur.

use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use moteur::EtatTransport;
use projet::NoteMusicale;
use projet::modele::{OptionsCanal, creer_projet};
use studio::Studio;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1600.0, 1000.0]).with_step_dt(1.0 / 60.0).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    let mut p = creer_projet("Essai");
    p.ajouter_canal(OptionsCanal { programme: 0, nom: Some("Piano".into()), ..Default::default() });
    p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Caisse claire".into()), note_defaut: Some(38), ..Default::default() });
    let id = p.pattern_actif.clone();
    p.ajouter_clip(&id, 0, 0);
    h.state_mut().remplacer_projet(p, None);
    h.run_steps(3);
    h
}

fn notes(h: &Harness<'static, Studio>) -> Vec<NoteMusicale> {
    let s = h.state();
    s.projet.pattern_actif().notes.get(&s.projet.canaux[0].id).cloned().unwrap_or_default()
}

fn ecran(h: &Harness<'static, Studio>, tic: f64, hauteur: u8) -> Pos2 {
    h.state().piano_roll.vers_ecran(tic, hauteur)
}

fn bouger(h: &mut Harness<'static, Studio>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
}

fn bouton(h: &mut Harness<'static, Studio>, pos: Pos2, b: PointerButton, appuye: bool, modifiers: Modifiers) {
    h.event(egui::Event::PointerButton { pos, button: b, pressed: appuye, modifiers });
    h.step();
}

/// Appui en `a`, glisser jusqu'à `b` (en quelques pas), relâcher.
fn glisser(h: &mut Harness<'static, Studio>, a: Pos2, b: Pos2, bouton_: PointerButton, modifiers: Modifiers) {
    h.event(egui::Event::ModifiersChanged(modifiers));
    bouger(h, a);
    bouton(h, a, bouton_, true, modifiers);
    for k in 1..=4 {
        bouger(h, a + (b - a) * (k as f32 / 4.0));
    }
    bouton(h, b, bouton_, false, modifiers);
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.step();
}

fn cliquer(h: &mut Harness<'static, Studio>, pos: Pos2, b: PointerButton) {
    glisser(h, pos, pos, b, Modifiers::NONE);
}

fn touche(h: &mut Harness<'static, Studio>, logique: Key, physique: Key, appuye: bool, modifiers: Modifiers) {
    // egui lit les modificateurs dans l'état du clavier, comme le fait la fenêtre réelle
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.event(egui::Event::Key { key: logique, physical_key: Some(physique), pressed: appuye, repeat: false, modifiers });
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.step();
}

/// Attend que le moteur fasse sonner au moins une voix sur le canal de ce rang (au plus 3 s).
fn attendre_voix(h: &mut Harness<'static, Studio>, rang: usize, voulu: bool) {
    let fin = Instant::now() + Duration::from_secs(3);
    loop {
        let voix = h.state().moteur().unwrap().mesures().voix.get(rang).copied().unwrap_or(0);
        if (voix > 0) == voulu {
            return;
        }
        assert!(Instant::now() < fin, "voix du canal {rang} : {voix}, attendu {}", if voulu { "au moins une" } else { "aucune" });
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
}

#[test]
fn poser_deplacer_redimensionner_effacer() {
    let mut h = studio();
    assert!(h.query_by_label("Piano roll de Piano").is_some());
    // clic dans le vide : une note d'un demi-temps (48 tics), aimantée au pas, choisie
    let p = ecran(&h, 30.0, 64);
    cliquer(&mut h, p, PointerButton::Primary);
    assert_eq!(notes(&h), vec![NoteMusicale { tic: 24, duree: 48, hauteur: 64, velocite: 100 }]);
    assert_eq!(h.state().piano_roll.selection.len(), 1);
    assert!(h.state().modifie);
    // glisser la note : déplacée de deux pas et d'un ton, aimantée
    let (a, b) = (ecran(&h, 40.0, 64), ecran(&h, 90.0, 66));
    glisser(&mut h, a, b, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(notes(&h)[0], NoteMusicale { tic: 72, duree: 48, hauteur: 66, velocite: 100 });
    // glisser son bord droit : la note s'allonge d'un temps
    let bord = ecran(&h, 72.0 + 48.0, 66) - egui::vec2(2.0, 0.0);
    let plus_loin = bord + egui::vec2(96.0 * h.state().piano_roll.zoom, 0.0);
    glisser(&mut h, bord, plus_loin, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(notes(&h)[0].duree, 144);
    // la note suivante prend la durée de la dernière touchée
    let p = ecran(&h, 400.0, 60);
    cliquer(&mut h, p, PointerButton::Primary);
    assert_eq!(notes(&h)[1], NoteMusicale { tic: 384, duree: 144, hauteur: 60, velocite: 100 });
    // clic droit glissé : efface les notes survolées
    let p = ecran(&h, 400.0, 60);
    cliquer(&mut h, p, PointerButton::Secondary);
    assert_eq!(notes(&h).len(), 1);
    let (a, b) = (ecran(&h, 60.0, 66), ecran(&h, 300.0, 66));
    glisser(&mut h, a, b, PointerButton::Secondary, Modifiers::NONE);
    assert!(notes(&h).is_empty());
}

#[test]
fn lasso_clavier_velocite_et_quantification() {
    let mut h = studio();
    let id = h.state().projet.canaux[0].id.clone();
    let depart = vec![
        NoteMusicale { tic: 5, duree: 40, hauteur: 60, velocite: 100 },
        NoteMusicale { tic: 100, duree: 40, hauteur: 62, velocite: 100 },
        NoteMusicale { tic: 300, duree: 40, hauteur: 70, velocite: 100 },
    ];
    h.state_mut().remplacer_notes(&id, depart.clone());
    h.run_steps(2);
    // Ctrl + glisser : lasso sur les deux premières
    let (a, b) = (ecran(&h, 0.0, 63), ecran(&h, 150.0, 59));
    glisser(&mut h, a, b, PointerButton::Primary, Modifiers::COMMAND);
    assert_eq!(h.state().piano_roll.selection.iter().copied().collect::<Vec<_>>(), vec![0, 1]);
    // flèche haut : un demi-ton ; Maj + haut : une octave ; droite : un pas de grille (les deux
    // notes choisies bougent)
    touche(&mut h, Key::ArrowUp, Key::ArrowUp, true, Modifiers::NONE);
    touche(&mut h, Key::ArrowUp, Key::ArrowUp, true, Modifiers::SHIFT);
    touche(&mut h, Key::ArrowRight, Key::ArrowRight, true, Modifiers::NONE);
    let n = notes(&h);
    assert_eq!((n[0].tic, n[0].hauteur, n[1].hauteur, n[2].hauteur), (29, 73, 75, 70));
    assert_eq!(h.state().jeu.base, projet::clavier::BASE_DEFAUT, "le piano roll a le focus : les flèches ne changent pas l'octave");
    // couloir de vélocité : la première note à 64 environ
    let v = h.state().piano_roll.vers_ecran_velocite(29.0, 64);
    cliquer(&mut h, v, PointerButton::Primary);
    assert!((60..=68).contains(&notes(&h)[0].velocite), "vélocité {}", notes(&h)[0].velocite);
    // Ctrl+A puis Quantifier : tout sur la grille du pas
    let centre = ecran(&h, 600.0, 50);
    cliquer(&mut h, centre, PointerButton::Secondary); // redonne le focus, sans rien effacer
    touche(&mut h, Key::A, Key::Q, true, Modifiers::COMMAND);
    assert_eq!(h.state().piano_roll.selection.len(), 3);
    h.get_by_label("Quantifier").click();
    h.run_steps(2);
    assert_eq!(notes(&h).iter().map(|n| n.tic).collect::<Vec<_>>(), vec![24, 120, 312]);
    // Échap vide la sélection ; Suppr sur une sélection vide ne fait rien
    let centre = ecran(&h, 600.0, 50);
    cliquer(&mut h, centre, PointerButton::Secondary);
    touche(&mut h, Key::A, Key::Q, true, Modifiers::COMMAND);
    touche(&mut h, Key::Escape, Key::Escape, true, Modifiers::NONE);
    touche(&mut h, Key::Delete, Key::Delete, true, Modifiers::NONE);
    assert_eq!(notes(&h).len(), 3);
    // sélection puis Suppr
    let p = ecran(&h, 320.0, 70);
    cliquer(&mut h, p, PointerButton::Primary);
    touche(&mut h, Key::Delete, Key::Delete, true, Modifiers::NONE);
    assert_eq!(notes(&h).len(), 2);
}

#[test]
fn le_clavier_du_piano_roll_et_de_la_vue_d_instrument_font_sonner_le_moteur() {
    let mut h = studio();
    // clavier du piano roll : la note sonne tant qu'on tient
    let x = h.state().piano_roll.zone.left() + 20.0;
    let y = ecran(&h, 0.0, 64).y;
    bouger(&mut h, Pos2::new(x, y));
    bouton(&mut h, Pos2::new(x, y), PointerButton::Primary, true, Modifiers::NONE);
    let id = h.state().projet.canaux[0].id.clone();
    assert!(h.state().notes_en_direct(&id).contains(&64));
    attendre_voix(&mut h, 0, true);
    bouton(&mut h, Pos2::new(x, y), PointerButton::Primary, false, Modifiers::NONE);
    assert!(h.state().notes_en_direct(&id).is_empty());
    attendre_voix(&mut h, 0, false);
    // vue d'instrument : appui sur Do3 (60), glissando jusqu'à Mi3 (64), relâcher
    let do3 = h.state().instrument.vers_ecran(60, 0.9).unwrap();
    let mi3 = h.state().instrument.vers_ecran(64, 0.9).unwrap();
    bouger(&mut h, do3);
    bouton(&mut h, do3, PointerButton::Primary, true, Modifiers::NONE);
    assert_eq!(h.state().notes_en_direct(&id).into_iter().collect::<Vec<_>>(), vec![60]);
    bouger(&mut h, mi3);
    assert_eq!(h.state().notes_en_direct(&id).into_iter().collect::<Vec<_>>(), vec![64], "glissando");
    bouton(&mut h, mi3, PointerButton::Primary, false, Modifiers::NONE);
    assert!(h.state().notes_en_direct(&id).is_empty());
    // la touche jouée s'allume
    assert!(h.query_by_label("Do3").is_some());
}

#[test]
fn clavier_de_l_ordinateur_octaves_et_disposition() {
    let mut h = studio();
    let id = h.state().projet.canaux[0].id.clone();
    // un clavier AZERTY : la touche physique Q écrit « A » ; elle joue le do central
    touche(&mut h, Key::A, Key::Q, true, Modifiers::NONE);
    assert_eq!(h.state().jeu.disposition, projet::clavier::DispositionClavier::Azerty);
    assert_eq!(h.state().notes_en_direct(&id).into_iter().collect::<Vec<_>>(), vec![60]);
    attendre_voix(&mut h, 0, true);
    touche(&mut h, Key::A, Key::Q, false, Modifiers::NONE);
    assert!(h.state().notes_en_direct(&id).is_empty());
    // flèche droite (piano roll sans focus) : une octave plus haut
    touche(&mut h, Key::ArrowRight, Key::ArrowRight, true, Modifiers::NONE);
    touche(&mut h, Key::A, Key::Q, true, Modifiers::NONE);
    assert_eq!(h.state().notes_en_direct(&id).into_iter().collect::<Vec<_>>(), vec![72]);
    // la fenêtre perd le focus touche enfoncée : rien ne reste bloqué
    h.event(egui::Event::WindowFocused(false));
    h.step();
    assert!(h.state().notes_en_direct(&id).is_empty());
    // pendant une saisie de texte (renommer un canal), les lettres écrivent au lieu de jouer
    h.state_mut().renommage = Some((id.clone(), "Piano".into()));
    h.run_steps(2);
    let champ = h.get(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput));
    champ.focus();
    h.step();
    touche(&mut h, Key::A, Key::Q, true, Modifiers::NONE);
    assert!(h.state().notes_en_direct(&id).is_empty());
}

#[test]
fn enregistrer_ce_qu_on_joue_pendant_la_lecture() {
    let mut h = studio();
    h.get_by_label("Enr.").click();
    h.step();
    assert!(h.state().jeu.enregistrement);
    // à l'arrêt : jouer n'enregistre rien
    touche(&mut h, Key::A, Key::Q, true, Modifiers::NONE);
    touche(&mut h, Key::A, Key::Q, false, Modifiers::NONE);
    assert!(notes(&h).is_empty());
    h.get_by_label("Lecture").click();
    let fin = Instant::now() + Duration::from_secs(3);
    while h.state().etat_transport() != EtatTransport::Lecture {
        assert!(Instant::now() < fin);
        std::thread::sleep(Duration::from_millis(10));
        h.step();
    }
    std::thread::sleep(Duration::from_millis(100));
    h.step();
    touche(&mut h, Key::E, Key::E, true, Modifiers::NONE); // mi central
    std::thread::sleep(Duration::from_millis(250));
    touche(&mut h, Key::E, Key::E, false, Modifiers::NONE);
    let n = notes(&h);
    assert_eq!(n.len(), 1, "la note jouée est inscrite");
    // 120 BPM : 250 ms = un demi-temps, 48 tics (à l'image près)
    assert_eq!(n[0].hauteur, 64);
    assert!((30..=80).contains(&n[0].duree), "durée {}", n[0].duree);
    assert!(n[0].tic < h.state().projet.pattern_actif().longueur);
    h.get_by_label("Stop").click();
    h.step();
}

#[test]
fn pads_de_batterie_et_ouvrir_dans_le_piano_roll() {
    let mut h = studio();
    let batterie = h.state().projet.canaux[1].id.clone();
    let piano = h.state().projet.canaux[0].id.clone();
    // choisir la batterie : la vue d'instrument montre les pads
    h.state_mut().canal_selectionne = Some(batterie.clone());
    h.run_steps(2);
    // aucun pad ne déborde du panneau (le piano roll est à sa droite)
    let limite = h.state().piano_roll.zone.left();
    for (_, sons) in studio::instrument::KIT {
        for &son in sons {
            let r = h.get_by_label(&format!("Pad {}", projet::gm::nom_son_batterie(son))).rect();
            assert!(r.right() <= limite, "pad {son} hors du panneau : {r:?}");
        }
    }
    // le pad du son de cette ligne joue sur elle
    h.get_by_label("Pad Caisse claire").scroll_to_me(); // le panneau est petit : on fait défiler jusqu'au pad
    h.run_steps(3);
    let pad = h.get_by_label("Pad Caisse claire").rect().center();
    bouger(&mut h, pad);
    bouton(&mut h, pad, PointerButton::Primary, true, Modifiers::NONE);
    assert_eq!(h.state().notes_en_direct(&batterie).into_iter().collect::<Vec<_>>(), vec![38]);
    attendre_voix(&mut h, 1, true);
    bouton(&mut h, pad, PointerButton::Primary, false, Modifiers::NONE);
    assert!(h.state().notes_en_direct(&batterie).is_empty());
    // un son sans ligne joue sur le canal choisi
    h.get_by_label("Pad Crash 1").scroll_to_me(); // le panneau est petit : on fait défiler jusqu'au pad
    h.run_steps(3);
    let pad = h.get_by_label("Pad Crash 1").rect().center();
    bouger(&mut h, pad);
    bouton(&mut h, pad, PointerButton::Primary, true, Modifiers::NONE);
    assert_eq!(h.state().notes_en_direct(&batterie).into_iter().collect::<Vec<_>>(), vec![49]);
    bouton(&mut h, pad, PointerButton::Primary, false, Modifiers::NONE);
    // menu du rack : ouvrir le piano dans le piano roll
    h.get(egui_kittest::kittest::by().role(egui::accesskit::Role::Button).label("Piano")).click_secondary();
    h.run_steps(2);
    h.get_by_label("Ouvrir dans le piano roll").click();
    h.run_steps(2);
    assert_eq!(h.state().canal_selectionne.as_deref(), Some(piano.as_str()));
    assert!(h.query_by_label("Piano roll de Piano").is_some());
}

#[test]
fn vingt_mille_notes_restent_fluides() {
    let mut h = studio();
    let id = h.state().projet.canaux[0].id.clone();
    // 20 000 notes sur 500 mesures : seules les visibles sont dessinées
    let beaucoup: Vec<NoteMusicale> = (0..20_000u32).map(|i| NoteMusicale { tic: i * 9, duree: 20, hauteur: 40 + (i % 48) as u8, velocite: 90 }).collect();
    h.state_mut().modifier(|p| {
        p.pattern_actif_mut().longueur = 20_000 * 9;
        p.pattern_actif_mut().notes.insert(id.clone(), beaucoup);
    });
    h.run_steps(3);
    let debut = Instant::now();
    for _ in 0..30 {
        h.step();
    }
    let par_image = debut.elapsed() / 30;
    eprintln!("piano roll, 20 000 notes : {par_image:?} par image");
    assert!(par_image < Duration::from_millis(40), "{par_image:?} par image");
    // poser une note reste instantané
    let p = ecran(&h, 30.0, 64);
    cliquer(&mut h, p, PointerButton::Primary);
    assert_eq!(notes(&h).len(), 20_001);
}
