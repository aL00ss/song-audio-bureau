//! D3 : téléchargement et installation des banques de sons, contre un petit serveur HTTP local
//! (aucun accès à Internet) : zip et tar.gz, reprise après une coupure, annulation, archive
//! sans instrument.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use studio::banques::{Archive, installee, instruments, supprimer, telecharger};

/// Un WAV mono minuscule.
fn wav() -> Vec<u8> {
    let n = 2000usize;
    let mut o = Vec::new();
    o.extend_from_slice(b"RIFF");
    o.extend_from_slice(&(36 + n as u32 * 2).to_le_bytes());
    o.extend_from_slice(b"WAVEfmt ");
    o.extend_from_slice(&16u32.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&44100u32.to_le_bytes());
    o.extend_from_slice(&88200u32.to_le_bytes());
    o.extend_from_slice(&2u16.to_le_bytes());
    o.extend_from_slice(&16u16.to_le_bytes());
    o.extend_from_slice(b"data");
    o.extend_from_slice(&(n as u32 * 2).to_le_bytes());
    for i in 0..n {
        o.extend_from_slice(&(((i as f32 * 0.06).sin() * 12000.0) as i16).to_le_bytes());
    }
    o
}

/// Une archive zip comme celles de GitHub : un dossier racine, un SFZ, des sons.
fn archive_zip(avec_sfz: bool) -> Vec<u8> {
    let mut tampon = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut tampon);
        let options = zip::write::SimpleFileOptions::default();
        if avec_sfz {
            z.start_file("kit-main/Kit.sfz", options).unwrap();
            z.write_all(b"<region> sample=sons/la.wav key=60").unwrap();
        }
        z.start_file("kit-main/sons/la.wav", options).unwrap();
        z.write_all(&wav()).unwrap();
        // remplissage, pour que la coupure tombe au milieu
        z.start_file("kit-main/LISEZMOI.txt", options).unwrap();
        z.write_all(&vec![b'x'; 200_000]).unwrap();
        z.finish().unwrap();
    }
    tampon.into_inner()
}

fn archive_tar_gz() -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut t = tar::Builder::new(&mut gz);
        for (chemin, contenu) in [("Piano/Piano.sfz", b"<region> sample=la.wav".to_vec()), ("Piano/la.wav", wav())] {
            let mut entete = tar::Header::new_gnu();
            entete.set_size(contenu.len() as u64);
            entete.set_mode(0o644);
            entete.set_cksum();
            t.append_data(&mut entete, chemin, contenu.as_slice()).unwrap();
        }
        t.finish().unwrap();
    }
    gz.finish().unwrap()
}

/// Comportement du serveur pour chaque requête.
#[derive(Clone, Copy)]
enum Mode {
    Normal,
    /// La première réponse est coupée après ce nombre d'octets (les suivantes sont complètes).
    CoupureApres(usize),
    /// Envoi lent (pour pouvoir annuler).
    Lent,
}

/// Petit serveur HTTP : sert `contenu`, gère « Range » ; note les plages demandées.
fn serveur(contenu: Vec<u8>, mode: Mode) -> (String, Arc<Mutex<Vec<Option<u64>>>>) {
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let adresse = format!("http://{}/banque", ecoute.local_addr().unwrap());
    let plages = Arc::new(Mutex::new(Vec::new()));
    let p = plages.clone();
    std::thread::spawn(move || {
        let mut premiere = true;
        for flux in ecoute.incoming() {
            let Ok(mut flux) = flux else { continue };
            let mut requete = Vec::new();
            let mut octet = [0u8; 1];
            while !requete.ends_with(b"\r\n\r\n") && flux.read(&mut octet).unwrap_or(0) == 1 {
                requete.push(octet[0]);
            }
            let texte = String::from_utf8_lossy(&requete).to_lowercase();
            let debut = texte.lines().find_map(|l| l.strip_prefix("range: bytes=")).and_then(|r| r.trim().trim_end_matches('-').parse::<u64>().ok());
            p.lock().unwrap().push(debut);
            let debut = debut.unwrap_or(0) as usize;
            let corps = &contenu[debut.min(contenu.len())..];
            let entete = if debut > 0 {
                format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {debut}-{}/{}\r\nConnection: close\r\n\r\n", corps.len(), contenu.len() - 1, contenu.len())
            } else {
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corps.len())
            };
            let _ = flux.write_all(entete.as_bytes());
            match mode {
                Mode::CoupureApres(n) if premiere => {
                    let _ = flux.write_all(&corps[..n.min(corps.len())]);
                    // coupure : la connexion se ferme avant la fin annoncée
                }
                Mode::Lent => {
                    for morceau in corps.chunks(4096) {
                        if flux.write_all(morceau).is_err() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                _ => {
                    let _ = flux.write_all(corps);
                }
            }
            premiere = false;
        }
    });
    (adresse, plages)
}

fn racine(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("song-audio-banques-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn attendre(t: &studio::banques::Telechargement) -> Result<(), String> {
    t.fin.recv_timeout(Duration::from_secs(20)).expect("le téléchargement se termine")
}

#[test]
fn installer_une_banque_zip_et_une_banque_tar_gz() {
    let r = racine("installer");
    let (url, _) = serveur(archive_zip(true), Mode::Normal);
    let t = telecharger(&url, Archive::Zip, &r, "kit");
    attendre(&t).unwrap();
    assert!(installee(&r, "kit"));
    let liste = instruments(&r, "kit");
    assert_eq!(liste.len(), 1);
    assert!(liste[0].ends_with(Path::new("kit-main/Kit.sfz")));
    assert!(!r.join("kit.part").exists(), "l'archive est retirée après l'installation");
    // l'instrument se charge
    let (i, avertissements) = moteur::echantillonneur::InstrumentEchantillons::depuis_sfz(&liste[0]).unwrap();
    assert_eq!((i.zones.len(), avertissements.len()), (1, 0));
    // tar.gz
    let (url, _) = serveur(archive_tar_gz(), Mode::Normal);
    attendre(&telecharger(&url, Archive::TarGz, &r, "piano")).unwrap();
    assert_eq!(instruments(&r, "piano").len(), 1);
    // supprimer
    supprimer(&r, "kit").unwrap();
    assert!(!installee(&r, "kit") && installee(&r, "piano"));
    std::fs::remove_dir_all(&r).ok();
}

#[test]
fn reprise_apres_une_coupure() {
    let r = racine("reprise");
    let contenu = archive_zip(true);
    let moitie = contenu.len() / 2;
    let (url, plages) = serveur(contenu.clone(), Mode::CoupureApres(moitie));
    let t = telecharger(&url, Archive::Zip, &r, "kit");
    let e = attendre(&t).unwrap_err();
    assert!(e.contains("incomplet") || e.contains("interrompu"), "{e}");
    assert_eq!(std::fs::metadata(r.join("kit.part")).unwrap().len(), moitie as u64, "ce qui est reçu est gardé");
    assert!(!installee(&r, "kit"));
    // on relance : le serveur reçoit une demande de reprise à la bonne position
    let t = telecharger(&url, Archive::Zip, &r, "kit");
    attendre(&t).unwrap();
    assert_eq!(*plages.lock().unwrap(), vec![None, Some(moitie as u64)]);
    assert_eq!(t.recus.load(std::sync::atomic::Ordering::Relaxed), contenu.len() as u64);
    assert!(installee(&r, "kit"));
    std::fs::remove_dir_all(&r).ok();
}

#[test]
fn annuler_et_archive_sans_instrument() {
    let r = racine("annuler");
    let (url, _) = serveur(archive_zip(true), Mode::Lent);
    let t = telecharger(&url, Archive::Zip, &r, "kit");
    let debut = Instant::now();
    while t.recus.load(std::sync::atomic::Ordering::Relaxed) == 0 {
        assert!(debut.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    t.annuler();
    assert_eq!(attendre(&t).unwrap_err(), "téléchargement annulé");
    assert!(!installee(&r, "kit"));
    // une archive sans fichier SFZ est refusée
    let (url, _) = serveur(archive_zip(false), Mode::Normal);
    let e = attendre(&telecharger(&url, Archive::Zip, &r, "vide")).unwrap_err();
    assert!(e.contains("aucun instrument SFZ"), "{e}");
    std::fs::remove_dir_all(&r).ok();
}

#[test]
fn dans_le_navigateur_telecharger_puis_ajouter_un_instrument() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    let r = racine("interface");
    std::fs::create_dir_all(&r).unwrap();
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources/banques/GeneralUser-GS.sf2");
    let banque = moteur::charger_banque(&chemin).unwrap();
    let donnees = r.clone();
    let mut h = Harness::builder().with_size([1400.0, 1600.0]).build_eframe(move |cc| {
        studio::app::appliquer_theme(&cc.egui_ctx);
        let mut s = studio::Studio::new(Some(banque));
        s.fichiers.donnees = Some(donnees);
        s
    });
    h.run_steps(2);
    // la section et le catalogue (famille « Basses »)
    assert!(h.query_by_label("BANQUES DE SONS").is_some());
    // « Basses » est aussi une famille d'instruments General MIDI : la section des banques vient après
    h.query_all_by_label("Basses").last().unwrap().click();
    h.run_steps(2);
    assert!(h.query_by_label("Télécharger (165 Mo)").is_some(), "Growlybass, pas encore installée");
    // téléchargement (depuis le serveur local, à la place de GitHub)
    let (url, _) = serveur(archive_zip(true), Mode::Lent);
    h.state_mut().telecharger_banque_depuis("growlybass", &url, Archive::Zip);
    h.run_steps(3);
    assert!(h.query_by_label("Annuler").is_some(), "progression affichée");
    let fin = Instant::now() + Duration::from_secs(20);
    while h.state().banques_en_cours() {
        assert!(Instant::now() < fin);
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    assert_eq!(h.state().message.as_deref(), Some("Banque « Growlybass » installée (1 instruments)."));
    // la banque installée liste son instrument ; un clic l'ajoute au rack
    h.get_by_label("Growlybass").click();
    h.run_steps(2);
    h.get_by_label("+ Kit").click();
    h.run_steps(2);
    let canal = h.state().projet.canaux.last().unwrap().clone();
    assert_eq!((canal.nom.as_str(), canal.source), ("Kit", projet::SourceCanal::Echantillons));
    std::fs::remove_dir_all(&r).ok();
}

#[test]
#[ignore = "réseau : lancer avec RESEAU=1 cargo test --test banques -- --ignored"]
fn vrai_telechargement_depuis_github() {
    if std::env::var("RESEAU").is_err() {
        return;
    }
    let r = racine("reseau");
    let b = studio::banques::banque("war-tuba").unwrap();
    let t = telecharger(b.url, b.archive, &r, b.id);
    let resultat = t.fin.recv_timeout(Duration::from_secs(600)).expect("fin du téléchargement");
    resultat.unwrap();
    let liste = instruments(&r, b.id);
    assert!(!liste.is_empty());
    let (i, avertissements) = moteur::echantillonneur::InstrumentEchantillons::depuis_sfz(&liste[0]).unwrap();
    eprintln!("{} : {} instruments, {} zones dans {:?}, {} avertissements", b.nom, liste.len(), i.zones.len(), liste[0].file_name(), avertissements.len());
    // mémoire des échantillons, et une note jouée : peu de voix, du son, pas de saturation
    let mut vus = std::collections::HashSet::new();
    let octets: usize = i.zones.iter().filter(|z| vus.insert(std::sync::Arc::as_ptr(&z.echantillon))).map(|z| z.echantillon.donnees.octets()).sum();
    eprintln!("échantillons en mémoire : {} Mo", octets / 1_000_000);
    use moteur::Source;
    let mut e = moteur::echantillonneur::Echantillonneur::new(44100);
    e.changer_instrument(Some(std::sync::Arc::new(i)));
    e.note_on(40, 100);
    let voix = e.voix();
    let (mut g, mut d) = (vec![0.0f32; 22050], vec![0.0f32; 22050]);
    e.rendre(&mut g, &mut d);
    let crete = g.iter().chain(d.iter()).fold(0.0f32, |m, v| m.max(v.abs()));
    eprintln!("une note : {voix} voix, crête {crete:.3}");
    assert!((1..=4).contains(&voix), "{voix} voix pour une note");
    assert!(crete > 0.01 && crete < 1.0, "crête {crete}");
    std::fs::remove_dir_all(&r).ok();
}
