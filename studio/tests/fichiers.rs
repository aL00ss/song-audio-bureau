//! N8 : annuler et rétablir, exports MIDI et WAV, confirmation avant de perdre des modifications,
//! reprise après plantage, copies de secours, projets récents, fiche du projet, modèles —
//! pilotés comme par un utilisateur, avec le vrai moteur.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use projet::lecture::ModeLecture;
use studio::Studio;

fn banque() -> std::sync::Arc<moteur::SoundFont> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/banques/GeneralUser-GS.sf2");
    moteur::charger_banque(&chemin).expect("banque General MIDI")
}

fn studio_avec(donnees: Option<PathBuf>) -> Harness<'static, Studio> {
    let banque = banque();
    let mut h = Harness::builder().with_size([1600.0, 1000.0]).with_step_dt(1.0 / 60.0).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        let mut s = Studio::new(Some(banque));
        s.fichiers.donnees = donnees;
        s.chercher_reprise();
        s
    });
    h.run_steps(3);
    h
}

fn studio() -> Harness<'static, Studio> {
    studio_avec(None)
}

fn dossier_temporaire(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("song-audio-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn touche(h: &mut Harness<'static, Studio>, cle: Key, modifiers: Modifiers) {
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.event(egui::Event::Key { key: cle, physical_key: Some(cle), pressed: true, repeat: false, modifiers });
    h.event(egui::Event::Key { key: cle, physical_key: Some(cle), pressed: false, repeat: false, modifiers });
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

fn menu(h: &mut Harness<'static, Studio>, menu: &str, chemin: &[&str]) {
    h.get_by_label(menu).click();
    h.run_steps(2);
    for (k, entree) in chemin.iter().enumerate() {
        if k + 1 < chemin.len() {
            // un sous-menu porte une flèche après son nom
            h.get_by_label(&format!("{entree} ⏵")).hover();
            h.run_steps(2);
            continue;
        }
        // un menu se dessine par-dessus tout : c'est le dernier nœud qui porte ce nom
        h.query_all_by_label_contains(entree).last().unwrap_or_else(|| panic!("entrée « {entree} » introuvable")).click();
        h.run_steps(2);
    }
}

fn nb_notes(h: &Harness<'static, Studio>) -> usize {
    h.state().projet.patterns.iter().flat_map(|p| p.notes.values()).map(Vec::len).sum()
}

fn nouveau_vide(h: &mut Harness<'static, Studio>) {
    h.state_mut().executer(studio::fichiers::Action::Modele(projet::modeles::Modele::Vide));
    h.run_steps(3);
}

#[test]
fn annuler_retablir_un_glisser_est_une_seule_etape() {
    let mut h = studio();
    nouveau_vide(&mut h);
    assert_eq!(nb_notes(&h), 0);
    // peindre 4 pas d'un seul geste
    let c = |h: &Harness<'static, Studio>, i: usize| h.get_by_label(&format!("Pas {i} Grosse caisse")).rect().center();
    let depart = c(&h, 1);
    h.event(egui::Event::PointerMoved(depart));
    h.step();
    h.event(egui::Event::PointerButton { pos: depart, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for i in 2..=4 {
        let p = c(&h, i);
        h.event(egui::Event::PointerMoved(p));
        h.step();
    }
    let fin = c(&h, 4);
    h.event(egui::Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(nb_notes(&h), 4);
    // puis un instrument
    h.get_by_label("+ Batterie").click();
    h.run_steps(2);
    let canaux = h.state().projet.canaux.len();
    assert_eq!(h.state().fichiers.historique.etapes(), vec!["Instrument ajouté", "Notes de Grosse caisse"]);
    // Ctrl+Z : l'instrument part ; encore : les 4 pas partent d'un coup
    touche(&mut h, Key::Z, Modifiers::COMMAND);
    assert_eq!(h.state().projet.canaux.len(), canaux - 1);
    touche(&mut h, Key::Z, Modifiers::COMMAND);
    assert_eq!(nb_notes(&h), 0);
    // Ctrl+Y et Ctrl+Maj+Z rétablissent
    touche(&mut h, Key::Y, Modifiers::COMMAND);
    assert_eq!(nb_notes(&h), 4);
    touche(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(h.state().projet.canaux.len(), canaux);
    // le menu Édition nomme l'étape ; l'historique y ramène en un clic
    menu(&mut h, "Édition", &["Annuler « Instrument ajouté »"]);
    assert_eq!(h.state().projet.canaux.len(), canaux - 1);
    touche(&mut h, Key::Y, Modifiers::COMMAND);
    menu(&mut h, "Édition", &["Historique", "Notes de Grosse caisse"]);
    assert_eq!((nb_notes(&h), h.state().projet.canaux.len()), (0, canaux - 1));
    // changer de pattern n'est pas une étape
    h.get_by_label("+ Pattern").click();
    h.run_steps(2);
    let avant = h.state().fichiers.historique.etapes().len();
    let premier = h.state().projet.patterns[0].id.clone();
    h.state_mut().modifier_si(|p| projet::playlist::choisir_pattern(p, &premier));
    h.run_steps(2);
    assert_eq!(h.state().fichiers.historique.etapes().len(), avant);
}

#[test]
fn exporter_en_midi_et_en_wav() {
    let mut h = studio();
    let dossier = dossier_temporaire("exports");
    // MIDI : relu, les mêmes notes
    let mid = dossier.join("demo.mid");
    h.state().exporter_midi_dans(&mid).unwrap();
    let relu = projet::import_midi::depuis_midi(&std::fs::read(&mid).unwrap(), "relu").unwrap();
    assert_eq!(relu.pattern_actif().notes.values().map(Vec::len).sum::<usize>(), nb_notes(&h));
    // WAV : 4 mesures à 100 BPM = 9,6 s, plus 3 s de queue ; du son, pas de saturation
    let wav = dossier.join("demo.wav");
    h.state().exporter_wav_dans(&wav).unwrap();
    let octets = std::fs::read(&wav).unwrap();
    assert_eq!(&octets[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(octets[24..28].try_into().unwrap()), 44_100);
    let trames = (octets.len() - 44) / 4;
    assert!((trames as f64 / 44_100.0 - 12.6).abs() < 0.05, "durée {}", trames as f64 / 44_100.0);
    let echantillons: Vec<i16> = octets[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    let crete = echantillons.iter().map(|e| e.unsigned_abs()).max().unwrap();
    assert!(crete > 3000 && crete < i16::MAX as u16, "crête {crete}");
    // en mode morceau, dans un fil à part : l'interface reste libre
    h.state_mut().changer_mode(ModeLecture::Morceau);
    let wav2 = dossier.join("morceau.wav");
    h.state_mut().exporter_wav_vers(wav2.clone());
    assert!(h.state().export_en_cours());
    assert!(h.state().message.as_deref().unwrap_or("").starts_with("Export WAV en cours (morceau)"));
    let fin = Instant::now() + Duration::from_secs(30);
    while h.state().export_en_cours() {
        assert!(Instant::now() < fin);
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().message.as_deref().unwrap_or("").starts_with("WAV exporté"));
    assert!(std::fs::metadata(&wav2).unwrap().len() > 1_000_000);
    std::fs::remove_dir_all(&dossier).ok();
}

#[test]
fn confirmation_avant_de_perdre_des_modifications() {
    let mut h = studio();
    let dossier = dossier_temporaire("confirmation");
    h.state_mut().modifier(|p| p.tempo = 133.0);
    h.run_steps(2);
    // « + Nouveau projet » : on demande ; Annuler garde tout
    h.get_by_label("+ Nouveau projet").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains("Enregistrer les modifications").is_some());
    h.get_by_label("Annuler").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.tempo, 133.0);
    // Ne pas enregistrer : le nouveau projet remplace
    h.get_by_label("+ Nouveau projet").click();
    h.run_steps(2);
    h.get_by_label("Ne pas enregistrer").click();
    h.run_steps(2);
    assert_eq!((h.state().projet.nom.as_str(), h.state().modifie), ("Nouveau projet", false));
    // Enregistrer (projet déjà enregistré quelque part) : écrit, puis remplace
    let chemin = dossier.join("essai.song-audio.json");
    h.state_mut().enregistrer_dans(&chemin).unwrap();
    h.state_mut().modifier(|p| p.tempo = 77.0);
    h.run_steps(2);
    h.get_by_label("⏵ Démo (4 instruments)").click();
    h.run_steps(2);
    h.get_by_label("Enregistrer").click();
    h.run_steps(2);
    assert_eq!(h.state().projet.nom, "Démo");
    assert_eq!(projet::serialisation::deserialiser(&std::fs::read_to_string(&chemin).unwrap()).unwrap().tempo, 77.0);
    // fermer la fenêtre avec des modifications : on demande aussi
    h.state_mut().modifier(|p| p.tempo = 88.0);
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    h.run_steps(2);
    assert!(h.query_by_label_contains("Enregistrer les modifications").is_some());
    assert!(!h.state().fichiers.fermeture_permise);
    h.get_by_label("Ne pas enregistrer").click();
    h.run_steps(2);
    assert!(h.state().fichiers.fermeture_permise);
    std::fs::remove_dir_all(&dossier).ok();
}

#[test]
fn reprise_apres_plantage_et_copies_de_secours() {
    let donnees = dossier_temporaire("reprise");
    {
        let mut h = studio_avec(Some(donnees.clone()));
        h.state_mut().modifier(|p| {
            p.nom = "Travail en cours".into();
            p.tempo = 141.0;
        });
        h.run_steps(2);
        // l'image a sauvegardé : fichier de reprise et une copie horodatée
        assert!(donnees.join("reprise.song-audio.json").exists());
        let copies: Vec<_> = std::fs::read_dir(donnees.join("copies")).unwrap().flatten().collect();
        assert_eq!(copies.len(), 1);
        assert!(copies[0].file_name().to_string_lossy().ends_with("Travail en cours.song-audio.json"));
        // « plantage » : la fenêtre disparaît sans fermeture normale
    }
    let mut h = studio_avec(Some(donnees.clone()));
    assert!(h.query_by_label_contains("Du travail non enregistré a été retrouvé").is_some());
    h.get_by_label("Reprendre").click();
    h.run_steps(2);
    assert_eq!((h.state().projet.nom.as_str(), h.state().projet.tempo, h.state().modifie), ("Travail en cours", 141.0, true));
    // enregistrer : la reprise ne sert plus
    let chemin = donnees.join("sauve.song-audio.json");
    h.state_mut().enregistrer_dans(&chemin).unwrap();
    h.run_steps(2);
    assert!(!donnees.join("reprise.song-audio.json").exists());
    // au démarrage suivant, rien à reprendre
    let h2 = studio_avec(Some(donnees.clone()));
    assert!(h2.query_by_label_contains("Du travail non enregistré").is_none());
    std::fs::remove_dir_all(&donnees).ok();
}

#[test]
fn recents_fiche_du_projet_et_modeles() {
    let mut h = studio();
    let dossier = dossier_temporaire("recents");
    let chemin = dossier.join("Ma chanson.song-audio.json");
    h.state_mut().enregistrer_dans(&chemin).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().fichiers.recents, vec![chemin.clone()]);
    // modèle « Hip-hop » par le menu Fichier > Nouveau (projet enregistré : pas de question)
    menu(&mut h, "Fichier", &["Nouveau", "Hip-hop"]);
    assert_eq!((h.state().projet.nom.as_str(), h.state().projet.tempo), ("Hip-hop", 90.0));
    // projets récents : on rouvre
    menu(&mut h, "Fichier", &["Projets récents", "Ma chanson.song-audio.json"]);
    assert_eq!(h.state().projet.nom, "Démo");
    // fiche du projet : auteur et notes
    menu(&mut h, "Fichier", &["Infos du projet"]);
    assert!(h.query_by_label("Infos du projet").is_some());
    let champs: Vec<_> = h.query_all(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput)).collect();
    champs[1].focus();
    h.step();
    h.get_all(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput)).nth(1).unwrap().type_text("Alex");
    h.run_steps(2);
    assert_eq!(h.state().projet.infos.auteur, "Alex");
    assert!(h.state().modifie);
    std::fs::remove_dir_all(&dossier).ok();
}
