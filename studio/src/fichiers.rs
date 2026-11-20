//! Fichiers et sûreté du travail : annuler et rétablir (une étape par geste), exports MIDI et
//! WAV (rendu par le moteur lui-même, dans un fil à part), projets récents, fiche du projet,
//! confirmation avant de perdre des modifications, reprise après plantage et copies de secours.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{self, RichText};
use projet::Projet;
use projet::export_midi::{nom_de_fichier, vers_midi};
use projet::historique::{Historique, meme_contenu};
use projet::lecture::ModeLecture;
use projet::modeles::Modele;
use projet::morceaux::Morceau;
use projet::serialisation::{deserialiser, serialiser};

use crate::app::Studio;
use crate::{pont, theme};

/// Fréquence des fichiers WAV exportés.
pub const FREQUENCE_EXPORT: u32 = 44_100;
/// Secondes ajoutées après la fin, pour laisser les sons et la réverbération s'éteindre.
pub const QUEUE_EXPORT: f64 = 3.0;
/// Projets récents gardés.
pub const RECENTS_MAX: usize = 10;
/// Clé des projets récents dans le stockage de l'application.
pub const CLE_RECENTS: &str = "recents";
/// Écriture du fichier de reprise au plus toutes les…
pub const INTERVALLE_REPRISE: Duration = Duration::from_secs(30);
/// Une copie de secours horodatée au plus toutes les…
pub const INTERVALLE_COPIE: Duration = Duration::from_secs(5 * 60);
/// Copies de secours gardées.
pub const COPIES_MAX: usize = 20;

/// Ce qui remplace le projet : demandé par l'utilisateur, confirmé s'il y a des modifications.
#[derive(Clone, Debug)]
pub enum Action {
    Modele(Modele),
    Fichier(PathBuf),
    Morceau(Morceau),
    ChoisirEtOuvrir,
    Fermer,
}

/// État des fichiers : historique, exports, récents, sauvegardes.
#[derive(Debug, Default)]
pub struct EtatFichiers {
    pub historique: Historique,
    /// Dernier état consigné dans l'historique.
    stable: Option<Projet>,
    /// Action qui attend la réponse à « Enregistrer les modifications ? ».
    pub en_attente: Option<Action>,
    /// La fenêtre peut se fermer (l'utilisateur a répondu).
    pub fermeture_permise: bool,
    pub recents: Vec<PathBuf>,
    pub fiche_ouverte: bool,
    /// Export WAV en cours : son résultat arrivera ici.
    export: Option<Receiver<Result<PathBuf, String>>>,
    /// Dossier des données de l'application (reprise, copies) ; None : pas de sauvegarde.
    pub donnees: Option<PathBuf>,
    derniere_reprise: Option<Instant>,
    derniere_copie: Option<Instant>,
    /// Projet de la reprise écrite en dernier (pour ne pas réécrire le même).
    reprise_ecrite: Option<Projet>,
    /// Travail non enregistré retrouvé au démarrage, et son âge.
    pub reprise_trouvee: Option<(Projet, Duration)>,
}

/// Dossier des données de l'application : %APPDATA%\song-audio, sinon ~/.local/share/song-audio.
pub fn dossier_donnees() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(d).join("song-audio"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/song-audio"))
}

/// Date et heure (UTC) pour un nom de fichier : 2026-11-20_21-45-09.
pub fn horodatage(instant: SystemTime) -> String {
    let s = instant.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (jours, reste) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // jours depuis 1970 -> date civile (algorithme de Howard Hinnant)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = yoe + ere * 400 + i64::from(mois <= 2);
    format!("{annee:04}-{mois:02}-{jour:02}_{:02}-{:02}-{:02}", reste / 3600, reste % 3600 / 60, reste % 60)
}

/// « il y a 5 minutes », « il y a 2 heures »…
pub fn age_lisible(age: Duration) -> String {
    let s = age.as_secs();
    match s {
        0..=59 => "il y a moins d'une minute".into(),
        60..=3599 => format!("il y a {} minute{}", s / 60, if s / 60 > 1 { "s" } else { "" }),
        3600..=86_399 => format!("il y a {} heure{}", s / 3600, if s / 3600 > 1 { "s" } else { "" }),
        _ => format!("il y a {} jour{}", s / 86_400, if s / 86_400 > 1 { "s" } else { "" }),
    }
}

impl Studio {
    // ---------------------------------------------------------------- annuler, rétablir

    /// Consigne dans l'historique ce qui a changé depuis la dernière étape, une fois le geste fini
    /// (bouton relâché, aucune saisie de texte) : un glisser entier est une seule étape.
    pub fn consigner_historique(&mut self, geste_en_cours: bool) {
        let Some(stable) = &self.fichiers.stable else {
            self.fichiers.stable = Some(self.projet.clone());
            return;
        };
        if geste_en_cours || *stable == self.projet {
            return;
        }
        let stable = self.fichiers.stable.take().expect("état stable");
        if !meme_contenu(&stable, &self.projet) {
            self.fichiers.historique.consigner(stable, &self.projet);
        }
        self.fichiers.stable = Some(self.projet.clone());
    }

    fn appliquer_etat(&mut self, projet: Projet) {
        self.projet = projet;
        self.fichiers.stable = Some(self.projet.clone());
        self.modifie = true;
        if self.canal_selectionne.as_deref().is_none_or(|id| self.projet.canal(id).is_none()) {
            self.canal_selectionne = self.projet.canaux.first().map(|c| c.id.clone());
        }
        self.renommage = None;
        self.renommage_piste = None;
    }

    pub fn annuler(&mut self) {
        self.consigner_historique(false);
        if let Some(p) = self.fichiers.historique.annuler(&self.projet) {
            self.appliquer_etat(p);
        }
    }

    pub fn retablir(&mut self) {
        self.consigner_historique(false);
        if let Some(p) = self.fichiers.historique.retablir(&self.projet) {
            self.appliquer_etat(p);
        }
    }

    /// Un nouveau projet repart d'un historique vide.
    pub(crate) fn historique_neuf(&mut self) {
        self.fichiers.historique.vider();
        self.fichiers.stable = Some(self.projet.clone());
    }

    // ---------------------------------------------------------------- remplacer le projet

    /// Exécute une action qui remplace le projet ; s'il y a des modifications non enregistrées,
    /// on demande d'abord.
    pub fn demander(&mut self, action: Action) {
        if self.modifie {
            self.fichiers.en_attente = Some(action);
        } else {
            self.executer(action);
        }
    }

    pub fn executer(&mut self, action: Action) {
        match action {
            Action::Modele(m) => {
                self.remplacer_projet(m.creer(), None);
                self.message = None;
            }
            Action::Fichier(chemin) => self.ouvrir_fichier(&chemin),
            Action::Morceau(m) => self.charger_morceau(&m),
            Action::ChoisirEtOuvrir => {
                if let Some(c) = rfd::FileDialog::new().set_title("Ouvrir un projet").add_filter("Projet song-audio", &["json"]).pick_file() {
                    self.ouvrir(&c);
                }
            }
            Action::Fermer => self.fichiers.fermeture_permise = true,
        }
    }

    /// Fenêtre « Enregistrer les modifications ? » ; vrai si la fenêtre doit se fermer.
    pub fn confirmation(&mut self, ctx: &egui::Context) -> bool {
        let Some(action) = self.fichiers.en_attente.clone() else { return false };
        let mut choix = None;
        egui::Modal::new(egui::Id::new("enregistrer ?")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(format!("Enregistrer les modifications de « {} » ?", self.projet.nom)).strong());
            ui.label(RichText::new("Sinon, elles seront perdues.").color(theme::DISCRET));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() {
                    choix = Some(0);
                }
                if ui.button("Ne pas enregistrer").clicked() {
                    choix = Some(1);
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    choix = Some(2);
                }
            });
        });
        match choix {
            Some(0) => {
                self.fichiers.en_attente = None;
                self.enregistrer(false);
                if !self.modifie {
                    self.executer(action);
                }
            }
            Some(1) => {
                self.fichiers.en_attente = None;
                self.modifie = false;
                self.supprimer_reprise();
                self.executer(action);
            }
            Some(2) => self.fichiers.en_attente = None,
            _ => {}
        }
        self.fichiers.fermeture_permise
    }

    // ---------------------------------------------------------------- enregistrer

    pub(crate) fn enregistrer(&mut self, sous: bool) {
        let chemin = match (&self.chemin, sous) {
            (Some(c), false) => Some(c.clone()),
            _ => rfd::FileDialog::new()
                .set_title("Enregistrer le projet")
                .add_filter("Projet song-audio", &["json"])
                .set_file_name(nom_de_fichier(&self.projet.nom, "song-audio.json"))
                .save_file(),
        };
        if let Some(c) = chemin {
            if let Err(e) = self.enregistrer_dans(&c) {
                self.message = Some(format!("Enregistrement impossible : {e}"));
            }
        }
    }

    /// Note un projet ouvert ou enregistré en tête des projets récents.
    pub(crate) fn noter_recent(&mut self, chemin: &Path) {
        self.fichiers.recents.retain(|c| c != chemin);
        self.fichiers.recents.insert(0, chemin.to_path_buf());
        self.fichiers.recents.truncate(RECENTS_MAX);
    }

    pub fn reprendre_recents(&mut self, stockage: Option<&dyn eframe::Storage>) {
        if let Some(r) = stockage.and_then(|s| eframe::get_value::<Vec<PathBuf>>(s, CLE_RECENTS)) {
            self.fichiers.recents = r.into_iter().take(RECENTS_MAX).collect();
        }
    }

    // ---------------------------------------------------------------- exports

    pub fn exporter_midi_dans(&self, chemin: &Path) -> Result<(), String> {
        std::fs::write(chemin, vers_midi(&self.projet, self.mode, false)).map_err(|e| e.to_string())
    }

    /// Rend le projet (pattern ou morceau, selon le mode) par le moteur et l'écrit en WAV.
    pub fn exporter_wav_dans(&self, chemin: &Path) -> Result<(), String> {
        let banque = self.banque().ok_or("aucune banque de sons")?;
        rendre_wav(&banque, &self.projet, self.mode, chemin)
    }

    fn choisir_export(&self, extension: &str, filtre: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(format!("Exporter en {}", extension.to_uppercase()))
            .add_filter(filtre, &[extension])
            .set_file_name(nom_de_fichier(&self.projet.nom, extension))
            .save_file()
    }

    pub(crate) fn exporter_midi(&mut self) {
        if let Some(c) = self.choisir_export("mid", "Fichier MIDI") {
            self.message = Some(match self.exporter_midi_dans(&c) {
                Ok(()) => format!("MIDI exporté : {}", c.display()),
                Err(e) => format!("Export MIDI impossible : {e}"),
            });
        }
    }

    /// Export WAV dans un fil à part : l'interface reste libre pendant le rendu.
    pub fn exporter_wav_vers(&mut self, chemin: PathBuf) {
        if self.fichiers.export.is_some() {
            return;
        }
        let Some(banque) = self.banque() else {
            self.message = Some("Export WAV impossible : aucune banque de sons.".into());
            return;
        };
        let (projet, mode) = (self.projet.clone(), self.mode);
        let (envoi, reception) = channel();
        std::thread::spawn(move || {
            let _ = envoi.send(rendre_wav(&banque, &projet, mode, &chemin).map(|()| chemin));
        });
        self.fichiers.export = Some(reception);
        self.message = Some(format!("Export WAV en cours ({})…", if mode == ModeLecture::Morceau { "morceau" } else { "pattern" }));
    }

    pub fn export_en_cours(&self) -> bool {
        self.fichiers.export.is_some()
    }

    pub(crate) fn exporter_wav(&mut self) {
        if let Some(c) = self.choisir_export("wav", "Fichier WAV") {
            self.exporter_wav_vers(c);
        }
    }

    /// Relève la fin d'un export WAV (appelé à chaque image).
    pub fn relever_export(&mut self) {
        let Some(r) = &self.fichiers.export else { return };
        if let Ok(resultat) = r.try_recv() {
            self.fichiers.export = None;
            self.message = Some(match resultat {
                Ok(c) => format!("WAV exporté : {}", c.display()),
                Err(e) => format!("Export WAV impossible : {e}"),
            });
        }
    }

    // ---------------------------------------------------------------- fiche du projet

    pub fn fiche_du_projet(&mut self, ctx: &egui::Context) {
        if !self.fichiers.fiche_ouverte {
            return;
        }
        let mut ouverte = true;
        let (mut nom, mut infos) = (self.projet.nom.clone(), self.projet.infos.clone());
        egui::Window::new("Infos du projet").open(&mut ouverte).resizable(true).default_width(420.0).show(ctx, |ui| {
            egui::Grid::new("fiche").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Titre");
                ui.add(egui::TextEdit::singleline(&mut nom).desired_width(300.0));
                ui.end_row();
                ui.label("Auteur");
                ui.add(egui::TextEdit::singleline(&mut infos.auteur).desired_width(300.0));
                ui.end_row();
                ui.label("Genre");
                ui.add(egui::TextEdit::singleline(&mut infos.genre).desired_width(300.0));
                ui.end_row();
            });
            ui.label("Notes");
            ui.add(egui::TextEdit::multiline(&mut infos.notes).desired_rows(8).desired_width(f32::INFINITY).hint_text("Idées, paroles, choses à faire…"));
        });
        self.fichiers.fiche_ouverte = ouverte;
        let nom = if nom.trim().is_empty() { self.projet.nom.clone() } else { nom };
        if nom != self.projet.nom || infos != self.projet.infos {
            self.modifier(|p| {
                p.nom = nom;
                p.infos = infos;
            });
        }
    }

    // ---------------------------------------------------------------- reprise et copies

    fn fichier_reprise(&self) -> Option<PathBuf> {
        self.fichiers.donnees.as_ref().map(|d| d.join("reprise.song-audio.json"))
    }

    /// Écrit le travail non enregistré (toutes les 30 s au plus, seulement s'il a changé) et une
    /// copie de secours horodatée (toutes les 5 min au plus, les 20 dernières gardées). Sans
    /// modification, le fichier de reprise disparaît.
    pub fn sauvegarder(&mut self, maintenant: Instant) {
        let Some(dossier) = self.fichiers.donnees.clone() else { return };
        if !self.modifie {
            if self.fichiers.reprise_ecrite.is_some() {
                self.supprimer_reprise();
            }
            return;
        }
        let du = |d: Option<Instant>, intervalle| d.is_none_or(|t| maintenant.duration_since(t) >= intervalle);
        if du(self.fichiers.derniere_reprise, INTERVALLE_REPRISE) && self.fichiers.reprise_ecrite.as_ref() != Some(&self.projet) {
            if std::fs::create_dir_all(&dossier).is_ok() && ecrire_sur(&dossier.join("reprise.song-audio.json"), &serialiser(&self.projet)).is_ok() {
                self.fichiers.reprise_ecrite = Some(self.projet.clone());
                self.fichiers.derniere_reprise = Some(maintenant);
            }
        }
        if du(self.fichiers.derniere_copie, INTERVALLE_COPIE) {
            let copies = dossier.join("copies");
            let nom = format!("{} {}.song-audio.json", horodatage(SystemTime::now()), nom_de_fichier(&self.projet.nom, "").trim_end_matches('.'));
            if std::fs::create_dir_all(&copies).is_ok() && ecrire_sur(&copies.join(nom), &serialiser(&self.projet)).is_ok() {
                self.fichiers.derniere_copie = Some(maintenant);
                elaguer_copies(&copies);
            }
        }
    }

    pub(crate) fn supprimer_reprise(&mut self) {
        if let Some(f) = self.fichier_reprise() {
            let _ = std::fs::remove_file(f);
        }
        self.fichiers.reprise_ecrite = None;
        self.fichiers.derniere_reprise = None;
    }

    /// Au démarrage : un fichier de reprise laissé par une session qui n'a pas pu enregistrer.
    pub fn chercher_reprise(&mut self) {
        let Some(f) = self.fichier_reprise() else { return };
        let age = std::fs::metadata(&f).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).unwrap_or_default();
        if let Some(p) = std::fs::read_to_string(&f).ok().and_then(|t| deserialiser(&t).ok()) {
            self.fichiers.reprise_trouvee = Some((p, age));
        }
    }

    /// Bandeau « Du travail non enregistré a été retrouvé ».
    pub fn bandeau_reprise(&mut self, ui: &mut egui::Ui) {
        let Some((projet, age)) = self.fichiers.reprise_trouvee.clone() else { return };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Du travail non enregistré a été retrouvé : « {} », {}.", projet.nom, age_lisible(age))).color(theme::ACCENT));
            if ui.button("Reprendre").clicked() {
                self.fichiers.reprise_trouvee = None;
                self.remplacer_projet(projet, None);
                self.modifie = true;
            }
            if ui.button("Ignorer").clicked() {
                self.fichiers.reprise_trouvee = None;
                self.supprimer_reprise();
            }
        });
    }

    // ---------------------------------------------------------------- menus

    pub(crate) fn menu_fichier(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Nouveau", |ui| {
            for m in Modele::TOUS {
                if ui.button(m.nom()).on_hover_text(m.description()).clicked() {
                    self.demander(Action::Modele(m));
                    ui.close();
                }
            }
        });
        if ui.button("Ouvrir un projet…   Ctrl+O").clicked() {
            ui.close();
            self.demander(Action::ChoisirEtOuvrir);
        }
        ui.add_enabled_ui(!self.fichiers.recents.is_empty(), |ui| {
            ui.menu_button("Projets récents", |ui| {
                for chemin in self.fichiers.recents.clone() {
                    let nom = chemin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    if ui.button(nom).on_hover_text(chemin.display().to_string()).clicked() {
                        self.demander(Action::Fichier(chemin));
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        if ui.button("Enregistrer   Ctrl+S").clicked() {
            ui.close();
            self.enregistrer(false);
        }
        if ui.button("Enregistrer sous…   Ctrl+Maj+S").clicked() {
            ui.close();
            self.enregistrer(true);
        }
        ui.separator();
        let quoi = if self.mode == ModeLecture::Morceau { "le morceau" } else { "le pattern" };
        if ui.button("Exporter en MIDI…").on_hover_text(format!("Exporte {quoi} (selon PAT ou SONG)")).clicked() {
            ui.close();
            self.exporter_midi();
        }
        if ui.add_enabled(!self.export_en_cours(), egui::Button::new("Exporter en WAV…")).on_hover_text(format!("Rend {quoi} en audio (selon PAT ou SONG)")).clicked() {
            ui.close();
            self.exporter_wav();
        }
        ui.separator();
        if ui.button("Infos du projet…").clicked() {
            ui.close();
            self.fichiers.fiche_ouverte = true;
        }
    }

    pub(crate) fn menu_edition(&mut self, ui: &mut egui::Ui) {
        let annuler = self.fichiers.historique.a_annuler().map(|l| format!("Annuler « {l} »   Ctrl+Z")).unwrap_or_else(|| "Annuler   Ctrl+Z".into());
        if ui.add_enabled(self.fichiers.historique.a_annuler().is_some(), egui::Button::new(annuler)).clicked() {
            self.annuler();
            ui.close();
        }
        let retablir = self.fichiers.historique.a_retablir().map(|l| format!("Rétablir « {l} »   Ctrl+Y")).unwrap_or_else(|| "Rétablir   Ctrl+Y".into());
        if ui.add_enabled(self.fichiers.historique.a_retablir().is_some(), egui::Button::new(retablir)).clicked() {
            self.retablir();
            ui.close();
        }
        ui.separator();
        let etapes: Vec<String> = self.fichiers.historique.etapes().iter().take(30).map(|s| s.to_string()).collect();
        ui.add_enabled_ui(!etapes.is_empty(), |ui| {
            ui.menu_button("Historique", |ui| {
                ui.label(RichText::new("Revenir avant :").small().color(theme::DISCRET));
                for (k, libelle) in etapes.iter().enumerate() {
                    if ui.button(libelle).clicked() {
                        for _ in 0..=k {
                            self.annuler();
                        }
                        ui.close();
                    }
                }
            });
        });
    }
}

/// Écrit par un fichier temporaire renommé : jamais de fichier à moitié écrit.
pub fn ecrire_sur(chemin: &Path, contenu: &str) -> std::io::Result<()> {
    let temporaire = chemin.with_extension("tmp");
    std::fs::write(&temporaire, contenu)?;
    std::fs::rename(&temporaire, chemin)
}

/// Garde les `COPIES_MAX` copies les plus récentes (leur nom commence par la date).
fn elaguer_copies(dossier: &Path) {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return };
    let mut copies: Vec<PathBuf> = lecture.flatten().map(|e| e.path()).filter(|c| c.to_string_lossy().ends_with(".song-audio.json")).collect();
    copies.sort();
    let trop = copies.len().saturating_sub(COPIES_MAX);
    for c in copies.into_iter().take(trop) {
        let _ = std::fs::remove_file(c);
    }
}

/// Rendu hors ligne par le moteur, écrit en WAV 16 bits stéréo.
fn rendre_wav(banque: &std::sync::Arc<moteur::SoundFont>, projet: &Projet, mode: ModeLecture, chemin: &Path) -> Result<(), String> {
    let configuration = pont::configuration(projet);
    let session = pont::session(projet, mode);
    let (g, d) = moteur::rendu::rendre_session(banque, FREQUENCE_EXPORT, &configuration, &session, QUEUE_EXPORT)?;
    std::fs::write(chemin, moteur::rendu::wav(&g, &d, FREQUENCE_EXPORT)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horodatage_et_age() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_763_675_109); // 20 nov. 2025, 21:45:09 UTC
        assert_eq!(horodatage(t), "2025-11-20_21-45-09");
        assert_eq!(horodatage(SystemTime::UNIX_EPOCH), "1970-01-01_00-00-00");
        assert_eq!(horodatage(SystemTime::UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29_00-00-00");
        assert_eq!(age_lisible(Duration::from_secs(20)), "il y a moins d'une minute");
        assert_eq!(age_lisible(Duration::from_secs(60)), "il y a 1 minute");
        assert_eq!(age_lisible(Duration::from_secs(7300)), "il y a 2 heures");
        assert_eq!(age_lisible(Duration::from_secs(3 * 86_400)), "il y a 3 jours");
    }

    #[test]
    fn on_garde_les_vingt_copies_les_plus_recentes() {
        let dossier = std::env::temp_dir().join(format!("song-audio-copies-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        for i in 0..25 {
            std::fs::write(dossier.join(format!("2026-11-{:02}_10-00-00 Essai.song-audio.json", i + 1)), "{}").unwrap();
        }
        std::fs::write(dossier.join("autre.txt"), "x").unwrap();
        elaguer_copies(&dossier);
        let mut restes: Vec<String> = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        restes.sort();
        assert_eq!(restes.len(), COPIES_MAX + 1);
        assert!(restes[0].starts_with("2026-11-06"), "les 5 plus anciennes sont parties : {:?}", restes[0]);
        assert!(restes.contains(&"autre.txt".to_string()));
        std::fs::remove_dir_all(&dossier).ok();
    }
}
