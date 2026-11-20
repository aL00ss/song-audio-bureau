//! L'application native : l'état du studio (projet, moteur, fichier) et son interface. Le moteur
//! vit dans le même programme : aucune communication entre processus, l'interface lit
//! directement l'horloge, la position et les vumètres.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText, Vec2};
use moteur::transport::TICS_PAR_NOIRE;
use moteur::{ActionTransport, EtatTransport, Moteur, SoundFont};
use projet::gm::{FAMILLES, famille_instrument, nom_instrument};
use projet::lecture::ModeLecture;
use projet::modele::OptionsCanal;
use projet::serialisation::{deserialiser, serialiser};
use projet::synthes::PREREGLAGES_SOUSTRACTIF;
use projet::temps::{formater_position, formater_secondes, tics_en_secondes};
use projet::{Projet, SourceCanal};

use crate::coquille::{self, Panneau, Vue};
use crate::navigateur::Navigateur;
use projet::morceaux::{Morceau, lire_catalogue, nom_court};
use crate::{pont, theme};
use egui_dock::{DockArea, DockState};

/// Clé de la disposition des panneaux dans le stockage de l'application.
pub const CLE_DISPOSITION: &str = "disposition";

/// Où trouver une ressource (banque, morceaux) : à côté de l'exécutable, ou dans le dépôt
/// (développement).
pub fn chemin_ressource(relatif: &str) -> Option<PathBuf> {
    let depot = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let candidats = [
        std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(relatif))),
        Some(depot.join("public").join(relatif)),
        Some(depot.join("src-tauri").join(relatif)),
    ];
    candidats.into_iter().flatten().find(|c| c.exists())
}

pub fn chemin_banque() -> Option<PathBuf> {
    chemin_ressource("banques/GeneralUser-GS.sf2")
}

/// Les morceaux fournis (catalogue à côté de l'exécutable ou dans le dépôt).
pub fn catalogue_des_morceaux() -> Vec<Morceau> {
    chemin_ressource("morceaux/catalogue.json")
        .and_then(|c| std::fs::read_to_string(c).ok())
        .and_then(|t| lire_catalogue(&t).ok())
        .unwrap_or_default()
}

pub struct Studio {
    pub projet: Projet,
    pub mode: ModeLecture,
    pub metronome: bool,
    /// Fichier d'où vient le projet ; None s'il n'a jamais été enregistré.
    pub chemin: Option<PathBuf>,
    /// Modifié depuis l'ouverture ou le dernier enregistrement.
    pub modifie: bool,
    pub message: Option<String>,
    pub canal_selectionne: Option<String>,
    moteur: Option<Moteur>,
    /// Ce que le moteur joue déjà (pour ne lui envoyer que les changements).
    envoye: Option<(Projet, ModeLecture)>,
    /// Disposition des panneaux (sortie le temps de dessiner les onglets).
    pub dock: Option<DockState<Panneau>>,
    pub navigateur: Navigateur,
    /// Peinture des pas en cours (bouton tenu).
    pub peinture: Option<crate::rack::Peinture>,
    /// Abscisses des colonnes de pas à l'écran (image courante), pour aligner les vélocités.
    pub(crate) colonnes_pas: Vec<egui::Rangef>,
    /// Canal en cours de renommage, et le texte saisi.
    pub renommage: Option<(String, String)>,
    /// Jeu en direct (notes tenues, clavier de l'ordinateur, enregistrement).
    pub jeu: crate::jeu::Jeu,
    pub piano_roll: crate::piano_roll::EtatPianoRoll,
    pub instrument: crate::instrument::EtatInstrument,
    /// Mesures du moteur lues à cette image (les crêtes ne se lisent qu'une fois).
    pub mesures: moteur::Mesures,
    /// Niveau affiché de chaque vumètre (avec sa retombée).
    pub niveaux: Vec<[f32; 2]>,
    /// Piste du mixer en cours de renommage, et le texte saisi.
    pub renommage_piste: Option<(usize, String)>,
    /// Hauteur des réglages d'une tranche (égaliseur, panoramique), mesurée à l'affichage.
    pub(crate) hauteur_reglages: f32,
    pub arrangement: crate::arrangement::EtatArrangement,
    /// Historique, exports, récents, sauvegardes.
    pub fichiers: crate::fichiers::EtatFichiers,
    /// Banque General MIDI (gardée pour les exports WAV).
    banque: Option<Arc<SoundFont>>,
}

impl Studio {
    /// Démarre le moteur (sortie audio, ou horloge silencieuse sans carte son) avec la banque.
    pub fn new(banque: Option<Arc<SoundFont>>) -> Self {
        let (moteur, message) = match banque.clone().map(Moteur::demarrer) {
            Some(Ok(m)) => {
                let message = m.sortie().is_none().then(|| "Aucune sortie audio détectée : branchez un casque ou des enceintes, puis relancez song-audio.".to_string());
                (Some(m), message)
            }
            Some(Err(e)) => (None, Some(format!("Le son n'a pas pu démarrer : {e}"))),
            None => (None, Some("Banque de sons introuvable (banques/GeneralUser-GS.sf2).".to_string())),
        };
        let projet = projet::demo::creer_demo();
        let canal_selectionne = projet.canaux.first().map(|c| c.id.clone());
        Studio {
            projet,
            mode: ModeLecture::Pattern,
            metronome: false,
            chemin: None,
            modifie: false,
            message,
            canal_selectionne,
            moteur,
            envoye: None,
            dock: Some(coquille::disposition_par_defaut()),
            navigateur: Navigateur::new(catalogue_des_morceaux()),
            peinture: None,
            colonnes_pas: Vec::new(),
            renommage: None,
            jeu: Default::default(),
            piano_roll: Default::default(),
            instrument: Default::default(),
            mesures: Default::default(),
            niveaux: Vec::new(),
            renommage_piste: None,
            hauteur_reglages: 70.0,
            arrangement: Default::default(),
            fichiers: Default::default(),
            banque,
        }
    }

    // ---------------------------------------------------------------- morceaux et fichiers

    /// Remplace le projet par un fichier MIDI ; faux (et un message) s'il est illisible ou vide.
    pub fn importer_midi(&mut self, octets: &[u8], nom: &str, jouer: bool) -> bool {
        match projet::import_midi::depuis_midi(octets, nom) {
            Ok(p) if p.canaux.is_empty() => {
                self.message = Some(format!("« {nom} » ne contient aucune note."));
                false
            }
            Ok(p) => {
                self.remplacer_projet(p, None);
                self.modifie = true; // pas encore enregistré comme projet
                self.message = None;
                if jouer {
                    self.basculer_lecture();
                }
                true
            }
            Err(e) => {
                self.message = Some(format!("Fichier MIDI illisible : {nom} ({e})"));
                false
            }
        }
    }

    /// Charge un morceau fourni et le joue.
    pub fn charger_morceau(&mut self, morceau: &Morceau) {
        match chemin_ressource(&morceau.fichier).and_then(|c| std::fs::read(c).ok()) {
            Some(octets) => {
                self.importer_midi(&octets, &morceau.titre, true);
            }
            None => self.message = Some(format!("Impossible de charger « {} » (fichier introuvable ou illisible).", morceau.titre)),
        }
    }

    /// Ouvre un fichier du disque : un projet (.json) ou un fichier MIDI (importé sans jouer).
    pub fn ouvrir_fichier(&mut self, chemin: &Path) {
        let nom = chemin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if nom.to_lowercase().ends_with(".json") {
            self.ouvrir(chemin);
            return;
        }
        let titre = nom.trim_end_matches(".midi").trim_end_matches(".MIDI").trim_end_matches(".mid").trim_end_matches(".MID").to_string();
        match std::fs::read(chemin) {
            Ok(octets) => {
                self.importer_midi(&octets, &titre, false);
            }
            Err(e) => self.message = Some(format!("Fichier illisible : {nom} ({e})")),
        }
    }

    /// Reprend la disposition des panneaux de la dernière session, si elle est lisible.
    pub fn reprendre_disposition(&mut self, stockage: Option<&dyn eframe::Storage>) {
        if let Some(dock) = stockage.and_then(|s| eframe::get_value::<DockState<Panneau>>(s, CLE_DISPOSITION)) {
            if !coquille::ouverts(&dock).is_empty() {
                self.dock = Some(dock);
            }
        }
    }

    pub fn panneaux_ouverts(&self) -> Vec<Panneau> {
        self.dock.as_ref().map(coquille::ouverts).unwrap_or_default()
    }

    pub fn basculer_panneau(&mut self, panneau: Panneau) {
        if let Some(dock) = &mut self.dock {
            coquille::basculer(dock, panneau);
        }
    }

    /// Ouvre un panneau et l'amène au premier plan.
    pub fn montrer_panneau(&mut self, panneau: Panneau) {
        if let Some(dock) = &mut self.dock {
            coquille::montrer(dock, panneau);
        }
    }

    pub fn disposition_par_defaut(&mut self) {
        self.dock = Some(coquille::disposition_par_defaut());
    }

    pub fn depuis_disque() -> Self {
        let banque = chemin_banque().and_then(|c| moteur::charger_banque(&c).ok());
        Self::new(banque)
    }

    pub fn banque(&self) -> Option<Arc<SoundFont>> {
        self.banque.clone()
    }

    pub fn moteur(&self) -> Option<&Moteur> {
        self.moteur.as_ref()
    }

    pub(crate) fn moteur_mut(&mut self) -> Option<&mut Moteur> {
        self.moteur.as_mut()
    }

    // ---------------------------------------------------------------- transport

    pub fn etat_transport(&self) -> EtatTransport {
        self.moteur.as_ref().map(|m| m.lecture().etat).unwrap_or(EtatTransport::Arret)
    }

    /// Position dans la boucle, en tics, prolongée depuis le dernier bloc rendu.
    pub fn position(&self) -> f64 {
        let Some(m) = &self.moteur else { return 0.0 };
        let l = m.lecture();
        if l.etat != EtatTransport::Lecture {
            return l.position;
        }
        let ecoule = m.horloge().saturating_sub(l.horloge) as f64 / m.frequence() as f64;
        (l.position + ecoule * l.tempo * TICS_PAR_NOIRE / 60.0) % l.longueur.max(1) as f64
    }

    pub fn basculer_lecture(&mut self) {
        self.synchroniser();
        let action = if self.etat_transport() == EtatTransport::Lecture { ActionTransport::Pause } else { ActionTransport::Jouer { decompte: false } };
        if let Some(m) = &mut self.moteur {
            m.transport(action);
        }
    }

    pub fn arreter(&mut self) {
        if let Some(m) = &mut self.moteur {
            m.transport(ActionTransport::Arreter);
        }
    }

    pub fn changer_mode(&mut self, mode: ModeLecture) {
        if mode != self.mode {
            self.arreter();
            self.mode = mode;
        }
    }

    pub fn basculer_metronome(&mut self) {
        self.metronome = !self.metronome;
        if let Some(m) = &mut self.moteur {
            m.transport(ActionTransport::Metronome(self.metronome));
        }
    }

    /// Envoie au moteur ce qui a changé dans le projet (réglages et notes), même en pleine lecture.
    pub fn synchroniser(&mut self) {
        let a_jour = matches!(&self.envoye, Some((p, mode)) if *p == self.projet && *mode == self.mode);
        if a_jour {
            return;
        }
        // canaux supprimés ou déplacés : leurs rangs dans le moteur changent, on coupe ce qui
        // sonne (une note tenue resterait sur un rang qui n'est plus le sien)
        let ordre_change = self.envoye.as_ref().is_some_and(|(p, _)| p.canaux.len() > self.projet.canaux.len() || !self.projet.canaux.iter().zip(&p.canaux).all(|(a, b)| a.id == b.id));
        if let Some(m) = &mut self.moteur {
            if ordre_change {
                m.tout_couper();
            }
            if let Err(e) = m.configurer(pont::configuration(&self.projet)) {
                self.message = Some(format!("Le moteur n'a pas pu ajouter d'instrument : {e}"));
            }
            m.definir_session(pont::session(&self.projet, self.mode));
        }
        self.envoye = Some((self.projet.clone(), self.mode));
    }

    // ---------------------------------------------------------------- projet

    /// Applique une modification au projet (marque le projet comme modifié).
    pub fn modifier(&mut self, f: impl FnOnce(&mut Projet)) {
        f(&mut self.projet);
        self.modifie = true;
    }

    pub fn remplacer_projet(&mut self, projet: Projet, chemin: Option<PathBuf>) {
        self.relacher_tout();
        self.arreter();
        self.canal_selectionne = projet.canaux.first().map(|c| c.id.clone());
        self.projet = projet;
        self.chemin = chemin;
        self.modifie = false;
        self.historique_neuf();
    }

    pub fn ajouter_canal(&mut self, options: OptionsCanal) {
        // aucune limite : le moteur fait grandir son graphe à la demande
        let id = self.projet.ajouter_canal(options);
        self.modifie = true;
        self.canal_selectionne = Some(id);
    }

    pub fn ouvrir(&mut self, chemin: &Path) {
        match std::fs::read_to_string(chemin).map_err(|e| e.to_string()).and_then(|t| deserialiser(&t).map_err(|e| e.0)) {
            Ok(p) => {
                self.remplacer_projet(p, Some(chemin.to_path_buf()));
                self.message = None;
                self.noter_recent(chemin);
            }
            Err(e) => self.message = Some(format!("Projet illisible : {e}")),
        }
    }

    /// Écrit le projet dans un fichier temporaire puis le renomme : un plantage en pleine
    /// écriture ne laisse jamais un projet à moitié écrit.
    pub fn enregistrer_dans(&mut self, chemin: &Path) -> Result<(), String> {
        let temporaire = chemin.with_extension("json.tmp");
        std::fs::write(&temporaire, serialiser(&self.projet)).map_err(|e| e.to_string())?;
        std::fs::rename(&temporaire, chemin).map_err(|e| e.to_string())?;
        self.chemin = Some(chemin.to_path_buf());
        self.modifie = false;
        self.noter_recent(chemin);
        self.supprimer_reprise();
        Ok(())
    }

    /// Titre de la fenêtre : nom du projet, et un point s'il n'est pas enregistré.
    pub fn titre(&self) -> String {
        let nom = self
            .chemin
            .as_ref()
            .and_then(|c| c.file_name())
            .map(|n| n.to_string_lossy().trim_end_matches(".json").trim_end_matches(".song-audio").to_string())
            .unwrap_or_else(|| self.projet.nom.clone());
        format!("{}{nom} — song-audio", if self.modifie { "• " } else { "" })
    }

    // ---------------------------------------------------------------- interface

    fn raccourcis(&mut self, ctx: &egui::Context) {
        // hors de la fermeture : interroger le contexte pendant `input` reprendrait son verrou
        let saisie = ctx.text_edit_focused();
        let (annuler, retablir) = ctx.input(|i| {
            let z = i.modifiers.command && i.key_pressed(egui::Key::Z);
            (!saisie && z && !i.modifiers.shift, !saisie && ((z && i.modifiers.shift) || (i.modifiers.command && i.key_pressed(egui::Key::Y))))
        });
        if annuler {
            self.annuler();
        }
        if retablir {
            self.retablir();
        }
        let (espace, maj, ctrl_s, ctrl_maj_s, ctrl_o) = ctx.input(|i| {
            (
                !saisie && i.key_pressed(egui::Key::Space) && !i.modifiers.shift,
                !saisie && i.key_pressed(egui::Key::Space) && i.modifiers.shift,
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.key_pressed(egui::Key::O),
            )
        });
        if espace {
            self.basculer_lecture();
        }
        if maj {
            self.arreter();
        }
        if ctrl_s || ctrl_maj_s {
            self.enregistrer(ctrl_maj_s);
        }
        if ctrl_o {
            self.demander(crate::fichiers::Action::ChoisirEtOuvrir);
        }
        for panneau in Panneau::TOUS {
            if let Some(touche) = panneau.touche() {
                if ctx.input(|i| i.key_pressed(touche)) {
                    self.basculer_panneau(panneau);
                }
            }
        }
    }

    fn barre_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // le nom en un seul bloc : « song » puis « -audio » en couleur, sans espace entre eux
            let mut nom = egui::text::LayoutJob::default();
            let police = egui::TextStyle::Button.resolve(ui.style());
            nom.append("song", 0.0, egui::TextFormat::simple(police.clone(), theme::TEXTE));
            nom.append("-audio", 0.0, egui::TextFormat::simple(police, theme::ACCENT));
            ui.label(nom);
            ui.separator();
            ui.menu_button("Fichier", |ui| self.menu_fichier(ui));
            ui.menu_button("Édition", |ui| self.menu_edition(ui));
            ui.separator();
            let lecture = self.etat_transport() == EtatTransport::Lecture;
            let bouton_lecture = egui::Button::new(RichText::new(if lecture { "Pause" } else { "Lecture" }).strong().color(Color32::BLACK))
                .fill(if lecture { theme::ACTIF } else { theme::ACCENT })
                .min_size(Vec2::new(72.0, 24.0));
            if ui.add(bouton_lecture).on_hover_text("Lecture / pause (Espace)").clicked() {
                self.basculer_lecture();
            }
            if ui.add(egui::Button::new("Stop").min_size(Vec2::new(44.0, 24.0))).on_hover_text("Arrêt (Maj+Espace)").clicked() {
                self.arreter();
            }
            if ui.selectable_label(self.metronome, "Métro").on_hover_text("Métronome : un clic à chaque temps").clicked() {
                self.basculer_metronome();
            }
            let enr = RichText::new("Enr.").color(if self.jeu.enregistrement { theme::ENREGISTREMENT } else { theme::TEXTE });
            if ui
                .selectable_label(self.jeu.enregistrement, enr)
                .on_hover_text("Enregistrement : les notes jouées pendant la lecture s'inscrivent dans le pattern")
                .clicked()
            {
                self.basculer_enregistrement();
            }
            ui.separator();
            for (mode, texte, aide) in [(ModeLecture::Pattern, "PAT", "Le pattern actif en boucle"), (ModeLecture::Morceau, "SONG", "Tout l'arrangement")] {
                if ui.selectable_label(self.mode == mode, texte).on_hover_text(aide).clicked() {
                    self.changer_mode(mode);
                }
            }
            ui.separator();
            let mut tempo = self.projet.tempo;
            let reponse = ui.add(egui::DragValue::new(&mut tempo).range(20.0..=300.0).speed(0.5).suffix(" BPM").max_decimals(1));
            if reponse.changed() {
                self.modifier(|p| p.tempo = tempo);
            }
            let position = self.position();
            afficheur(ui, &formater_position(position, self.projet.signature), "Position : mesure:temps:tic");
            afficheur(ui, &formater_secondes(tics_en_secondes(position, self.projet.tempo)), "Temps écoulé dans la boucle");
        });
    }

    /// Le panneau Navigateur : morceaux, fichiers, instruments.
    pub fn navigateur(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.morceaux(ui);
            ui.add_space(8.0);
            self.fichiers(ui);
            ui.add_space(8.0);
            self.instruments(ui);
        });
    }

    fn morceaux(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("MORCEAUX").small().strong().color(theme::DISCRET));
        if ui.button("+ Nouveau projet").on_hover_text("Projet vide avec un kit de batterie").clicked() {
            self.demander(crate::fichiers::Action::Modele(projet::modeles::Modele::Vide));
        }
        if ui.button("⏵ Démo (4 instruments)").clicked() {
            self.demander(crate::fichiers::Action::Modele(projet::modeles::Modele::Demo));
        }
        for m in self.navigateur.morceaux.clone() {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(format!("⏵ {}", m.titre)).frame(false)).on_hover_text(format!("{} — {} ({})\n{}", m.compositeur, m.oeuvre, m.annee, m.credit)).clicked() {
                    self.demander(crate::fichiers::Action::Morceau(m.clone()));
                }
                ui.label(RichText::new(nom_court(&m.compositeur)).small().color(theme::DISCRET));
            });
        }
    }

    fn fichiers(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("FICHIERS").small().strong().color(theme::DISCRET));
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("Ouvrir un fichier…").on_hover_text("Un projet (.json) ou un fichier MIDI (.mid)").clicked() {
                if let Some(c) = rfd::FileDialog::new().add_filter("Projet ou MIDI", &["json", "mid", "midi"]).pick_file() {
                    self.demander(crate::fichiers::Action::Fichier(c));
                }
            }
            if ui.small_button("Dossier…").on_hover_text("Choisir le dossier affiché").clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    self.navigateur.changer_dossier(d);
                }
            }
            if ui.small_button("Actualiser").clicked() {
                self.navigateur.actualiser();
            }
        });
        ui.label(RichText::new("ou glissez un fichier sur la fenêtre").small().color(theme::DISCRET));
        let racine = self.navigateur.dossier.clone();
        let titre = racine.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| racine.display().to_string());
        egui::CollapsingHeader::new(titre).id_salt(&racine).default_open(false).show(ui, |ui| self.dossier(ui, &racine, 0));
    }

    /// Un dossier de la liste des fichiers (lu seulement quand on l'ouvre).
    fn dossier(&mut self, ui: &mut egui::Ui, dossier: &Path, profondeur: usize) {
        let entrees = self.navigateur.contenu(dossier);
        if entrees.is_empty() {
            ui.label(RichText::new("(aucun fichier MIDI ni projet)").small().color(theme::DISCRET));
        }
        for e in entrees {
            if e.dossier {
                if profondeur < 12 {
                    egui::CollapsingHeader::new(&e.nom).id_salt(&e.chemin).default_open(false).show(ui, |ui| self.dossier(ui, &e.chemin, profondeur + 1));
                }
            } else if ui.add(egui::Button::new(&e.nom).frame(false)).on_hover_text(e.chemin.display().to_string()).clicked() {
                self.demander(crate::fichiers::Action::Fichier(e.chemin.clone()));
            }
        }
    }

    pub fn instruments(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("INSTRUMENTS").small().strong().color(theme::DISCRET));
        if ui.button("+ Batterie").on_hover_text("Une ligne de batterie, en pas à pas").clicked() {
            self.ajouter_canal(OptionsCanal { batterie: true, ..Default::default() });
        }
        egui::CollapsingHeader::new(RichText::new("Synthés intégrés").color(theme::ACCENT)).default_open(true).show(ui, |ui| {
            for (i, p) in PREREGLAGES_SOUSTRACTIF.iter().enumerate() {
                if ui.button(format!("+ {}", p.nom)).clicked() {
                    self.ajouter_canal(OptionsCanal { source: SourceCanal::Soustractif, programme: i as u8, ..Default::default() });
                }
            }
        });
        for (f, famille) in FAMILLES.iter().enumerate() {
            egui::CollapsingHeader::new(*famille).show(ui, |ui| {
                for programme in (f * 8) as u8..(f * 8 + 8) as u8 {
                    if ui.button(format!("+ {}", nom_instrument(programme))).clicked() {
                        self.ajouter_canal(OptionsCanal { programme, ..Default::default() });
                    }
                }
            });
        }
    }
}

pub(crate) fn famille_ou_synthe(canal: &projet::Canal) -> String {
    match canal.source {
        SourceCanal::Soustractif => "Synthé intégré".into(),
        SourceCanal::Soundfont => format!("{} ({})", nom_instrument(canal.programme), famille_instrument(canal.programme)),
    }
}

pub(crate) fn couleur(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    let octet = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgb(octet(0), octet(2), octet(4))
}

fn afficheur(ui: &mut egui::Ui, texte: &str, aide: &str) {
    egui::Frame::new().fill(theme::ECRAN).corner_radius(3).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
        ui.label(RichText::new(texte).monospace().color(theme::ACTIF));
    })
    .response
    .on_hover_text(aide);
}

impl eframe::App for Studio {
    fn on_exit(&mut self) {
        // fermeture normale : le travail est enregistré, ou l'utilisateur a choisi de ne pas le
        // faire ; la reprise ne sert plus
        if !self.modifie || self.fichiers.fermeture_permise {
            self.supprimer_reprise();
        }
    }

    fn save(&mut self, stockage: &mut dyn eframe::Storage) {
        if let Some(dock) = &self.dock {
            eframe::set_value(stockage, CLE_DISPOSITION, dock);
        }
        eframe::set_value(stockage, crate::fichiers::CLE_RECENTS, &self.fichiers.recents);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.raccourcis(&ctx);
        // ← → : octave du clavier, sauf quand le piano roll a le focus (il déplace ses notes)
        let fleches_libres = !self.piano_roll.a_le_focus(&ctx);
        self.lire_clavier(&ctx, fleches_libres);
        let secondes = ctx.input(|i| i.stable_dt).min(0.1);
        self.lire_mesures(secondes);
        // fichiers glissés sur la fenêtre : projet ou MIDI
        let deposes: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|c| c.is_absolute()).collect());
        if let Some(chemin) = deposes.into_iter().next() {
            self.demander(crate::fichiers::Action::Fichier(chemin));
        }
        egui::Panel::top("transport").show(ui, |ui| {
            ui.add_space(4.0);
            self.barre_transport(ui);
            self.bandeau_reprise(ui);
            ui.add_space(4.0);
        });
        let etat = match self.etat_transport() {
            EtatTransport::Lecture => "Lecture",
            EtatTransport::Pause => "Pause",
            EtatTransport::Arret => "Prêt",
        };
        egui::Panel::bottom("etat").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(m) = &self.message {
                    ui.label(RichText::new(m).color(theme::ACCENT));
                } else {
                    ui.label(RichText::new("Espace : lecture/pause · Maj+Espace : arrêt · Ctrl+S : enregistrer").color(theme::DISCRET));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(etat);
                    if let Some(m) = &self.moteur {
                        ui.label(RichText::new(m.sortie().unwrap_or("sans sortie audio")).color(theme::DISCRET));
                    }
                    ui.separator();
                    if ui.small_button("Disposition par défaut").clicked() {
                        self.disposition_par_defaut();
                    }
                    let ouverts = self.panneaux_ouverts();
                    for panneau in Panneau::TOUS.iter().rev() {
                        let aide = match panneau.touche() {
                            Some(t) => format!("{} ({t:?})", panneau.titre()),
                            None => panneau.titre().to_string(),
                        };
                        if ui.selectable_label(ouverts.contains(panneau), RichText::new(panneau.titre()).small()).on_hover_text(aide).clicked() {
                            self.basculer_panneau(*panneau);
                        }
                    }
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::FOND)).show(ui, |ui| {
            // la disposition sort de l'état le temps que les onglets se dessinent avec lui
            if let Some(mut dock) = self.dock.take() {
                let style = egui_dock::Style::from_egui(ui.style());
                DockArea::new(&mut dock).style(style).show_inside(ui, &mut Vue { studio: self });
                self.dock = Some(dock);
            }
        });
        self.fiche_du_projet(&ctx);
        self.relever_export();
        // fermer la fenêtre avec des modifications : on demande d'abord
        if ctx.input(|i| i.viewport().close_requested()) && self.modifie && !self.fichiers.fermeture_permise {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.fichiers.en_attente = Some(crate::fichiers::Action::Fermer);
        }
        if self.confirmation(&ctx) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.sauvegarder(std::time::Instant::now());
        let geste = ctx.input(|i| i.pointer.any_down()) || ctx.text_edit_focused();
        self.consigner_historique(geste);
        // le son suit l'interface, à chaque image
        self.synchroniser();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.titre()));
        if self.etat_transport() == EtatTransport::Lecture {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

pub fn appliquer_theme(ctx: &egui::Context) {
    theme::appliquer(ctx);
}
