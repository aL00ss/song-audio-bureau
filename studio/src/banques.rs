//! Banques de sons à télécharger : des bibliothèques d'échantillons professionnelles, gratuites
//! et redistribuables (licences vérifiées), au format SFZ. Téléchargement dans un fil à part
//! (avec reprise d'un téléchargement interrompu quand le serveur le permet), extraction dans le
//! dossier des données de l'application, puis leurs instruments s'ajoutent comme
//! échantillonneurs.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// Format de l'archive téléchargée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archive {
    Zip,
    TarGz,
}

/// Une banque du catalogue.
#[derive(Clone, Debug, PartialEq)]
pub struct Banque {
    pub id: &'static str,
    pub nom: &'static str,
    pub famille: &'static str,
    pub auteur: &'static str,
    pub licence: &'static str,
    pub url: &'static str,
    pub archive: Archive,
    /// Taille approximative du téléchargement, en Mo.
    pub taille_mo: u32,
    pub description: &'static str,
}

const SFZI: &str = "https://github.com/sfzinstruments";

macro_rules! depot {
    ($depot:literal, $branche:literal) => {
        concat!("https://github.com/sfzinstruments/", $depot, "/archive/refs/heads/", $branche, ".zip")
    };
}

/// Le catalogue : seulement des licences qui permettent l'usage libre et la redistribution
/// (CC0, CC BY), vérifiées sur la page de chaque banque.
pub const CATALOGUE: [Banque; 14] = [
    Banque {
        id: "salamander",
        nom: "Salamander Grand Piano V3",
        famille: "Pianos",
        auteur: "Alexander Holm",
        licence: "CC BY 3.0",
        url: "https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/SalamanderGrandPiano-SFZ+FLAC-V3+20200602.tar.gz",
        archive: Archive::TarGz,
        taille_mo: 741,
        description: "Yamaha C5, 16 couches de vélocité, 48 kHz 24 bits, bruits de pédale et de relâchement",
    },
    Banque {
        id: "osiris",
        nom: "Osiris Piano",
        famille: "Pianos",
        auteur: "Versilian Studios et Karoryfer Samples",
        licence: "CC0",
        url: depot!("Osiris_Piano", "main"),
        archive: Archive::Zip,
        taille_mo: 450,
        description: "Piano droit au caractère marqué",
    },
    Banque {
        id: "vsco2",
        nom: "VSCO 2 Community Edition",
        famille: "Orchestre",
        auteur: "Versilian Studios",
        licence: "CC0",
        url: "https://github.com/sgossner/VSCO-2-CE/archive/refs/tags/1.1.0.zip",
        archive: Archive::Zip,
        taille_mo: 2400,
        description: "Orchestre complet : cordes, bois, cuivres, percussions, toutes articulations",
    },
    Banque {
        id: "vcsl",
        nom: "VCSL (Versilian Community Sample Library)",
        famille: "Instruments variés",
        auteur: "Versilian Studios",
        licence: "CC0",
        url: "https://github.com/sgossner/VCSL/archive/refs/tags/v1.2.2-RC.zip",
        archive: Archive::Zip,
        taille_mo: 4000,
        description: "Des centaines d'instruments : claviers, percussions du monde, vents, cordes",
    },
    Banque {
        id: "naked-drums",
        nom: "Naked Drums",
        famille: "Batteries",
        auteur: "Wilkinson Audio",
        licence: "CC BY 4.0",
        url: depot!("WilkinsonAudio.NakedDrums", "master"),
        archive: Archive::Zip,
        taille_mo: 1250,
        description: "Batterie acoustique multi-micros, 10 prises alternées, jusqu'à 5 couches",
    },
    Banque {
        id: "big-rusty-drums",
        nom: "Big Rusty Drums",
        famille: "Batteries",
        auteur: "Karoryfer Samples",
        licence: "CC0",
        url: depot!("karoryfer.big-rusty-drums", "main"),
        archive: Archive::Zip,
        taille_mo: 600,
        description: "Batterie rock au son brut",
    },
    Banque {
        id: "drs-kit",
        nom: "DrumGizmo DRS Kit",
        famille: "Batteries",
        auteur: "DrumGizmo (Lars Muldjord et contributeurs)",
        licence: "CC BY 4.0",
        url: depot!("DrumGizmo.DRSKit", "master"),
        archive: Archive::Zip,
        taille_mo: 730,
        description: "Batterie acoustique complète",
    },
    Banque {
        id: "black-and-blue-basses",
        nom: "Black And Blue Basses",
        famille: "Basses",
        auteur: "Karoryfer Samples",
        licence: "CC0",
        url: depot!("karoryfer.black-and-blue-basses", "main"),
        archive: Archive::Zip,
        taille_mo: 980,
        description: "Basses électriques, doigts et médiator",
    },
    Banque {
        id: "growlybass",
        nom: "Growlybass",
        famille: "Basses",
        auteur: "Karoryfer Samples",
        licence: "CC0",
        url: depot!("karoryfer.growlybass", "master"),
        archive: Archive::Zip,
        taille_mo: 165,
        description: "Basse électrique grondante",
    },
    Banque {
        id: "black-and-green-guitars",
        nom: "Black And Green Guitars",
        famille: "Guitares",
        auteur: "Karoryfer Samples",
        licence: "CC0",
        url: depot!("karoryfer.black-and-green-guitars", "main"),
        archive: Archive::Zip,
        taille_mo: 455,
        description: "Guitares électriques",
    },
    Banque {
        id: "violoncelle",
        nom: "Violoncelle (Karoryfer et Bigcat)",
        famille: "Cordes",
        auteur: "Karoryfer Samples et Bigcat Instruments",
        licence: "CC0",
        url: depot!("karoryfer-bigcat.cello", "master"),
        archive: Archive::Zip,
        taille_mo: 130,
        description: "Violoncelle solo",
    },
    Banque {
        id: "contrebasse",
        nom: "Contrebasse (D. Smolken)",
        famille: "Cordes",
        auteur: "D. Smolken",
        licence: "CC0",
        url: depot!("dsmolken.double-bass", "master"),
        archive: Archive::Zip,
        taille_mo: 260,
        description: "Contrebasse, arco et pizzicato",
    },
    Banque {
        id: "solo-sax",
        nom: "Saxophones solistes",
        famille: "Vents",
        auteur: "Music Technology Group (Université Pompeu Fabra)",
        licence: "CC BY 4.0",
        url: depot!("MTG.SoloSax", "master"),
        archive: Archive::Zip,
        taille_mo: 105,
        description: "Saxophones soprano, alto, ténor et baryton",
    },
    Banque {
        id: "war-tuba",
        nom: "War Tuba",
        famille: "Vents",
        auteur: "Karoryfer Samples",
        licence: "CC0",
        url: depot!("karoryfer.war-tuba", "master"),
        archive: Archive::Zip,
        taille_mo: 105,
        description: "Tuba",
    },
];

/// Page des bibliothèques SFZ libres d'où vient l'essentiel du catalogue.
pub const SOURCE_SFZ: &str = SFZI;

pub fn banque(id: &str) -> Option<&'static Banque> {
    CATALOGUE.iter().find(|b| b.id == id)
}

/// Dossier d'une banque installée.
pub fn dossier_banque(racine: &Path, id: &str) -> PathBuf {
    racine.join(id)
}

/// Une banque est installée quand son dossier porte la marque de fin d'installation.
pub fn installee(racine: &Path, id: &str) -> bool {
    dossier_banque(racine, id).join(".installee").exists()
}

/// Les instruments (fichiers .sfz) d'une banque installée, par ordre alphabétique de chemin.
pub fn instruments(racine: &Path, id: &str) -> Vec<PathBuf> {
    fn parcourir(d: &Path, sortie: &mut Vec<PathBuf>, profondeur: usize) {
        let Ok(lecture) = std::fs::read_dir(d) else { return };
        for e in lecture.flatten() {
            let c = e.path();
            if c.is_dir() && profondeur < 8 {
                parcourir(&c, sortie, profondeur + 1);
            } else if c.extension().is_some_and(|x| x.eq_ignore_ascii_case("sfz")) {
                sortie.push(c);
            }
        }
    }
    let mut r = Vec::new();
    parcourir(&dossier_banque(racine, id), &mut r, 0);
    // les fichiers inclus par un autre (morceaux d'instrument) ne sont pas des instruments
    let normaliser = |c: &Path| -> PathBuf {
        let mut n = PathBuf::new();
        for partie in c.components() {
            match partie {
                std::path::Component::ParentDir => {
                    n.pop();
                }
                std::path::Component::CurDir => {}
                autre => n.push(autre),
            }
        }
        n
    };
    let inclus: std::collections::HashSet<PathBuf> = r
        .iter()
        .flat_map(|c| {
            let dossier = c.parent().map(Path::to_path_buf).unwrap_or_default();
            let texte = std::fs::read_to_string(c).unwrap_or_default();
            moteur::sfz::inclusions(&texte).into_iter().map(move |i| normaliser(&dossier.join(i))).collect::<Vec<_>>()
        })
        .collect();
    r.retain(|c| !inclus.contains(&normaliser(c)));
    r.sort();
    r
}

/// Supprime une banque installée (et un téléchargement partiel).
pub fn supprimer(racine: &Path, id: &str) -> std::io::Result<()> {
    let _ = std::fs::remove_file(racine.join(format!("{id}.part")));
    match std::fs::remove_dir_all(dossier_banque(racine, id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Un téléchargement en cours : sa progression et son résultat.
pub struct Telechargement {
    pub recus: Arc<AtomicU64>,
    /// Taille annoncée par le serveur (0 : inconnue).
    pub total: Arc<AtomicU64>,
    annule: Arc<AtomicBool>,
    pub fin: Receiver<Result<(), String>>,
}

impl Telechargement {
    pub fn annuler(&self) {
        self.annule.store(true, Ordering::Relaxed);
    }
}

/// Télécharge et installe une banque dans `racine/<id>` (un fichier `<id>.part` garde ce qui est
/// déjà reçu : une reprise continue là où elle s'était arrêtée).
pub fn telecharger(url: &str, archive: Archive, racine: &Path, id: &str) -> Telechargement {
    let (recus, total, annule) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
    let (envoi, fin) = channel();
    let (r, t, a) = (recus.clone(), total.clone(), annule.clone());
    let (url, racine, id) = (url.to_string(), racine.to_path_buf(), id.to_string());
    std::thread::spawn(move || {
        let resultat = recevoir(&url, &racine, &id, &r, &t, &a).and_then(|partiel| installer(&partiel, archive, &racine, &id));
        let _ = envoi.send(resultat);
    });
    Telechargement { recus, total, annule, fin }
}

/// Reçoit l'archive dans `<id>.part` ; renvoie son chemin.
fn recevoir(url: &str, racine: &Path, id: &str, recus: &AtomicU64, total: &AtomicU64, annule: &AtomicBool) -> Result<PathBuf, String> {
    std::fs::create_dir_all(racine).map_err(|e| e.to_string())?;
    let partiel = racine.join(format!("{id}.part"));
    let deja = std::fs::metadata(&partiel).map(|m| m.len()).unwrap_or(0);
    let config = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(20))).timeout_recv_body(Some(Duration::from_secs(60))).build();
    let agent = ureq::Agent::new_with_config(config);
    let mut requete = agent.get(url).header("User-Agent", "song-audio");
    if deja > 0 {
        requete = requete.header("Range", &format!("bytes={deja}-"));
    }
    let reponse = requete.call().map_err(|e| format!("téléchargement impossible : {e}"))?;
    // 206 : le serveur reprend où l'on en était ; 200 : il renvoie tout, on recommence
    let reprise = reponse.status().as_u16() == 206;
    let longueur = reponse.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let debut = if reprise { deja } else { 0 };
    total.store(if longueur > 0 { debut + longueur } else { 0 }, Ordering::Relaxed);
    recus.store(debut, Ordering::Relaxed);
    let mut fichier = OpenOptions::new().create(true).write(true).truncate(!reprise).open(&partiel).map_err(|e| e.to_string())?;
    if reprise {
        fichier.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    }
    let mut corps = reponse.into_body().into_reader();
    let mut tampon = vec![0u8; 1 << 16];
    loop {
        if annule.load(Ordering::Relaxed) {
            return Err("téléchargement annulé".into());
        }
        let n = corps.read(&mut tampon).map_err(|e| format!("téléchargement interrompu : {e}"))?;
        if n == 0 {
            break;
        }
        fichier.write_all(&tampon[..n]).map_err(|e| e.to_string())?;
        recus.fetch_add(n as u64, Ordering::Relaxed);
    }
    fichier.flush().map_err(|e| e.to_string())?;
    let attendu = total.load(Ordering::Relaxed);
    if attendu > 0 && recus.load(Ordering::Relaxed) < attendu {
        return Err("téléchargement incomplet".into());
    }
    Ok(partiel)
}

/// Extrait l'archive dans un dossier provisoire, puis le met à sa place ; l'archive est retirée.
fn installer(partiel: &Path, archive: Archive, racine: &Path, id: &str) -> Result<(), String> {
    let provisoire = racine.join(format!("{id}.extraction"));
    let _ = std::fs::remove_dir_all(&provisoire);
    std::fs::create_dir_all(&provisoire).map_err(|e| e.to_string())?;
    let fichier = File::open(partiel).map_err(|e| e.to_string())?;
    match archive {
        Archive::Zip => zip::ZipArchive::new(fichier).and_then(|mut z| z.extract(&provisoire)).map_err(|e| format!("archive illisible : {e}"))?,
        Archive::TarGz => tar::Archive::new(flate2::read::GzDecoder::new(fichier)).unpack(&provisoire).map_err(|e| format!("archive illisible : {e}"))?,
    }
    let destination = dossier_banque(racine, id);
    let _ = std::fs::remove_dir_all(&destination);
    std::fs::rename(&provisoire, &destination).map_err(|e| e.to_string())?;
    std::fs::write(destination.join(".installee"), b"").map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(partiel);
    if instruments(racine, id).is_empty() {
        return Err("l'archive ne contient aucun instrument SFZ".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_complet_et_licences_libres() {
        let mut ids: Vec<&str> = CATALOGUE.iter().map(|b| b.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOGUE.len(), "identifiants uniques");
        for b in &CATALOGUE {
            assert!(b.url.starts_with("https://"), "{}", b.id);
            assert!(["CC0", "CC BY 3.0", "CC BY 4.0"].contains(&b.licence), "{} : {}", b.id, b.licence);
            assert_eq!(b.archive == Archive::TarGz, b.url.ends_with(".tar.gz"), "{}", b.id);
        }
        assert_eq!(banque("salamander").unwrap().licence, "CC BY 3.0");
        assert!(banque("inconnue").is_none());
    }
}

// ------------------------------------------------------------------ dans l'application

use eframe::egui::{self, RichText};

use crate::app::Studio;
use crate::theme;

/// Téléchargements en cours, par banque.
#[derive(Default)]
pub struct EtatBanques {
    pub en_cours: std::collections::HashMap<&'static str, Telechargement>,
}

impl std::fmt::Debug for EtatBanques {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtatBanques").field("en_cours", &self.en_cours.keys().collect::<Vec<_>>()).finish()
    }
}

impl Studio {
    /// Dossier où s'installent les banques (dans les données de l'application).
    pub fn dossier_des_banques(&self) -> Option<PathBuf> {
        self.fichiers.donnees.as_ref().map(|d| d.join("banques"))
    }

    /// Lance le téléchargement d'une banque du catalogue depuis cette adresse.
    pub fn telecharger_banque_depuis(&mut self, id: &'static str, url: &str, archive: Archive) {
        let Some(racine) = self.dossier_des_banques() else {
            self.message = Some("Téléchargement impossible : dossier des données introuvable.".into());
            return;
        };
        if self.banques.en_cours.contains_key(id) {
            return;
        }
        self.banques.en_cours.insert(id, telecharger(url, archive, &racine, id));
    }

    /// Relève les téléchargements finis (appelé à chaque image).
    pub fn relever_banques(&mut self) {
        let finis: Vec<(&'static str, Result<(), String>)> = self.banques.en_cours.iter().filter_map(|(id, t)| t.fin.try_recv().ok().map(|r| (*id, r))).collect();
        for (id, resultat) in finis {
            self.banques.en_cours.remove(id);
            let nom = banque(id).map(|b| b.nom).unwrap_or(id);
            self.message = Some(match resultat {
                Ok(()) => {
                    let n = self.dossier_des_banques().map(|r| instruments(&r, id).len()).unwrap_or(0);
                    format!("Banque « {nom} » installée ({n} instruments).")
                }
                Err(e) if e == "téléchargement annulé" => format!("Téléchargement de « {nom} » annulé (ce qui est reçu est gardé pour reprendre)."),
                Err(e) => format!("Banque « {nom} » : {e}"),
            });
        }
    }

    pub fn banques_en_cours(&self) -> bool {
        !self.banques.en_cours.is_empty()
    }

    /// La section « Banques de sons » du navigateur.
    pub fn section_banques(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("BANQUES DE SONS").small().strong().color(theme::DISCRET));
        let Some(racine) = self.dossier_des_banques() else {
            ui.label(RichText::new("Dossier des données introuvable : banques indisponibles.").small().color(theme::DISCRET));
            return;
        };
        ui.label(RichText::new("Échantillons professionnels gratuits (CC0, CC BY), au format SFZ").small().color(theme::DISCRET));
        let mut familles: Vec<&str> = CATALOGUE.iter().map(|b| b.famille).collect();
        familles.dedup();
        for famille in familles {
            egui::CollapsingHeader::new(famille).id_salt(("banques", famille)).show(ui, |ui| {
                for b in CATALOGUE.iter().filter(|b| b.famille == famille) {
                    self.ligne_banque(ui, &racine, b);
                }
            });
        }
    }

    fn ligne_banque(&mut self, ui: &mut egui::Ui, racine: &Path, b: &'static Banque) {
        let aide = format!("{}\n{} — {}\n{}", b.description, b.auteur, b.licence, b.url);
        if let Some(t) = self.banques.en_cours.get(b.id) {
            let (recus, total) = (t.recus.load(Ordering::Relaxed), t.total.load(Ordering::Relaxed));
            let total = if total > 0 { total } else { b.taille_mo as u64 * 1_000_000 };
            ui.label(RichText::new(b.nom).color(theme::TEXTE)).on_hover_text(&aide);
            ui.horizontal(|ui| {
                let part = (recus as f32 / total.max(1) as f32).min(1.0);
                ui.add(egui::ProgressBar::new(part).desired_width(150.0).text(format!("{} / {} Mo", recus / 1_000_000, total / 1_000_000)));
                if ui.small_button("Annuler").clicked() {
                    t.annuler();
                }
            });
            return;
        }
        if installee(racine, b.id) {
            let dossier = dossier_banque(racine, b.id);
            let reponse = egui::CollapsingHeader::new(RichText::new(b.nom).color(theme::ACCENT)).id_salt(("banque", b.id)).show(ui, |ui| {
                for chemin in instruments(racine, b.id) {
                    let nom = chemin.strip_prefix(&dossier).unwrap_or(&chemin).with_extension("").to_string_lossy().replace('\\', "/");
                    let court = nom.rsplit('/').next().unwrap_or(&nom).to_string();
                    let bouton = ui.add(egui::Button::new(format!("+ {court}")).frame(false).sense(egui::Sense::click_and_drag())).on_hover_text(nom);
                    bouton.dnd_set_drag_payload(crate::rack::SonGlisse::Sfz(chemin.clone()));
                    if bouton.clicked() {
                        self.ajouter_echantillonneur(&chemin);
                    }
                }
            });
            reponse.header_response.on_hover_text(&aide).context_menu(|ui| {
                if ui.button("Supprimer la banque").clicked() {
                    self.message = Some(match supprimer(racine, b.id) {
                        Ok(()) => format!("Banque « {} » supprimée.", b.nom),
                        Err(e) => format!("Suppression impossible : {e}"),
                    });
                    ui.close();
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.label(b.nom).on_hover_text(&aide);
            let reprise = racine.join(format!("{}.part", b.id)).exists();
            let texte = if reprise { "Reprendre".to_string() } else { format!("Télécharger ({} Mo)", b.taille_mo) };
            if ui.small_button(texte).on_hover_text(&aide).clicked() {
                self.telecharger_banque_depuis(b.id, b.url, b.archive);
            }
        });
    }
}
