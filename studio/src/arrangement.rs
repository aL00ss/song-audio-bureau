//! L'arrangement : à gauche les patterns (choisir, double clic pour renommer, menu pour
//! dupliquer ou supprimer, « + Nouveau ») ; à droite une règle des mesures (clic : jouer le
//! morceau depuis cette mesure) et des pistes où l'on pose des clips du pattern actif (clic),
//! qu'on déplace (glisser) ou supprime (clic droit, glissé ou non). Les clips s'aimantent à la
//! mesure ; double clic sur un clip : choisir son pattern. Pistes sans limite : seul ce qui est
//! visible est dessiné. Molette : défiler (Maj : horizontal, Ctrl : zoom).

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType};
use moteur::{ActionTransport, EtatTransport};
use projet::lecture::ModeLecture;
use projet::playlist::{
    ajouter_clip, choisir_pattern, clip_sous, deplacer_clip, dupliquer_pattern, nombre_de_pistes, nouveau_pattern, renommer_pattern, supprimer_clip,
    supprimer_pattern,
};
use projet::temps::tics_par_mesure;
use projet::{Pattern, Projet};

use crate::app::{Studio, couleur};
use crate::theme;

pub const H_PISTE: f32 = 34.0;
pub const L_ENTETE: f32 = 64.0;
pub const H_REGLE: f32 = 20.0;
const L_PATTERNS: f32 = 170.0;
const COULEURS: [&str; 8] = ["#5aa9f5", "#f5a524", "#4cc38a", "#e5679a", "#a78bfa", "#f97362", "#2dd4bf", "#facc15"];

/// Couleur d'un pattern : d'après sa place dans la liste.
pub fn couleur_pattern(projet: &Projet, id: &str) -> Color32 {
    let i = projet.patterns.iter().position(|p| p.id == id).unwrap_or(0);
    couleur(COULEURS[i % COULEURS.len()])
}

#[derive(Clone, Debug)]
enum Geste {
    /// Clip saisi : décalage du pointeur dans le clip, en tics et en pistes.
    Deplacer { id: String, decalage_tic: f64 },
    Effacer,
}

#[derive(Debug)]
pub struct EtatArrangement {
    pub px_par_mesure: f32,
    /// Défilement, en pixels.
    pub x: f32,
    pub y: f32,
    geste: Option<Geste>,
    /// Zone des pistes (sans la règle ni les noms) à la dernière image.
    pub zone: Rect,
    /// Pattern en cours de renommage, et le texte saisi.
    renommage: Option<(String, String)>,
}

impl Default for EtatArrangement {
    fn default() -> Self {
        EtatArrangement { px_par_mesure: 64.0, x: 0.0, y: 0.0, geste: None, zone: Rect::NOTHING, renommage: None }
    }
}

impl EtatArrangement {
    fn px_par_tic(&self, mesure: u32) -> f32 {
        self.px_par_mesure / mesure as f32
    }

    /// Point de l'écran d'une piste à un instant (milieu de la hauteur de la piste).
    pub fn vers_ecran(&self, piste: u32, tic: f64, mesure: u32) -> Pos2 {
        Pos2::new(self.zone.left() + tic as f32 * self.px_par_tic(mesure) - self.x, self.zone.top() + (piste as f32 + 0.5) * H_PISTE - self.y)
    }

    fn vers_tic_piste(&self, p: Pos2, mesure: u32) -> (f64, i64) {
        let tic = ((p.x - self.zone.left() + self.x) / self.px_par_tic(mesure)) as f64;
        let piste = ((p.y - self.zone.top() + self.y) / H_PISTE).floor() as i64;
        (tic, piste)
    }
}

impl Studio {
    /// Passe en mode morceau et place la lecture à ce tic.
    pub fn jouer_depuis(&mut self, tic: f64) {
        self.changer_mode(ModeLecture::Morceau);
        self.synchroniser();
        if let Some(m) = self.moteur_mut() {
            m.transport(ActionTransport::Chercher(tic));
        }
    }

    pub fn arrangement(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(L_PATTERNS, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_size(Vec2::new(L_PATTERNS, ui.available_height()));
                egui::ScrollArea::vertical().id_salt("patterns").auto_shrink([false, false]).show(ui, |ui| self.liste_patterns(ui));
            });
            ui.separator();
            ui.vertical(|ui| self.pistes_arrangement(ui));
        });
    }

    fn liste_patterns(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("PATTERNS").small().color(theme::DISCRET));
            if ui.small_button("+ Nouveau").on_hover_text("Nouveau pattern vide, qui devient le pattern actif").clicked() {
                self.modifier(|p| {
                    nouveau_pattern(p);
                });
            }
        });
        let patterns: Vec<(String, String)> = self.projet.patterns.iter().map(|p| (p.id.clone(), p.nom.clone())).collect();
        let plusieurs = patterns.len() > 1;
        for (id, nom) in patterns {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, couleur_pattern(&self.projet, &id));
                if self.arrangement.renommage.as_ref().is_some_and(|(i, _)| *i == id) {
                    let (i, mut texte) = self.arrangement.renommage.take().expect("renommage");
                    let champ = ui.add(egui::TextEdit::singleline(&mut texte).desired_width(L_PATTERNS - 30.0).id_salt(("nom pattern", &i)));
                    if !champ.has_focus() && !champ.lost_focus() {
                        champ.request_focus();
                    }
                    let (entree, echap) = ui.input(|e| (e.key_pressed(egui::Key::Enter), e.key_pressed(egui::Key::Escape)));
                    if !echap {
                        if entree || champ.lost_focus() {
                            self.modifier(|p| {
                                renommer_pattern(p, &i, &texte);
                            });
                        } else {
                            self.arrangement.renommage = Some((i, texte));
                        }
                    }
                    return;
                }
                let actif = self.projet.pattern_actif == id;
                let bouton = ui
                    .add(egui::Button::selectable(actif, RichText::new(&nom).color(if actif { theme::ACCENT } else { theme::TEXTE })).truncate())
                    .on_hover_text("Clic : choisir · double clic : renommer · clic droit : menu");
                if bouton.clicked() {
                    self.modifier_si(|p| choisir_pattern(p, &id));
                }
                if bouton.double_clicked() || bouton.triple_clicked() {
                    self.arrangement.renommage = Some((id.clone(), nom.clone()));
                }
                bouton.context_menu(|ui| {
                    if ui.button("Renommer…").clicked() {
                        self.arrangement.renommage = Some((id.clone(), nom.clone()));
                        ui.close();
                    }
                    if ui.button("Dupliquer").clicked() {
                        self.modifier(|p| {
                            dupliquer_pattern(p, &id);
                        });
                        ui.close();
                    }
                    if ui.add_enabled(plusieurs, egui::Button::new(RichText::new("Supprimer").color(theme::ACCENT))).clicked() {
                        self.modifier(|p| {
                            supprimer_pattern(p, &id);
                        });
                        ui.close();
                    }
                });
            });
        }
    }

    /// Applique une modification qui dit si elle a changé quelque chose (le projet n'est marqué
    /// modifié que dans ce cas).
    pub fn modifier_si(&mut self, f: impl FnOnce(&mut Projet) -> bool) {
        let mut copie = self.projet.clone();
        if f(&mut copie) {
            self.modifier(|p| *p = copie);
        }
    }

    fn pistes_arrangement(&mut self, ui: &mut egui::Ui) {
        let mesure = tics_par_mesure(self.projet.signature);
        ui.horizontal(|ui| {
            let morceau = self.mode == ModeLecture::Morceau;
            ui.label(RichText::new("Mode").color(theme::DISCRET));
            ui.label(RichText::new(if morceau { "SONG" } else { "PAT" }).strong());
            if !morceau && ui.link("jouer le morceau").clicked() {
                self.changer_mode(ModeLecture::Morceau);
            }
            ui.separator();
            ui.label(RichText::new("Zoom").color(theme::DISCRET));
            if ui.small_button("-").on_hover_text("Zoom arrière").clicked() {
                self.arrangement.px_par_mesure = (self.arrangement.px_par_mesure / 1.5).max(8.0);
            }
            if ui.small_button("+").on_hover_text("Zoom avant").clicked() {
                self.arrangement.px_par_mesure = (self.arrangement.px_par_mesure * 1.5).min(512.0);
            }
            ui.label(RichText::new("Clic : poser le pattern actif · glisser : déplacer · clic droit : supprimer · double clic : choisir").small().color(theme::DISCRET));
        });
        let (tout, reponse) = ui.allocate_exact_size(ui.available_size().max(Vec2::new(200.0, H_REGLE + H_PISTE)), Sense::click_and_drag());
        reponse.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Pistes de l'arrangement"));
        let zone = Rect::from_min_max(tout.min + Vec2::new(L_ENTETE, H_REGLE), tout.max);
        self.arrangement.zone = zone;
        let px_par_tic = self.arrangement.px_par_tic(mesure);

        // molette : défiler, Maj horizontal (egui le fait), Ctrl zoom autour du pointeur
        if reponse.hovered() {
            let (defilement, zoom, souris) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            let a = &mut self.arrangement;
            if zoom != 1.0 {
                if let Some(s) = souris {
                    let tic = (s.x - zone.left() + a.x) / px_par_tic;
                    a.px_par_mesure = (a.px_par_mesure * zoom).clamp(8.0, 512.0);
                    a.x = tic * a.px_par_tic(mesure) - (s.x - zone.left());
                }
            } else {
                a.x -= defilement.x;
                a.y -= defilement.y;
            }
        }
        let pistes = nombre_de_pistes(&self.projet);
        let a = &mut self.arrangement;
        a.x = a.x.max(0.0);
        a.y = a.y.clamp(0.0, (pistes as f32 * H_PISTE - zone.height() + H_PISTE).max(0.0));

        self.gestes_arrangement(ui, &reponse, tout, zone, mesure);
        self.dessiner_arrangement(ui, tout, zone, mesure);
    }

    fn gestes_arrangement(&mut self, ui: &egui::Ui, reponse: &egui::Response, tout: Rect, zone: Rect, mesure: u32) {
        let (pos, appui_g, appui_d, enfonce) = ui.input(|i| {
            (i.pointer.interact_pos(), i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.button_pressed(egui::PointerButton::Secondary), i.pointer.any_down())
        });
        let double = ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary) || i.pointer.button_triple_clicked(egui::PointerButton::Primary));
        let Some(pos) = pos else { return };
        let pistes = nombre_de_pistes(&self.projet) as i64;
        // règle : jouer le morceau depuis la mesure cliquée
        let regle = Rect::from_min_max(Pos2::new(zone.left(), tout.top()), Pos2::new(zone.right(), zone.top()));
        if appui_g && reponse.contains_pointer() && regle.contains(pos) {
            let (tic, _) = self.arrangement.vers_tic_piste(pos, mesure);
            self.jouer_depuis(((tic.max(0.0) as u32) / mesure * mesure) as f64);
            return;
        }
        // double clic sur un clip : choisir son pattern (avant tout autre geste)
        if double && reponse.contains_pointer() && zone.contains(pos) {
            let (tic, piste) = self.arrangement.vers_tic_piste(pos, mesure);
            if let Some(pattern) = clip_sous(&self.projet, piste.max(0) as u32, tic).map(|c| c.pattern.clone()) {
                self.arrangement.geste = None;
                self.modifier_si(|p| choisir_pattern(p, &pattern));
                return;
            }
        }
        if (appui_g || appui_d) && reponse.contains_pointer() && zone.contains(pos) {
            let (tic, piste) = self.arrangement.vers_tic_piste(pos, mesure);
            if piste < 0 || piste >= pistes {
                return;
            }
            let sous = clip_sous(&self.projet, piste as u32, tic).cloned();
            if appui_d {
                self.arrangement.geste = Some(Geste::Effacer);
                if let Some(c) = sous {
                    self.modifier(|p| {
                        supprimer_clip(p, &c.id);
                    });
                }
                return;
            }
            match sous {
                Some(c) => self.arrangement.geste = Some(Geste::Deplacer { decalage_tic: tic - c.debut as f64, id: c.id }),
                None => {
                    // dans le vide : un clip du pattern actif, aimanté à la mesure, aussitôt déplaçable
                    let debut = (tic.max(0.0) as u32) / mesure * mesure;
                    let pattern = self.projet.pattern_actif.clone();
                    let mut id = None;
                    self.modifier(|p| id = ajouter_clip(p, &pattern, piste, debut as i64));
                    if let Some(id) = id {
                        self.arrangement.geste = Some(Geste::Deplacer { id, decalage_tic: tic - debut as f64 });
                    }
                }
            }
            return;
        }
        if !enfonce {
            self.arrangement.geste = None;
            return;
        }
        let (tic, piste) = self.arrangement.vers_tic_piste(pos, mesure);
        match self.arrangement.geste.clone() {
            Some(Geste::Deplacer { id, decalage_tic }) => {
                let debut = (((tic - decalage_tic) / mesure as f64).round() * mesure as f64).max(0.0) as i64;
                let piste = piste.clamp(0, pistes - 1);
                let actuel = self.projet.playlist.iter().find(|c| c.id == id).map(|c| (c.piste as i64, c.debut as i64));
                if actuel.is_some() && actuel != Some((piste, debut)) {
                    self.modifier(|p| {
                        deplacer_clip(p, &id, piste, debut);
                    });
                }
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
            Some(Geste::Effacer) => {
                if piste >= 0 {
                    if let Some(c) = clip_sous(&self.projet, piste as u32, tic).cloned() {
                        self.modifier(|p| {
                            supprimer_clip(p, &c.id);
                        });
                    }
                }
            }
            None => {}
        }
    }

    fn dessiner_arrangement(&self, ui: &egui::Ui, tout: Rect, zone: Rect, mesure: u32) {
        let a = &self.arrangement;
        let px_par_tic = a.px_par_tic(mesure);
        let peintre = ui.painter_at(tout);
        let pistes = nombre_de_pistes(&self.projet);
        peintre.rect_filled(tout, 0.0, Color32::from_rgb(0x22, 0x27, 0x2e));
        // lignes des pistes et des mesures (seules les visibles)
        let premiere = (a.y / H_PISTE).floor().max(0.0) as u32;
        let derniere = (((a.y + zone.height()) / H_PISTE).ceil() as u32).min(pistes);
        let peintre_zone = ui.painter_at(zone);
        for piste in premiere..=derniere {
            let y = zone.top() + piste as f32 * H_PISTE - a.y;
            peintre_zone.hline(zone.x_range(), y, Stroke::new(1.0, Color32::from_rgb(0x11, 0x15, 0x1a)));
        }
        let m0 = (a.x / a.px_par_mesure).floor().max(0.0) as u32;
        let m1 = ((a.x + zone.width()) / a.px_par_mesure).ceil() as u32;
        for m in m0..=m1 {
            let x = zone.left() + m as f32 * a.px_par_mesure - a.x;
            peintre_zone.vline(x, zone.y_range(), Stroke::new(1.0, Color32::from_rgb(0x3a, 0x42, 0x4c)));
        }
        // fin du morceau
        let fin = projet::lecture::longueur_du_morceau(&self.projet);
        if !self.projet.playlist.is_empty() {
            let x = zone.left() + fin as f32 * px_par_tic - a.x;
            peintre_zone.vline(x, zone.y_range(), Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.5)));
        }
        // clips visibles
        let vue = Rect::from_min_size(Pos2::new(a.x, a.y), zone.size());
        for clip in &self.projet.playlist {
            let Some(pattern) = self.projet.patterns.iter().find(|p| p.id == clip.pattern) else { continue };
            let r = Rect::from_min_size(Pos2::new(clip.debut as f32 * px_par_tic, clip.piste as f32 * H_PISTE + 2.0), Vec2::new((pattern.longueur as f32 * px_par_tic - 1.0).max(4.0), H_PISTE - 4.0));
            if !r.intersects(vue) {
                continue;
            }
            let ecran = r.translate(zone.min.to_vec2() - vue.min.to_vec2());
            let fond = couleur_pattern(&self.projet, &pattern.id);
            peintre_zone.rect_filled(ecran, 3.0, fond);
            let actif = pattern.id == self.projet.pattern_actif;
            peintre_zone.rect_stroke(ecran, 3.0, Stroke::new(1.0, if actif { Color32::WHITE } else { Color32::from_black_alpha(150) }), StrokeKind::Inside);
            apercu_clip(&peintre_zone, pattern, ecran.shrink2(Vec2::new(3.0, 2.0)).with_min_y(ecran.top() + 13.0));
            ui.painter_at(ecran.intersect(zone)).text(ecran.min + Vec2::new(4.0, 2.0), Align2::LEFT_TOP, &pattern.nom, FontId::proportional(10.0), Color32::BLACK);
            // pour l'accessibilité et les tests : un nœud par clip visible
            let id = ui.id().with(("clip", &clip.id));
            let r = ui.interact(ecran.intersect(zone), id, Sense::hover());
            let numero = clip.debut / mesure + 1;
            r.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, format!("Clip {} piste {} mesure {numero}", pattern.nom, clip.piste + 1)));
        }
        // tête de lecture en mode morceau
        if self.mode == ModeLecture::Morceau && self.etat_transport() != EtatTransport::Arret {
            let x = zone.left() + self.position() as f32 * px_par_tic - a.x;
            peintre_zone.vline(x, zone.y_range(), Stroke::new(2.0, theme::ACTIF));
        }
        // règle des mesures
        let regle = Rect::from_min_max(Pos2::new(zone.left(), tout.top()), Pos2::new(zone.right(), zone.top()));
        let peintre_regle = ui.painter_at(regle);
        peintre_regle.rect_filled(regle, 0.0, Color32::from_rgb(0x1f, 0x24, 0x2a));
        let pas_numero = (40.0 / a.px_par_mesure).ceil().max(1.0) as u32;
        for m in m0..=m1 {
            let x = zone.left() + m as f32 * a.px_par_mesure - a.x;
            peintre_regle.vline(x, regle.y_range(), Stroke::new(1.0, Color32::from_rgb(0x3a, 0x42, 0x4c)));
            if m % pas_numero == 0 {
                peintre_regle.text(Pos2::new(x + 3.0, regle.center().y), Align2::LEFT_CENTER, (m + 1).to_string(), FontId::monospace(10.0), theme::DISCRET);
            }
        }
        // noms des pistes
        let entete = Rect::from_min_max(Pos2::new(tout.left(), zone.top()), Pos2::new(zone.left(), zone.bottom()));
        let peintre_entete = ui.painter_at(entete);
        peintre_entete.rect_filled(entete, 0.0, theme::PANNEAU);
        for piste in premiere..derniere {
            let y = zone.top() + piste as f32 * H_PISTE - a.y;
            peintre_entete.text(Pos2::new(entete.left() + 6.0, y + H_PISTE / 2.0), Align2::LEFT_CENTER, format!("Piste {}", piste + 1), FontId::proportional(12.0), theme::DISCRET);
            peintre_entete.hline(entete.x_range(), y + H_PISTE, Stroke::new(1.0, Color32::from_black_alpha(120)));
        }
    }
}

/// Aperçu des notes d'un pattern dans un clip (tous canaux confondus).
fn apercu_clip(peintre: &egui::Painter, pattern: &Pattern, r: Rect) {
    let notes: Vec<_> = pattern.notes.values().flatten().collect();
    let (Some(basse), Some(haute)) = (notes.iter().map(|n| n.hauteur).min(), notes.iter().map(|n| n.hauteur).max()) else { return };
    if r.width() < 2.0 || r.height() < 2.0 {
        return;
    }
    let etendue = ((haute - basse) as f32 + 1.0).max(12.0);
    let px = r.width() / pattern.longueur.max(1) as f32;
    let h = (r.height() / etendue).max(1.0);
    for n in notes {
        let x = r.left() + n.tic as f32 * px;
        if x > r.right() {
            continue;
        }
        let y = r.top() + (haute - n.hauteur) as f32 / etendue * r.height();
        let l = (n.duree as f32 * px).max(1.0).min(r.right() - x);
        peintre.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(l, h)), 0.0, Color32::from_black_alpha(140));
    }
}
