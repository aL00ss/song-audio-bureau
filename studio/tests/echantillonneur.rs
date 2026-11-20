//! D2 : l'échantillonneur dans l'application — un instrument SFZ chargé en arrière-plan, joué
//! par le vrai moteur, rendu dans l'export WAV, gardé dans le projet.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use projet::SourceCanal;
use projet::modele::creer_projet;
use studio::Studio;

/// Un WAV mono 16 bits, 44,1 kHz : une sinusoïde.
fn ecrire_wav(chemin: &Path, frequence: f32, secondes: f32) {
    let n = (secondes * 44100.0) as usize;
    let mut o = Vec::with_capacity(44 + n * 2);
    o.extend_from_slice(b"RIFF");
    o.extend_from_slice(&(36 + n as u32 * 2).to_le_bytes());
    o.extend_from_slice(b"WAVEfmt ");
    o.extend_from_slice(&16u32.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&44100u32.to_le_bytes());
    o.extend_from_slice(&(44100u32 * 2).to_le_bytes());
    o.extend_from_slice(&2u16.to_le_bytes());
    o.extend_from_slice(&16u16.to_le_bytes());
    o.extend_from_slice(b"data");
    o.extend_from_slice(&(n as u32 * 2).to_le_bytes());
    for i in 0..n {
        let v = (i as f32 / 44100.0 * frequence * std::f32::consts::TAU).sin() * 0.6;
        o.extend_from_slice(&((v * i16::MAX as f32) as i16).to_le_bytes());
    }
    std::fs::write(chemin, o).unwrap();
}

/// Un instrument : un la à 440 Hz, joué sur tout le clavier.
fn instrument(nom: &str) -> (PathBuf, PathBuf) {
    let dossier = std::env::temp_dir().join(format!("song-audio-sfz-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(dossier.join("sons")).unwrap();
    ecrire_wav(&dossier.join("sons/la 440.wav"), 440.0, 2.0);
    let sfz = dossier.join("Sinus.sfz");
    std::fs::write(&sfz, "<control> default_path=sons/\n<region> sample=la 440.wav pitch_keycenter=69 ampeg_release=0.05").unwrap();
    (dossier, sfz)
}

fn studio() -> Harness<'static, Studio> {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).expect("banque General MIDI");
    let mut h = Harness::builder().with_size([1400.0, 900.0]).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        Studio::new(Some(banque))
    });
    h.state_mut().remplacer_projet(creer_projet("Échantillons"), None);
    h.run_steps(2);
    h
}

fn attendre(h: &mut Harness<'static, Studio>, quoi: &str, condition: impl Fn(&Studio) -> bool) {
    let fin = Instant::now() + Duration::from_secs(5);
    while !condition(h.state()) {
        assert!(Instant::now() < fin, "attendu : {quoi}");
        std::thread::sleep(Duration::from_millis(15));
        h.step();
    }
}

#[test]
fn un_instrument_sfz_charge_joue_et_s_exporte() {
    let (dossier, sfz) = instrument("joue");
    let mut h = studio();
    h.state_mut().ajouter_echantillonneur(&sfz);
    h.step();
    let canal = h.state().projet.canaux[0].clone();
    assert_eq!((canal.source, canal.nom.as_str()), (SourceCanal::Echantillons, "Sinus"));
    let chemin = sfz.to_string_lossy().to_string();
    attendre(&mut h, "l'instrument chargé", |s| s.instruments.charge(&chemin).is_some());
    h.step();
    assert!(h.state().message.as_deref().unwrap_or("").contains("Instrument « Sinus » chargé (1 zones)"));
    // jouer : le moteur fait sonner une voix sur ce canal
    h.state_mut().jouer_note(69, 100, &canal.id);
    attendre(&mut h, "une voix", |s| s.moteur().unwrap().mesures().voix.first().copied().unwrap_or(0) > 0);
    h.state_mut().relacher_note(69, &canal.id);
    attendre(&mut h, "le silence", |s| s.moteur().unwrap().mesures().voix.first().copied().unwrap_or(1) == 0);
    // export WAV d'une note : le la à 440 Hz (et 880 Hz une octave plus haut)
    h.state_mut().modifier(|p| {
        let id = p.canaux[0].id.clone();
        p.pattern_actif_mut().notes.insert(id, vec![projet::NoteMusicale { tic: 0, duree: 192, hauteur: 81, velocite: 110, ..Default::default() }]);
    });
    h.step();
    let wav = dossier.join("rendu.wav");
    h.state().exporter_wav_dans(&wav).unwrap();
    let octets = std::fs::read(&wav).unwrap();
    let gauche: Vec<f32> = octets[44..].chunks(4).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
    let debut = &gauche[2000..40000];
    let passages = debut.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f32 / (debut.len() as f32 / 44100.0);
    assert!((passages - 880.0).abs() < 20.0, "{passages} Hz");
    std::fs::remove_dir_all(&dossier).ok();
}

#[test]
fn le_projet_garde_l_instrument_et_la_liste_des_fichiers_l_ajoute() {
    let (dossier, sfz) = instrument("projet");
    let mut h = studio();
    // un .sfz cliqué dans la liste des fichiers : un canal échantillonneur
    h.state_mut().navigateur.changer_dossier(dossier.clone());
    h.run_steps(2);
    h.get_by_label("FICHIERS").hover();
    let entree = h.state_mut().navigateur.contenu(&dossier).into_iter().find(|e| e.nom == "Sinus.sfz").expect("le .sfz est listé");
    assert!(!entree.dossier);
    h.state_mut().ajouter_echantillonneur(&entree.chemin);
    h.step();
    // enregistrer, rouvrir : l'instrument est toujours là, et se recharge
    let fichier = dossier.join("projet.song-audio.json");
    h.state_mut().enregistrer_dans(&fichier).unwrap();
    h.state_mut().remplacer_projet(creer_projet("Autre"), None);
    h.step();
    h.state_mut().ouvrir(&fichier);
    h.step();
    let canal = h.state().projet.canaux[0].clone();
    assert_eq!(canal.instrument.as_deref(), Some(sfz.to_string_lossy().as_ref()));
    let chemin = sfz.to_string_lossy().to_string();
    attendre(&mut h, "l'instrument chargé", |s| s.instruments.charge(&chemin).is_some());
    // un fichier absent : un message clair, pas de plantage
    h.state_mut().ajouter_echantillonneur(&dossier.join("absent.sfz"));
    attendre(&mut h, "le message d'erreur", |s| s.message.as_deref().unwrap_or("").starts_with("Instrument illisible"));
    std::fs::remove_dir_all(&dossier).ok();
}
