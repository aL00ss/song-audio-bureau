//! Piano roll du canal choisi, dessiné par la carte graphique (seul ce qui est visible est
//! dessiné : des milliers de notes restent fluides). Mêmes gestes que la version web : clic dans
//! le vide pour poser une note (aussitôt déplaçable), glisser une note pour la déplacer, glisser
//! son bord droit pour la redimensionner, clic droit (glissé) pour effacer, Ctrl + glisser pour
//! sélectionner au lasso (Maj pour ajouter), Suppr, Ctrl+A, Échap, flèches (Maj : octave). En bas,
//! le couloir de vélocité ; à gauche, un clavier qui fait entendre les notes.

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType};
use moteur::EtatTransport;
use projet::NoteMusicale;
use projet::clavier::est_noire;
use projet::edition::{
    GRILLES, Selection, aimanter, aimanter_bas, ajouter_note, definir_velocite, deplacer_notes, etendue, note_sous, notes_dans_rectangle, quantifier,
    redimensionner_notes, supprimer_notes,
};
use projet::gm::{nom_note, nom_son_batterie};
use projet::lecture::ModeLecture;
use projet::temps::{TICS_PAR_NOIRE, TICS_PAR_PAS, tics_par_mesure};

use crate::app::{Studio, couleur};
use crate::theme;

pub const H_LIGNE: f32 = 12.0; // px par demi-ton
pub const L_CLAVIER: f32 = 54.0;
pub const H_VELOCITE: f32 = 56.0;
const BORD: f32 = 6.0; // px du bord droit d'une note qui servent à la redimensionner
const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 4.0;

#[derive(Clone, Debug)]
enum Geste {
    Deplacer { tic: f64, hauteur: i32, depart: Vec<NoteMusicale>, indices: Selection },
    Redimensionner { tic: f64, depart: Vec<NoteMusicale>, indices: Selection },
    Lasso { tic: f64, hauteur: i32, tic_fin: f64, hauteur_fin: i32, ajout: Selection },
    Effacer,
    Velocite,
    Ecoute { hauteur: u8 },
}

/// État de l'éditeur : vue (zoom, défilement), sélection, grille, geste en cours.
#[derive(Debug)]
pub struct EtatPianoRoll {
    /// Canal affiché ; un autre canal remet la vue et la sélection à zéro.
    canal: Option<String>,
    /// Pixels par tic.
    pub zoom: f32,
    /// Défilement horizontal (px) et vertical (px depuis la note 127).
    pub x: f32,
    pub y: f32,
    pub selection: Selection,
    /// Pas de l'aimantation, en tics (0 = aucune).
    pub grille: u32,
    /// Durée d'une note posée : celle de la dernière note touchée.
    pub duree_nouvelle: u32,
    geste: Option<Geste>,
    /// Zone de dessin de la dernière image (pour convertir positions et notes).
    pub zone: Rect,
    /// Identifiant du widget (pour savoir s'il a le focus).
    pub id: Option<egui::Id>,
    centrer: bool,
}

impl Default for EtatPianoRoll {
    fn default() -> Self {
        EtatPianoRoll {
            canal: None,
            zoom: 0.6,
            x: 0.0,
            y: (127.0 - 72.0) * H_LIGNE,
            selection: Selection::new(),
            grille: TICS_PAR_PAS,
            duree_nouvelle: TICS_PAR_PAS * 2,
            geste: None,
            zone: Rect::NOTHING,
            id: None,
            centrer: true,
        }
    }
}

impl EtatPianoRoll {
    fn h_bas(&self) -> f32 {
        self.zone.height() - H_VELOCITE
    }
    fn vers_tic(&self, x: f32) -> f64 {
        ((x - L_CLAVIER + self.x) / self.zoom) as f64
    }
    fn vers_hauteur(&self, y: f32) -> i32 {
        127 - ((y + self.y) / H_LIGNE).floor() as i32
    }
    fn x_de(&self, tic: f64) -> f32 {
        L_CLAVIER + tic as f32 * self.zoom - self.x
    }
    fn y_de(&self, hauteur: i32) -> f32 {
        (127 - hauteur) as f32 * H_LIGNE - self.y
    }
    fn borner(&mut self) {
        self.y = self.y.min((128.0 * H_LIGNE - self.h_bas()).max(0.0)).max(0.0);
        self.x = self.x.max(0.0);
    }

    /// Point de l'écran au milieu de la ligne d'une note, à un instant donné.
    pub fn vers_ecran(&self, tic: f64, hauteur: u8) -> Pos2 {
        self.zone.min + Vec2::new(self.x_de(tic), self.y_de(hauteur as i32) + H_LIGNE / 2.0)
    }

    /// Point de l'écran du couloir de vélocité qui donne cette vélocité à une note.
    pub fn vers_ecran_velocite(&self, tic: f64, velocite: u8) -> Pos2 {
        self.zone.min + Vec2::new(self.x_de(tic) + 1.0, self.h_bas() + H_VELOCITE - 2.0 - velocite as f32 / 127.0 * (H_VELOCITE - 8.0))
    }

    pub fn a_le_focus(&self, ctx: &egui::Context) -> bool {
        self.id.is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
    }
}

impl Studio {
    fn notes_du_canal(&self, canal: &str) -> Vec<NoteMusicale> {
        self.projet.pattern_actif().notes.get(canal).cloned().unwrap_or_default()
    }

    /// Remplace les notes d'un canal dans le pattern actif (rien si elles n'ont pas changé).
    pub fn remplacer_notes(&mut self, canal: &str, notes: Vec<NoteMusicale>) {
        if self.notes_du_canal(canal) == notes || self.projet.canal(canal).is_none() {
            return;
        }
        let id = canal.to_string();
        self.modifier(|p| {
            p.pattern_actif_mut().notes.insert(id, notes);
        });
    }

    pub fn piano_roll(&mut self, ui: &mut egui::Ui) {
        let Some(canal) = self.canal_selectionne.as_deref().and_then(|id| self.projet.canal(id)).cloned() else {
            ui.label(RichText::new("Choisissez un instrument dans le rack.").color(theme::DISCRET));
            return;
        };
        if self.piano_roll.canal.as_deref() != Some(canal.id.as_str()) {
            self.piano_roll.canal = Some(canal.id.clone());
            self.piano_roll.selection.clear();
            self.piano_roll.geste = None;
            self.piano_roll.centrer = true;
        }
        let notes = self.notes_du_canal(&canal.id);
        // une sélection qui désigne des notes disparues (modifiées ailleurs) est oubliée
        if self.piano_roll.selection.last().is_some_and(|&i| i >= notes.len()) {
            self.piano_roll.selection.clear();
        }

        // ---------------------------------------------------------------- en-tête
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&canal.nom).strong().color(couleur(&canal.couleur)));
            if canal.batterie {
                ui.label(RichText::new(nom_son_batterie(canal.note_defaut)).color(theme::DISCRET));
            }
            ui.label(RichText::new(format!("{} notes", notes.len())).color(theme::DISCRET));
            let n = self.piano_roll.selection.len();
            if n > 0 {
                ui.label(RichText::new(format!("{n} sélectionnée{}", if n > 1 { "s" } else { "" })).color(theme::DISCRET));
            }
            ui.separator();
            ui.label("Grille");
            let nom_grille = GRILLES.iter().find(|(_, t)| *t == self.piano_roll.grille).map(|(n, _)| *n).unwrap_or("?");
            egui::ComboBox::from_id_salt("grille").selected_text(nom_grille).show_ui(ui, |ui| {
                for (nom, tics) in GRILLES {
                    ui.selectable_value(&mut self.piano_roll.grille, tics, nom);
                }
            });
            if ui.button("Quantifier").on_hover_text("Ramener le début des notes (sélectionnées, sinon toutes) sur la grille").clicked() {
                let q = quantifier(&notes, &self.piano_roll.selection, self.piano_roll.grille);
                self.remplacer_notes(&canal.id, q);
            }
            ui.label(RichText::new("Molette : défiler · Maj : horizontal · Ctrl : zoom · Ctrl+glisser : lasso").small().color(theme::DISCRET));
        });
        let notes = self.notes_du_canal(&canal.id);

        // ---------------------------------------------------------------- zone
        let taille = ui.available_size().max(Vec2::new(200.0, H_VELOCITE + 4.0 * H_LIGNE));
        let (zone, reponse) = ui.allocate_exact_size(taille, Sense::click_and_drag());
        let nom = format!("Piano roll de {}", canal.nom);
        reponse.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &nom));
        self.piano_roll.zone = zone;
        self.piano_roll.id = Some(reponse.id);
        if self.piano_roll.centrer {
            let centre = etendue(&notes).map(|(b, h)| (b as f32 + h as f32) / 2.0).unwrap_or(if canal.batterie { 42.0 } else { 66.0 });
            self.piano_roll.y = (127.0 - centre) * H_LIGNE - (zone.height() - H_VELOCITE).max(0.0) / 2.0;
            self.piano_roll.centrer = false;
        }
        self.piano_roll.borner();

        // ---------------------------------------------------------------- molette
        if reponse.hovered() {
            let (defilement, zoom, souris) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            let v = &mut self.piano_roll;
            if zoom != 1.0 {
                if let Some(souris) = souris {
                    let x = souris.x - zone.left();
                    let tic = v.vers_tic(x) as f32;
                    v.zoom = (v.zoom * zoom).clamp(ZOOM_MIN, ZOOM_MAX);
                    v.x = tic * v.zoom - (x - L_CLAVIER);
                }
            } else {
                v.y -= defilement.y;
                v.x -= defilement.x;
            }
            v.borner();
        }

        // ---------------------------------------------------------------- gestes
        let (pointeur, appui_g, appui_d, enfonce, mods) = ui.input(|i| {
            (i.pointer.interact_pos(), i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.button_pressed(egui::PointerButton::Secondary), i.pointer.any_down(), i.modifiers)
        });
        if let Some(p) = pointeur {
            let rel = p - zone.min;
            if (appui_g || appui_d) && reponse.contains_pointer() {
                reponse.request_focus();
                self.appui_piano_roll(&canal, rel.x, rel.y, appui_d, mods);
            } else if enfonce && self.piano_roll.geste.is_some() {
                self.glisser_piano_roll(&canal, rel.x, rel.y);
            }
        }
        if !enfonce && self.piano_roll.geste.is_some() {
            if let Some(Geste::Ecoute { hauteur }) = self.piano_roll.geste.take() {
                self.relacher_note(hauteur, &canal.id);
            }
        }
        if reponse.has_focus() {
            // les flèches et Échap restent au piano roll (sinon egui déplacerait le focus)
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(reponse.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: true })
            });
            self.clavier_piano_roll(ui, &canal.id);
        }
        // curseur selon ce qui est sous le pointeur
        if let (Some(p), None) = (reponse.hover_pos(), &self.piano_roll.geste) {
            let rel = p - zone.min;
            let v = &self.piano_roll;
            let notes = self.notes_du_canal(&canal.id);
            let i = if rel.y < v.h_bas() && rel.x >= L_CLAVIER { note_sous(&notes, v.vers_tic(rel.x), v.vers_hauteur(rel.y)) } else { None };
            ui.ctx().set_cursor_icon(match i {
                Some(i) if v.x_de((notes[i].tic + notes[i].duree) as f64) - rel.x <= BORD => CursorIcon::ResizeHorizontal,
                Some(_) => CursorIcon::Move,
                None if rel.x < L_CLAVIER => CursorIcon::PointingHand,
                None => CursorIcon::Crosshair,
            });
        }

        self.dessiner_piano_roll(ui, zone, &canal);
    }

    fn appui_piano_roll(&mut self, canal: &projet::Canal, x: f32, y: f32, droit: bool, mods: egui::Modifiers) {
        let v = &self.piano_roll;
        if y >= v.h_bas() {
            self.piano_roll.geste = Some(Geste::Velocite);
            self.regler_velocite(&canal.id, x, y);
            return;
        }
        let hauteur = v.vers_hauteur(y);
        if x < L_CLAVIER {
            if (0..=127).contains(&hauteur) && !droit {
                self.piano_roll.geste = Some(Geste::Ecoute { hauteur: hauteur as u8 });
                self.jouer_note(hauteur as u8, 100, &canal.id);
            }
            return;
        }
        let tic = v.vers_tic(x);
        let notes = self.notes_du_canal(&canal.id);
        let sous = note_sous(&notes, tic, hauteur);
        if droit {
            self.piano_roll.geste = Some(Geste::Effacer);
            if let Some(i) = sous {
                self.remplacer_notes(&canal.id, supprimer_notes(&notes, &Selection::from([i])));
                self.piano_roll.selection.clear();
            }
            return;
        }
        if mods.command || mods.ctrl {
            let ajout = if mods.shift { self.piano_roll.selection.clone() } else { Selection::new() };
            self.piano_roll.geste = Some(Geste::Lasso { tic, hauteur, tic_fin: tic, hauteur_fin: hauteur, ajout });
            return;
        }
        if let Some(i) = sous {
            let mut choix = self.piano_roll.selection.clone();
            if !choix.contains(&i) {
                if !mods.shift {
                    choix.clear();
                }
                choix.insert(i);
            }
            self.piano_roll.selection = choix.clone();
            let n = notes[i];
            let bord_droit = self.piano_roll.x_de((n.tic + n.duree) as f64);
            self.piano_roll.geste = Some(if bord_droit - x <= BORD {
                Geste::Redimensionner { tic, depart: notes.clone(), indices: choix }
            } else {
                Geste::Deplacer { tic, hauteur, depart: notes.clone(), indices: choix }
            });
            self.piano_roll.duree_nouvelle = n.duree;
            self.ecouter(n.hauteur, n.velocite, &canal.id);
            return;
        }
        // dans le vide : une nouvelle note, qu'on peut aussitôt déplacer
        if !(0..=127).contains(&hauteur) {
            return;
        }
        let suivantes = ajouter_note(&notes, aimanter_bas(tic, self.piano_roll.grille), self.piano_roll.duree_nouvelle as i64, hauteur, 100);
        let choix = Selection::from([suivantes.len() - 1]);
        self.remplacer_notes(&canal.id, suivantes.clone());
        self.piano_roll.selection = choix.clone();
        self.piano_roll.geste = Some(Geste::Deplacer { tic, hauteur, depart: suivantes, indices: choix });
        self.ecouter(hauteur as u8, 100, &canal.id);
    }

    fn glisser_piano_roll(&mut self, canal: &projet::Canal, x: f32, y: f32) {
        let tic = self.piano_roll.vers_tic(x);
        let hauteur = self.piano_roll.vers_hauteur(y);
        let grille = self.piano_roll.grille;
        match self.piano_roll.geste.clone() {
            Some(Geste::Deplacer { tic: t0, hauteur: h0, depart, indices }) => {
                let premiere = indices.iter().filter_map(|&i| depart.get(i)).map(|n| n.tic).min().unwrap_or(0) as f64;
                let dt = aimanter(premiere + (tic - t0), grille) - premiere as i64;
                self.remplacer_notes(&canal.id, deplacer_notes(&depart, &indices, dt, hauteur - h0));
            }
            Some(Geste::Redimensionner { tic: t0, depart, indices }) => {
                let dt = if grille > 0 { ((tic - t0) / grille as f64).round() as i64 * grille as i64 } else { (tic - t0).round() as i64 };
                let resultat = redimensionner_notes(&depart, &indices, dt, (grille / 2).max(1));
                if let Some(n) = indices.first().and_then(|&i| resultat.get(i)) {
                    self.piano_roll.duree_nouvelle = n.duree;
                }
                self.remplacer_notes(&canal.id, resultat);
            }
            Some(Geste::Lasso { tic: t0, hauteur: h0, ajout, .. }) => {
                let notes = self.notes_du_canal(&canal.id);
                let mut choix = ajout.clone();
                choix.extend(notes_dans_rectangle(&notes, t0, tic, h0, hauteur));
                self.piano_roll.selection = choix;
                self.piano_roll.geste = Some(Geste::Lasso { tic: t0, hauteur: h0, tic_fin: tic, hauteur_fin: hauteur, ajout });
            }
            Some(Geste::Effacer) => {
                let notes = self.notes_du_canal(&canal.id);
                if let Some(i) = note_sous(&notes, tic, hauteur) {
                    self.remplacer_notes(&canal.id, supprimer_notes(&notes, &Selection::from([i])));
                    self.piano_roll.selection.clear();
                }
            }
            Some(Geste::Velocite) => self.regler_velocite(&canal.id, x, y),
            Some(Geste::Ecoute { hauteur: avant }) => {
                if hauteur != avant as i32 && (0..=127).contains(&hauteur) {
                    self.relacher_note(avant, &canal.id);
                    self.jouer_note(hauteur as u8, 100, &canal.id);
                    self.piano_roll.geste = Some(Geste::Ecoute { hauteur: hauteur as u8 });
                }
            }
            None => {}
        }
    }

    /// Couloir de vélocité : les notes qui commencent près du pointeur prennent la hauteur visée.
    fn regler_velocite(&mut self, canal: &str, x: f32, y: f32) {
        let v = &self.piano_roll;
        let notes = self.notes_du_canal(canal);
        let tic = v.vers_tic(x);
        let rayon = 5.0 / v.zoom as f64;
        let indices: Selection = notes.iter().enumerate().filter(|(_, n)| (n.tic as f64 - tic).abs() <= rayon).map(|(i, _)| i).collect();
        if indices.is_empty() {
            return;
        }
        let velocite = (v.h_bas() + H_VELOCITE - 2.0 - y) as f64 / (H_VELOCITE - 8.0) as f64 * 127.0;
        self.remplacer_notes(canal, definir_velocite(&notes, &indices, velocite));
    }

    fn clavier_piano_roll(&mut self, ui: &egui::Ui, canal: &str) {
        let notes = self.notes_du_canal(canal);
        let choix = self.piano_roll.selection.clone();
        let (suppr, tout, echap, haut, bas, gauche, droite, maj) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.modifiers.command && i.key_pressed(egui::Key::A),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.modifiers.shift,
            )
        });
        if tout {
            self.piano_roll.selection = (0..notes.len()).collect();
            return;
        }
        if choix.is_empty() {
            return;
        }
        if suppr {
            self.remplacer_notes(canal, supprimer_notes(&notes, &choix));
            self.piano_roll.selection.clear();
        } else if echap {
            self.piano_roll.selection.clear();
        } else if haut || bas {
            let pas = if maj { 12 } else { 1 };
            self.remplacer_notes(canal, deplacer_notes(&notes, &choix, 0, if haut { pas } else { -pas }));
        } else if gauche || droite {
            let pas = self.piano_roll.grille.max(1) as i64;
            self.remplacer_notes(canal, deplacer_notes(&notes, &choix, if droite { pas } else { -pas }, 0));
        }
    }

    fn dessiner_piano_roll(&self, ui: &egui::Ui, zone: Rect, canal: &projet::Canal) {
        let v = &self.piano_roll;
        let peintre = ui.painter_at(zone);
        let o = zone.min;
        let bas = v.h_bas();
        let teinte = couleur(&canal.couleur);
        let notes = self.notes_du_canal(&canal.id);
        let pattern = self.projet.pattern_actif();
        let r = |x: f32, y: f32, l: f32, h: f32| Rect::from_min_size(o + Vec2::new(x, y), Vec2::new(l, h));

        // fond : lignes de notes (touches noires plus sombres)
        peintre.rect_filled(zone, 0.0, Color32::from_rgb(0x20, 0x25, 0x2b));
        let h_max = v.vers_hauteur(0.0).min(127);
        let h_min = v.vers_hauteur(bas).max(0);
        for h in h_min..=h_max {
            let y = v.y_de(h);
            let fond = if est_noire(h) { Color32::from_rgb(0x1a, 0x1e, 0x23) } else { Color32::from_rgb(0x26, 0x2c, 0x33) };
            peintre.rect_filled(r(L_CLAVIER, y, zone.width(), H_LIGNE), 0.0, fond);
            let trait_ = if h % 12 == 0 { Color32::from_rgb(0x3a, 0x42, 0x4c) } else { Color32::from_rgb(0x1d, 0x21, 0x26) };
            peintre.rect_filled(r(L_CLAVIER, y + H_LIGNE - 1.0, zone.width(), 1.0), 0.0, trait_);
        }
        // grille verticale : pas, temps, mesures (les pas s'effacent quand ils sont trop serrés)
        let mesure = tics_par_mesure(self.projet.signature);
        let pas = if TICS_PAR_PAS as f32 * v.zoom >= 4.0 { TICS_PAR_PAS } else if TICS_PAR_NOIRE as f32 * v.zoom >= 4.0 { TICS_PAR_NOIRE } else { mesure };
        let debut = ((v.vers_tic(L_CLAVIER).max(0.0) as u32) / pas) * pas;
        let fin = v.vers_tic(zone.width()).max(0.0) as u32;
        let mut t = debut;
        while t <= fin {
            let x = v.x_de(t as f64).round();
            let c = if t % mesure == 0 {
                Color32::from_rgb(0x56, 0x60, 0x6c)
            } else if t % TICS_PAR_NOIRE == 0 {
                Color32::from_rgb(0x35, 0x3d, 0x46)
            } else {
                Color32::from_rgb(0x2a, 0x30, 0x38)
            };
            peintre.rect_filled(r(x, 0.0, 1.0, bas), 0.0, c);
            t += pas;
        }
        // après la fin du pattern : assombri
        let x_fin = v.x_de(pattern.longueur as f64);
        if x_fin < zone.width() {
            peintre.rect_filled(r(x_fin.max(L_CLAVIER), 0.0, zone.width(), bas), 0.0, Color32::from_black_alpha(90));
        }
        // lasso
        if let Some(Geste::Lasso { tic, hauteur, tic_fin, hauteur_fin, .. }) = &v.geste {
            let (x1, x2) = (v.x_de(tic.min(*tic_fin)), v.x_de(tic.max(*tic_fin)));
            let (y1, y2) = (v.y_de(*hauteur.max(hauteur_fin)), v.y_de(*hauteur.min(hauteur_fin)) + H_LIGNE);
            let cadre = r(x1, y1, x2 - x1, y2 - y1);
            peintre.rect_filled(cadre, 0.0, theme::ACCENT.gamma_multiply(0.12));
            peintre.rect_stroke(cadre, 0.0, Stroke::new(1.0, theme::ACCENT), StrokeKind::Inside);
        }
        // notes (seules les visibles)
        for (i, n) in notes.iter().enumerate() {
            let x = v.x_de(n.tic as f64);
            let l = (n.duree as f32 * v.zoom).max(2.0);
            let y = v.y_de(n.hauteur as i32);
            if x + l < L_CLAVIER || x > zone.width() || y + H_LIGNE < 0.0 || y > bas {
                continue;
            }
            let rect = r(x + 0.5, y + 1.0, l - 1.0, H_LIGNE - 2.0);
            peintre.rect_filled(rect, 1.0, teinte.gamma_multiply(0.55 + n.velocite as f32 / 127.0 * 0.45));
            let choisie = v.selection.contains(&i);
            let contour = if choisie { Stroke::new(2.0, Color32::WHITE) } else { Stroke::new(1.0, Color32::from_black_alpha(150)) };
            peintre.rect_stroke(rect, 1.0, contour, StrokeKind::Inside);
        }
        // tête de lecture (en mode pattern : la boucle est ce pattern)
        if self.etat_transport() == EtatTransport::Lecture && self.mode == ModeLecture::Pattern {
            let x = v.x_de(self.position()).round();
            peintre.rect_filled(r(x, 0.0, 2.0, bas), 0.0, theme::ACTIF);
        }
        // clavier vertical
        let actives = self.notes_en_direct(&canal.id);
        for h in h_min..=h_max {
            let y = v.y_de(h);
            let noire = est_noire(h);
            let fond = if actives.contains(&(h as u8)) {
                teinte
            } else if noire {
                Color32::from_rgb(0x15, 0x17, 0x1b)
            } else {
                Color32::from_rgb(0xe9, 0xe5, 0xdc)
            };
            peintre.rect_filled(r(0.0, y, if noire { L_CLAVIER * 0.62 } else { L_CLAVIER }, H_LIGNE), 0.0, fond);
            peintre.rect_filled(r(0.0, y + H_LIGNE - 1.0, L_CLAVIER, 1.0), 0.0, Color32::from_rgb(0x8b, 0x85, 0x77));
            if h % 12 == 0 {
                peintre.text(o + Vec2::new(L_CLAVIER - 3.0, y + H_LIGNE - 1.0), Align2::RIGHT_BOTTOM, nom_note(h as u8), FontId::proportional(9.0), Color32::from_rgb(0x33, 0x33, 0x33));
            }
        }
        peintre.rect_filled(r(L_CLAVIER - 1.0, 0.0, 1.0, bas), 0.0, Color32::from_rgb(0x11, 0x15, 0x1a));
        // couloir de vélocité
        peintre.rect_filled(r(0.0, bas, zone.width(), H_VELOCITE), 0.0, Color32::from_rgb(0x17, 0x1b, 0x20));
        peintre.rect_filled(r(0.0, bas, zone.width(), 1.0), 0.0, Color32::from_rgb(0x11, 0x15, 0x1a));
        peintre.text(o + Vec2::new(6.0, bas + 4.0), Align2::LEFT_TOP, "Vélocité", FontId::proportional(10.0), theme::DISCRET);
        for (i, n) in notes.iter().enumerate() {
            let x = v.x_de(n.tic as f64);
            if x < L_CLAVIER || x > zone.width() {
                continue;
            }
            let h = n.velocite as f32 / 127.0 * (H_VELOCITE - 8.0);
            let c = if v.selection.contains(&i) { Color32::WHITE } else { teinte };
            peintre.rect_filled(r(x.round(), bas + H_VELOCITE - h - 2.0, 3.0, h), 0.0, c);
        }
        if ui.memory(|m| m.has_focus(v.id.unwrap_or(egui::Id::NULL))) {
            peintre.rect_stroke(zone, 0.0, Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
        }
    }
}
