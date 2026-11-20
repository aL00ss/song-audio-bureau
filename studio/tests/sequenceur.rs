//! N3 : le séquenceur pas à pas, piloté comme par un utilisateur (clics, glisser pour peindre,
//! clic droit pour effacer, vélocités), avec le vrai moteur.

use std::path::Path;

use eframe::egui::{self, Pos2};
use egui_kittest::Harness;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{Queryable, by};
use projet::modele::{OptionsCanal, creer_projet};
use projet::pas::{definir_nombre_de_pas, definir_note_defaut, velocites_des_pas};
use studio::Studio;

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1600.0, 900.0]).with_step_dt(1.0 / 60.0).build_eframe(|cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    let mut p = creer_projet("Rythme");
    p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), note_defaut: Some(36), ..Default::default() });
    p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Clap".into()), note_defaut: Some(39), ..Default::default() });
    h.state_mut().remplacer_projet(p, None);
    h.step();
    h.step();
    h
}

/// Le bouton du canal dans le rack (son nom apparaît aussi dans le piano roll et la vue
/// d'instrument, comme simple texte).
fn canal<'a>(h: &'a Harness<'static, Studio>, nom: &'a str) -> egui_kittest::Node<'a> {
    h.get(by().role(Role::Button).label(nom))
}

/// Valeurs des listes qui ne sont pas des sons : grille du piano roll, pattern du rack.
fn autres_listes(h: &Harness<'static, Studio>) -> Vec<String> {
    let mut v: Vec<String> = projet::edition::GRILLES.iter().map(|(n, _)| n.to_string()).collect();
    v.extend(h.state().projet.patterns.iter().map(|p| p.nom.clone()));
    v.extend(projet::pas::ParametrePas::TOUS.iter().map(|p| p.nom().to_string()));
    v
}

/// Les listes de sons de batterie du rack.
fn sons(h: &Harness<'static, Studio>) -> Vec<String> {
    let autres = autres_listes(h);
    h.query_all(by().role(Role::ComboBox)).filter_map(|n| n.value()).filter(|v| !autres.contains(v)).collect()
}

fn pas(h: &Harness<'static, Studio>, canal: usize) -> Vec<u8> {
    let s = h.state();
    let c = &s.projet.canaux[canal];
    velocites_des_pas(s.projet.pattern_actif(), &c.id, c.note_defaut)
}

fn centre(h: &Harness<'static, Studio>, etiquette: &str) -> Pos2 {
    h.get_by_label(etiquette).rect().center()
}

fn bouton(h: &mut Harness<'static, Studio>, pos: Pos2, bouton: egui::PointerButton, appuye: bool) {
    h.event(egui::Event::PointerButton { pos, button: bouton, pressed: appuye, modifiers: egui::Modifiers::NONE });
    h.step();
}

fn deplacer(h: &mut Harness<'static, Studio>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
}

/// Amène le pointeur au centre du widget nommé.
fn survoler(h: &mut Harness<'static, Studio>, etiquette: &str) -> Pos2 {
    let c = centre(h, etiquette);
    deplacer(h, c);
    c
}

fn appuyer(h: &mut Harness<'static, Studio>, etiquette: &str, b: egui::PointerButton) {
    let c = survoler(h, etiquette);
    bouton(h, c, b, true);
}

fn relacher(h: &mut Harness<'static, Studio>, etiquette: &str, b: egui::PointerButton) {
    let c = survoler(h, etiquette);
    bouton(h, c, b, false);
}

#[test]
fn un_clic_allume_un_pas_un_autre_l_eteint() {
    let mut h = studio();
    h.get_by_label("Pas 5 Grosse caisse").click();
    h.step();
    assert_eq!(pas(&h, 0)[..6], [0, 0, 0, 0, 100, 0]);
    assert_eq!(pas(&h, 1), vec![0; 16], "l'autre canal ne bouge pas");
    assert!(h.state().modifie);
    h.get_by_label("Pas 5 Grosse caisse").click();
    h.step();
    assert_eq!(pas(&h, 0), vec![0; 16]);
}

#[test]
fn glisser_peint_les_pas_et_le_clic_droit_efface() {
    let mut h = studio();
    use egui::PointerButton::{Primary, Secondary};
    // peinture : appui sur le pas 2, on passe sur 3, 4, 5, puis sur la ligne voisine (ignorée)
    appuyer(&mut h, "Pas 2 Grosse caisse", Primary);
    for i in 3..=5 {
        survoler(&mut h, &format!("Pas {i} Grosse caisse"));
    }
    relacher(&mut h, "Pas 6 Clap", Primary);
    assert_eq!(pas(&h, 0)[..7], [0, 100, 100, 100, 100, 0, 0]);
    assert_eq!(pas(&h, 1), vec![0; 16], "la peinture reste sur sa ligne");
    // après le relâchement, survoler ne peint plus
    survoler(&mut h, "Pas 8 Grosse caisse");
    assert_eq!(pas(&h, 0)[7], 0);
    // partir d'un pas allumé éteint les pas survolés
    appuyer(&mut h, "Pas 3 Grosse caisse", Primary);
    relacher(&mut h, "Pas 4 Grosse caisse", Primary);
    assert_eq!(pas(&h, 0)[..6], [0, 100, 0, 0, 100, 0]);
    // clic droit : efface, même glissé, et n'allume jamais
    appuyer(&mut h, "Pas 1 Grosse caisse", Secondary);
    for i in 2..=4 {
        survoler(&mut h, &format!("Pas {i} Grosse caisse"));
    }
    relacher(&mut h, "Pas 5 Grosse caisse", Secondary);
    assert_eq!(pas(&h, 0), vec![0; 16]);
}

#[test]
fn la_grille_suit_la_longueur_et_le_son_de_batterie() {
    let mut h = studio();
    assert!(h.query_by_label("Pas 16 Clap").is_some());
    assert!(h.query_by_label("Pas 17 Clap").is_none());
    h.state_mut().modifier(|p| definir_nombre_de_pas(p, 32));
    h.step();
    // le pas 32 dépasse du panneau : on fait défiler jusqu'à lui
    h.get_by_label("Pas 32 Clap").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Pas 32 Clap").click();
    h.step();
    assert_eq!(pas(&h, 1)[31], 100);
    // changer de son garde les pas
    let id = h.state().projet.canaux[1].id.clone();
    h.state_mut().modifier(|p| definir_note_defaut(p, &id, 40));
    h.step();
    assert_eq!(h.state().projet.canaux[1].note_defaut, 40);
    assert_eq!(pas(&h, 1)[31], 100);
    assert_eq!(sons(&h), vec!["Grosse caisse", "Caisse claire électrique"], "le son de chaque canal s'affiche");
}

#[test]
fn l_editeur_de_velocite_regle_le_pas_du_canal_choisi() {
    let mut h = studio();
    h.get_by_label("Pas 1 Clap").click();
    h.step();
    canal(&h, "Clap").click();
    h.step();
    assert_eq!(h.state().canal_selectionne.as_deref(), Some(h.state().projet.canaux[1].id.as_str()));
    // appui en haut de la barre : vélocité maximale ; au milieu : environ la moitié
    let r = h.get_by_label("Vélocité du pas 1").rect();
    let x = r.center().x;
    let haut = Pos2::new(x, r.top() + 0.5);
    deplacer(&mut h, haut);
    bouton(&mut h, haut, egui::PointerButton::Primary, true);
    bouton(&mut h, haut, egui::PointerButton::Primary, false);
    assert!(pas(&h, 1)[0] >= 126, "vélocité {}", pas(&h, 1)[0]);
    let milieu = Pos2::new(x, r.center().y);
    deplacer(&mut h, milieu);
    bouton(&mut h, milieu, egui::PointerButton::Primary, true);
    bouton(&mut h, milieu, egui::PointerButton::Primary, false);
    assert!((60..=68).contains(&pas(&h, 1)[0]), "vélocité {}", pas(&h, 1)[0]);
}

#[test]
fn un_canal_aux_notes_libres_montre_un_apercu() {
    let mut h = studio();
    h.state_mut().remplacer_projet(projet::demo::creer_demo(), None);
    h.step();
    h.step();
    let n = h.state().projet.canaux.len();
    assert_eq!(h.query_all_by_label("Aperçu des notes").count(), n, "aucune grille : les notes sont libres");
    assert!(h.query_by_label_contains("Pas 1 ").is_none());
}

#[test]
fn choisir_le_son_de_batterie_dans_la_liste() {
    let mut h = studio();
    h.get_by_label("Pas 1 Grosse caisse").click();
    h.step();
    let autres = autres_listes(&h);
    h.query_all(by().role(Role::ComboBox)).find(|n| n.value().is_some_and(|v| !autres.contains(&v))).expect("liste des sons").click();
    h.run_steps(2);
    h.get_by_label("Cerclage").scroll_to_me();
    h.run_steps(2);
    h.get_by_label("Cerclage").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.canaux[0].note_defaut, 37);
    assert_eq!(pas(&h, 0)[0], 100, "le pas a suivi le nouveau son");
}

fn noms(h: &Harness<'static, Studio>) -> Vec<String> {
    h.state().projet.canaux.iter().map(|c| c.nom.clone()).collect()
}

/// Ouvre le menu d'un canal (clic droit sur son nom) et choisit une entrée.
fn menu(h: &mut Harness<'static, Studio>, nom: &str, entree: &str) {
    canal(h, nom).click_secondary();
    h.run_steps(2);
    h.get_by_label(entree).click();
    h.run_steps(2);
}

#[test]
fn le_menu_d_un_canal_clone_deplace_et_supprime() {
    let mut h = studio();
    h.get_by_label("Pas 1 Grosse caisse").click();
    h.step();
    menu(&mut h, "Grosse caisse", "Cloner");
    assert_eq!(noms(&h), vec!["Grosse caisse", "Grosse caisse 2", "Clap"]);
    assert_eq!(pas(&h, 1)[0], 100, "la copie a les pas de l'original");
    let copie = h.state().projet.canaux[1].id.clone();
    assert_eq!(h.state().canal_selectionne.as_deref(), Some(copie.as_str()), "la copie est choisie");
    menu(&mut h, "Clap", "Monter");
    assert_eq!(noms(&h), vec!["Grosse caisse", "Clap", "Grosse caisse 2"]);
    menu(&mut h, "Grosse caisse", "Descendre");
    assert_eq!(noms(&h), vec!["Clap", "Grosse caisse", "Grosse caisse 2"]);
    // le moteur suit : autant de canaux actifs que dans le projet
    menu(&mut h, "Grosse caisse 2", "Supprimer");
    assert_eq!(noms(&h), vec!["Clap", "Grosse caisse"]);
    assert!(!h.state().projet.pattern_actif().notes.contains_key(&copie));
    assert_eq!(h.state().canal_selectionne.as_deref(), Some(h.state().projet.canaux[1].id.as_str()), "la sélection passe au voisin");
    assert!(h.query_by_label("Pas 1 Grosse caisse 2").is_none());
}

#[test]
fn renommer_par_le_menu_ou_le_double_clic() {
    let mut h = studio();
    menu(&mut h, "Clap", "Renommer…");
    let champ = h.get(by().role(Role::TextInput).value("Clap"));
    champ.focus();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get(by().role(Role::TextInput).value("Clap")).type_text("Claquement");
    h.step();
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(noms(&h), vec!["Grosse caisse", "Claquement"]);
    // Échap : on garde l'ancien nom
    let c = canal(&h, "Claquement").rect().center();
    deplacer(&mut h, c);
    // double clic (une image dure 1/60 s : les deux clics sont à moins de 0,3 s)
    for appuye in [true, false, true, false] {
        bouton(&mut h, c, egui::PointerButton::Primary, appuye);
    }
    assert!(h.state().renommage.is_some(), "le double clic ouvre le champ");
    h.run_steps(2);
    h.get(by().role(Role::TextInput).value("Claquement")).type_text("xyz");
    h.step();
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(noms(&h), vec!["Grosse caisse", "Claquement"]);
    assert!(h.state().renommage.is_none());
}

#[test]
fn remplacer_le_son_et_la_couleur() {
    let mut h = studio();
    h.state_mut().modifier(|p| {
        p.ajouter_canal(OptionsCanal { programme: 0, ..Default::default() });
    });
    h.run_steps(2);
    canal(&h, "Piano à queue").click_secondary();
    h.run_steps(2);
    h.get_by_label_contains("Remplacer le son").hover();
    h.run_steps(2);
    // « Basses » est aussi une section du navigateur : le menu est dessiné en dernier
    h.query_all_by_label_contains("Basses").last().expect("menu Basses").hover();
    h.run_steps(2);
    h.get_by_label("Basse fretless").click();
    h.run_steps(2);
    let c = &h.state().projet.canaux[2];
    assert_eq!((c.programme, c.nom.as_str()), (35, "Basse fretless"));
    canal(&h, "Basse fretless").click_secondary();
    h.run_steps(2);
    h.get_by_label_contains("Couleur").hover();
    h.run_steps(2);
    h.get_by_label("Couleur 10").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.canaux[2].couleur, projet::canaux::PALETTE[9]);
    // une batterie n'a pas « Remplacer le son »
    canal(&h, "Clap").click_secondary();
    h.run_steps(2);
    assert!(h.query_by_label("Cloner").is_some(), "le menu est ouvert");
    assert!(h.query_by_label_contains("Remplacer le son").is_none());
}

#[test]
fn supprimer_et_deplacer_pendant_la_lecture() {
    use std::time::{Duration, Instant};
    let mut h = studio();
    h.state_mut().remplacer_projet(projet::demo::creer_demo(), None);
    h.step();
    h.get_by_label("Lecture").click();
    let fin = Instant::now() + Duration::from_secs(3);
    while h.state().moteur().unwrap().mesures().voix.iter().sum::<u32>() == 0 {
        assert!(Instant::now() < fin, "la démo devrait sonner");
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    let premier = h.state().projet.canaux[0].nom.clone();
    let dernier = h.state().projet.canaux.last().unwrap().nom.clone();
    menu(&mut h, &premier, "Supprimer");
    menu(&mut h, &dernier, "Monter");
    std::thread::sleep(Duration::from_millis(200));
    h.step();
    assert_eq!(h.state().etat_transport(), moteur::EtatTransport::Lecture, "la lecture continue");
    assert!(!noms(&h).contains(&premier));
    h.get_by_label("Stop").click();
    h.step();
}

/// Ouvre le menu d'un canal, passe sur un sous-menu, choisit une entrée.
fn sous_menu(h: &mut Harness<'static, Studio>, nom: &str, sous: &str, entree: &str) {
    canal(h, nom).click_secondary();
    h.run_steps(2);
    h.get_by_label(&format!("{sous} ⏵")).hover();
    h.run_steps(2);
    h.query_all_by_label(entree).last().unwrap_or_else(|| panic!("{entree} introuvable")).click();
    h.run_steps(2);
}

#[test]
fn outils_de_pas_groupes_et_swing_du_canal() {
    let mut h = studio();
    let actifs = |h: &Harness<'static, Studio>, c: usize| pas(h, c).iter().map(|v| u8::from(*v > 0)).collect::<Vec<_>>();
    sous_menu(&mut h, "Grosse caisse", "Pas", "Remplir tous les 4 pas");
    assert_eq!(actifs(&h, 0), vec![1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    sous_menu(&mut h, "Grosse caisse", "Pas", "Décaler vers la droite");
    assert_eq!(actifs(&h, 0)[0..2], [0, 1]);
    sous_menu(&mut h, "Grosse caisse", "Pas", "Effacer");
    assert_eq!(actifs(&h, 0), vec![0; 16]);
    // un groupe tout neuf, puis le filtre du rack
    canal(&h, "Clap").click_secondary();
    h.run_steps(2);
    h.get_by_label("Groupe ⏵").hover();
    h.run_steps(2);
    let champ = h.get(by().role(Role::TextInput));
    champ.focus();
    champ.type_text("Mains");
    h.step();
    h.query_all_by_label("+").last().unwrap().click();
    h.run_steps(2);
    assert_eq!(h.state().projet.canaux[1].groupe.as_deref(), Some("Mains"));
    h.state_mut().filtre_groupe = studio::rack::FiltreGroupe::Groupe("Mains".into());
    h.run_steps(2);
    assert!(h.query_by_label("Pas 1 Clap").is_some());
    assert!(h.query_by_label("Pas 1 Grosse caisse").is_none(), "le filtre cache les autres groupes");
    // le pas d'un canal filtré garde son vrai rang (moteur, témoins)
    h.get_by_label("Pas 3 Clap").click();
    h.step();
    assert_eq!(pas(&h, 1)[2], 100);
    h.state_mut().filtre_groupe = studio::rack::FiltreGroupe::Tous;
    h.run_steps(2);
    // swing du canal
    canal(&h, "Clap").click_secondary();
    h.run_steps(2);
    h.get_by_label("Swing du canal ⏵").hover();
    h.run_steps(2);
    let curseur = h.get_by_label("Swing de Clap");
    curseur.focus();
    h.step();
    h.key_press(egui::Key::ArrowLeft);
    h.run_steps(2);
    assert!(h.state().projet.canaux[1].swing < 1.0, "swing {}", h.state().projet.canaux[1].swing);
}

#[test]
fn editeur_des_pas_panoramique_et_retour_au_centre() {
    let mut h = studio();
    h.get_by_label("Pas 1 Clap").click();
    h.step();
    canal(&h, "Clap").click();
    h.step();
    h.state_mut().parametre_pas = projet::pas::ParametrePas::Panoramique;
    h.run_steps(2);
    let r = h.get_by_label("Panoramique du pas 1").rect();
    let haut = Pos2::new(r.center().x, r.top() + 0.5);
    deplacer(&mut h, haut);
    bouton(&mut h, haut, egui::PointerButton::Primary, true);
    bouton(&mut h, haut, egui::PointerButton::Primary, false);
    let note = |h: &Harness<'static, Studio>| {
        let s = h.state();
        projet::pas::notes_des_pas(s.projet.pattern_actif(), &s.projet.canaux[1].id, s.projet.canaux[1].note_defaut)[0].unwrap()
    };
    assert!(note(&h).pan >= 95, "à droite : {}", note(&h).pan);
    // près du milieu : retour exact au centre
    let milieu = Pos2::new(r.center().x, r.center().y + 1.0);
    deplacer(&mut h, milieu);
    bouton(&mut h, milieu, egui::PointerButton::Primary, true);
    bouton(&mut h, milieu, egui::PointerButton::Primary, false);
    assert_eq!(note(&h).pan, 0);
    assert_eq!(note(&h).velocite, 100, "la vélocité n'a pas bougé");
}
