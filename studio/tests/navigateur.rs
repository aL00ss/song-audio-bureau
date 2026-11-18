//! N2 : le navigateur (recherche en ligne avec un faux réseau, morceaux fournis, fichiers du
//! disque), piloté comme par un utilisateur, avec le vrai moteur.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};
use moteur::EtatTransport;
use projet::recherche::ResultatRecherche;
use studio::Studio;
use studio::navigateur::{EtatRecherche, Reseau};

/// Faux réseau : compte les recherches, répond « chopin » par deux résultats et télécharge la
/// Lettre à Élise fournie.
#[derive(Default)]
struct FauxReseau {
    recherches: AtomicUsize,
    requetes: Mutex<Vec<String>>,
    en_panne: bool,
}

impl Reseau for FauxReseau {
    fn chercher(&self, requete: &str) -> Result<Vec<ResultatRecherche>, String> {
        self.recherches.fetch_add(1, Ordering::SeqCst);
        self.requetes.lock().unwrap().push(requete.to_string());
        if self.en_panne {
            return Err("hors ligne".into());
        }
        let r = |id: &str, titre: &str| ResultatRecherche {
            id: id.into(),
            titre: titre.into(),
            nom_fichier: format!("{titre}.mid"),
            url: format!("https://exemple/{id}.mid"),
            ecoutes: 1234,
        };
        Ok(if requete.contains("chopin") { vec![r("1", "chopin nocturne"), r("2", "chopin valse")] } else { vec![] })
    }

    fn telecharger(&self, _url: &str) -> Result<Vec<u8>, String> {
        let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/morceaux/lettre-a-elise.mid");
        std::fs::read(chemin).map_err(|e| e.to_string())
    }
}

fn studio_avec(reseau: Arc<FauxReseau>) -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1400.0, 900.0]).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::avec_reseau(Some(banque), reseau)
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
fn recherche_une_seule_fois_apres_la_frappe_puis_charge_et_joue() {
    let reseau = Arc::new(FauxReseau::default());
    let mut h = studio_avec(reseau.clone());
    let champ = h.get(by().role(Role::TextInput));
    champ.focus();
    champ.type_text("chop");
    h.step();
    h.get(by().role(Role::TextInput)).type_text("in");
    h.step();
    attendre(&mut h, "des résultats", |s| matches!(s.navigateur.recherche, EtatRecherche::Resultats(_)));
    assert_eq!(reseau.recherches.load(Ordering::SeqCst), 1, "une seule recherche malgré deux frappes");
    assert_eq!(reseau.requetes.lock().unwrap()[0], "chopin");
    h.step();
    assert!(h.query_by_label_contains("chopin nocturne").is_some());
    assert_eq!(h.query_all_by_label_contains("1\u{202f}234 écoutes").count(), 2, "les écoutes de chaque résultat");
    h.get_by_label_contains("chopin nocturne").click();
    h.step();
    attendre(&mut h, "le morceau chargé", |s| s.projet.nom == "chopin nocturne");
    assert_eq!(h.state().projet.canaux.len(), 2);
    attendre(&mut h, "la lecture", |s| s.etat_transport() == EtatTransport::Lecture);
    arreter(&mut h);
}

#[test]
fn moins_de_deux_lettres_aucune_recherche_et_reponse_vide() {
    let reseau = Arc::new(FauxReseau::default());
    let mut h = studio_avec(reseau.clone());
    let champ = h.get(by().role(Role::TextInput));
    champ.focus();
    champ.type_text("c");
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    assert_eq!(reseau.recherches.load(Ordering::SeqCst), 0);
    assert_eq!(h.state().navigateur.recherche, EtatRecherche::Repos);
    h.get(by().role(Role::TextInput)).type_text("zzz");
    attendre(&mut h, "une réponse vide", |s| s.navigateur.recherche == EtatRecherche::Resultats(vec![]));
    h.step();
    assert!(h.query_by_label("Aucun fichier trouvé pour « czzz ».").is_some());
}

#[test]
fn recherche_en_panne_message_clair() {
    let reseau = Arc::new(FauxReseau { en_panne: true, ..Default::default() });
    let mut h = studio_avec(reseau);
    let champ = h.get(by().role(Role::TextInput));
    champ.focus();
    champ.type_text("chopin");
    attendre(&mut h, "l'erreur", |s| s.navigateur.recherche == EtatRecherche::Erreur);
    h.step();
    assert!(h.query_by_label("Recherche indisponible : vérifiez la connexion.").is_some());
}

#[test]
fn morceau_fourni_charge_et_joue() {
    let mut h = studio_avec(Arc::new(FauxReseau::default()));
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
    let mut h = studio_avec(Arc::new(FauxReseau::default()));
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
