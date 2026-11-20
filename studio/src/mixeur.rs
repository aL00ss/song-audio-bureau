//! Le mixer : le maître puis une tranche par piste (égaliseur 3 bandes, envoi de réverbération,
//! panoramique, piste active, solo, fader, vumètre stéréo). Pistes sans limite : seules les
//! tranches visibles sont dessinées. Les canaux du rack y sont routés (numéro de piste dans le
//! rack).

use std::f32::consts::PI;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, UiBuilder, Vec2, WidgetInfo, WidgetType};
use projet::mixage::{
    gain_du_fader, hauteur_vumetre, pistes_inutilisees, renommer_piste, retirer_pistes_inutilisees, retombee, texte_db, texte_pan,
    une_piste_par_canal,
};
use projet::modele::{REVERB_DEFAUT, pistes_audibles};

use crate::app::Studio;
use crate::theme;

/// Largeur d'une tranche, écart compris.
pub const LARGEUR_TRANCHE: f32 = 116.0;
/// Diamètre des boutons de l'égaliseur et de la réverbération.
const PETIT_BOUTON: f32 = 22.0;
const HAUTEUR_FADER: f32 = 120.0;
/// Hauteur utile d'une tranche : en dessous, le mixer défile verticalement.
const HAUTEUR_TRANCHE: f32 = 330.0;

/// Double clic (un clic rapide de plus compte comme triple : on l'accepte aussi).
fn double_clic(r: &egui::Response) -> bool {
    r.double_clicked() || r.triple_clicked()
}

/// Bouton rotatif : glisser verticalement (Maj : finement), double clic pour la valeur par
/// défaut, flèches haut et bas quand il a le focus. Vrai si la valeur a changé.
#[allow(clippy::too_many_arguments)]
pub fn bouton_rotatif(ui: &mut egui::Ui, taille: f32, valeur: &mut f32, min: f32, max: f32, defaut: f32, ancre: f32, etiquette: &str, texte: &str) -> egui::Response {
    let (rect, mut reponse) = ui.allocate_exact_size(Vec2::splat(taille), Sense::click_and_drag());
    reponse.widget_info(|| WidgetInfo::slider(true, *valeur as f64, etiquette));
    let avant = *valeur;
    let etendue = max - min;
    if reponse.dragged() {
        let fin = if ui.input(|i| i.modifiers.shift) { 0.2 } else { 1.0 };
        *valeur = (*valeur - reponse.drag_delta().y * etendue / 150.0 * fin).clamp(min, max);
    }
    if double_clic(&reponse) {
        *valeur = defaut;
    }
    if reponse.has_focus() {
        let (haut, bas) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
        if haut || bas {
            *valeur = (*valeur + if haut { etendue / 50.0 } else { -etendue / 50.0 }).clamp(min, max);
        }
        ui.memory_mut(|m| m.set_focus_lock_filter(reponse.id, egui::EventFilter { vertical_arrows: true, ..Default::default() }));
    }
    if *valeur != avant {
        reponse.mark_changed();
    }
    // dessin : piste, arc depuis l'ancre, aiguille
    let p = ui.painter();
    let centre = rect.center();
    let rayon = taille / 2.0 - 3.0;
    let angle = |v: f32| (0.75 + 1.5 * (v - min) / etendue) * PI; // de 135° à 405°
    let arc = |a: f32, b: f32, couleur: Color32| {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let n = (((b - a) / PI * 24.0).ceil() as usize).max(2);
        let points: Vec<Pos2> = (0..=n).map(|k| a + (b - a) * k as f32 / n as f32).map(|t| centre + Vec2::new(t.cos(), t.sin()) * rayon).collect();
        Shape::line(points, Stroke::new(3.0, couleur))
    };
    p.add(arc(angle(min), angle(max), Color32::from_rgb(0x15, 0x1a, 0x20)));
    p.add(arc(angle(ancre), angle(*valeur), theme::ACCENT));
    p.circle_filled(centre, rayon - 3.0, Color32::from_rgb(0x3a, 0x42, 0x4d));
    let t = angle(*valeur);
    p.line_segment([centre + Vec2::new(t.cos(), t.sin()) * 2.0, centre + Vec2::new(t.cos(), t.sin()) * (rayon - 3.0)], Stroke::new(2.0, theme::TEXTE));
    if reponse.has_focus() {
        p.circle_stroke(centre, rayon + 2.0, Stroke::new(1.0, theme::ACCENT));
    }
    reponse.on_hover_text(format!("{etiquette} : {texte}"))
}

/// Vumètre stéréo : deux barres de -60 à +6 dB, vert, jaune puis rouge.
fn vumetre(ui: &mut egui::Ui, niveau: [f32; 2]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, HAUTEUR_FADER), Sense::hover());
    let p = ui.painter();
    for (k, n) in niveau.iter().enumerate() {
        let barre = Rect::from_min_size(Pos2::new(rect.left() + k as f32 * 10.0, rect.top()), Vec2::new(8.0, rect.height()));
        p.rect_filled(barre, 1.0, Color32::from_black_alpha(150));
        let h = hauteur_vumetre(*n);
        if h <= 0.0 {
            continue;
        }
        // trois zones : jusqu'à -6 dB vert, jusqu'à 0 dB jaune, au-delà rouge
        let zones = [(0.0, 54.0 / 66.0, Color32::from_rgb(0x3e, 0xc4, 0x6d)), (54.0 / 66.0, 60.0 / 66.0, Color32::from_rgb(0xe5, 0xd8, 0x4a)), (60.0 / 66.0, 1.0, Color32::from_rgb(0xf0, 0x5a, 0x4f))];
        for (debut, fin, couleur) in zones {
            if h <= debut {
                break;
            }
            let haut = barre.bottom() - barre.height() * h.min(fin);
            let bas = barre.bottom() - barre.height() * debut;
            p.rect_filled(Rect::from_x_y_ranges(barre.x_range(), haut..=bas), 0.0, couleur);
        }
    }
}

impl Studio {
    /// Lit les mesures du moteur une fois par image (la lecture remet les crêtes à zéro) et fait
    /// retomber les vumètres.
    pub fn lire_mesures(&mut self, secondes: f32) {
        let Some(m) = self.moteur() else { return };
        let mesures = m.mesures();
        let n = mesures.cretes.len().max(self.projet.mixage.len());
        self.niveaux.resize(n, [0.0; 2]);
        for (i, niveau) in self.niveaux.iter_mut().enumerate() {
            let crete = mesures.cretes.get(i).copied().unwrap_or([0.0; 2]);
            *niveau = [retombee(niveau[0], crete[0], secondes), retombee(niveau[1], crete[1], secondes)];
        }
        self.mesures = mesures;
    }

    pub fn mixeur(&mut self, ui: &mut egui::Ui) {
        let inutiles = pistes_inutilisees(&self.projet).len();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} pistes", self.projet.mixage.len())).color(theme::DISCRET));
            ui.separator();
            if ui.button("+ Piste").on_hover_text("Ajouter une piste au mixer").clicked() {
                self.modifier(|p| {
                    projet::mixage::ajouter_piste(p, None);
                });
            }
            if ui.button("Une piste par canal").on_hover_text("Donner une piste à chaque canal qui partage la sienne ou va au maître").clicked() {
                self.modifier(|p| {
                    une_piste_par_canal(p);
                });
            }
            let retirer = ui
                .add_enabled(inutiles > 0, egui::Button::new(format!("Retirer les pistes inutilisées ({inutiles})")))
                .on_hover_text("Retirer les pistes qui ne reçoivent aucun canal (les autres sont renumérotées)");
            if retirer.clicked() {
                self.modifier(|p| {
                    retirer_pistes_inutilisees(p);
                });
            }
        });
        ui.separator();
        let audibles = pistes_audibles(&self.projet.mixage);
        let nb = self.projet.mixage.len();
        egui::ScrollArea::both().id_salt("mixer").auto_shrink([false, false]).show_viewport(ui, |ui, vue| {
            let hauteur = ui.available_height().max(HAUTEUR_TRANCHE);
            let (total, _) = ui.allocate_exact_size(Vec2::new(nb as f32 * LARGEUR_TRANCHE, hauteur), Sense::hover());
            // seules les tranches visibles sont dessinées
            let premiere = (vue.left() / LARGEUR_TRANCHE).floor().max(0.0) as usize;
            let derniere = ((vue.right() / LARGEUR_TRANCHE).ceil() as usize).min(nb);
            for index in premiere..derniere {
                let rect = Rect::from_min_size(Pos2::new(total.left() + index as f32 * LARGEUR_TRANCHE, total.top()), Vec2::new(LARGEUR_TRANCHE - 4.0, hauteur));
                let audible = index == 0 || audibles.contains(&index);
                ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt(("tranche", index)), |ui| {
                    if !audible {
                        ui.multiply_opacity(0.6);
                    }
                    self.tranche(ui, index, rect);
                });
            }
        });
    }

    fn tranche(&mut self, ui: &mut egui::Ui, index: usize, rect: Rect) {
        let Some(piste) = self.projet.mixage.get(index).cloned() else { return };
        let maitre = index == 0;
        ui.painter().rect_filled(rect, 3.0, if maitre { Color32::from_rgb(0x2e, 0x2a, 0x22) } else { Color32::from_rgb(0x27, 0x2c, 0x33) });
        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_black_alpha(150)), StrokeKind::Inside);
        let canaux: Vec<String> = self.projet.canaux.iter().filter(|c| c.piste_mixage == index).map(|c| c.nom.clone()).collect();
        ui.vertical_centered(|ui| {
            ui.add_space(3.0);
            ui.label(RichText::new(if maitre { "M".to_string() } else { index.to_string() }).monospace().small().color(theme::DISCRET));
            // nom : double clic pour renommer
            if self.renommage_piste.as_ref().is_some_and(|(i, _)| *i == index) {
                let (i, mut texte) = self.renommage_piste.take().expect("renommage");
                let r = ui.add(egui::TextEdit::singleline(&mut texte).desired_width(LARGEUR_TRANCHE - 14.0).id_salt(("nom piste", i)));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                let (entree, echap) = ui.input(|e| (e.key_pressed(egui::Key::Enter), e.key_pressed(egui::Key::Escape)));
                if !echap {
                    if entree || r.lost_focus() {
                        self.modifier(|p| {
                            renommer_piste(p, i, &texte);
                        });
                    } else {
                        self.renommage_piste = Some((i, texte));
                    }
                }
            } else {
                let aide = if canaux.is_empty() { "Aucun canal".to_string() } else { canaux.join(", ") };
                let nom = ui.add(egui::Label::new(RichText::new(&piste.nom).strong()).truncate().sense(Sense::click())).on_hover_text(format!("{aide}\nDouble clic : renommer"));
                if double_clic(&nom) {
                    self.renommage_piste = Some((index, piste.nom.clone()));
                }
            }
            let mut reglage = piste.clone();
            let debut = ui.cursor().top();
            if maitre {
                // même hauteur que les réglages des autres pistes : les faders restent alignés
                ui.add_space(self.hauteur_reglages);
            } else {
                // égaliseur et réverbération
                egui::Grid::new(("eq", index)).spacing([3.0, 0.0]).min_col_width(PETIT_BOUTON).show(ui, |ui| {
                    let db = |v: f32| format!("{}{v:.1} dB", if v > 0.0 { "+" } else { "" });
                    for (bande, valeur) in [("Grave", &mut reglage.eq.grave), ("Médium", &mut reglage.eq.medium), ("Aigu", &mut reglage.eq.aigu)] {
                        let texte = db(*valeur);
                        bouton_rotatif(ui, PETIT_BOUTON, valeur, -12.0, 12.0, 0.0, 0.0, &format!("Égaliseur {bande} {}", piste.nom), &texte);
                    }
                    let texte = format!("{} %", (reglage.reverb * 100.0).round());
                    bouton_rotatif(ui, PETIT_BOUTON, &mut reglage.reverb, 0.0, 1.0, REVERB_DEFAUT, 0.0, &format!("Réverbération {}", piste.nom), &texte);
                    ui.end_row();
                    for t in ["Grave", "Méd.", "Aigu", "Rév."] {
                        ui.vertical_centered(|ui| ui.label(RichText::new(t).size(8.0).color(theme::DISCRET)));
                    }
                    ui.end_row();
                });
                let texte = texte_pan(reglage.pan);
                bouton_rotatif(ui, 28.0, &mut reglage.pan, -1.0, 1.0, 0.0, 0.0, &format!("Panoramique piste {}", piste.nom), &texte);
                self.hauteur_reglages = ui.cursor().top() - debut;
            }
            ui.horizontal(|ui| {
                ui.add_space(if maitre { 34.0 } else { 22.0 });
                // piste active (allumée) ou coupée
                let (r, reponse) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                reponse.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, !reglage.muet, format!("Activer {}", piste.nom)));
                ui.painter().circle_filled(r.center(), 7.0, if reglage.muet { Color32::from_rgb(0x3a, 0x41, 0x4b) } else { theme::ACTIF });
                ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.0, Color32::from_black_alpha(150)));
                if reponse.on_hover_text("Piste active (cliquer pour couper)").clicked() {
                    reglage.muet = !reglage.muet;
                }
                if !maitre {
                    let solo = ui.toggle_value(&mut reglage.solo, RichText::new("S").small().strong()).on_hover_text(format!("Solo {}", piste.nom));
                    solo.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, reglage.solo, format!("Solo {}", piste.nom)));
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.spacing_mut().slider_width = HAUTEUR_FADER;
                let texte = texte_db(gain_du_fader(reglage.volume));
                let fader = ui
                    .add(egui::Slider::new(&mut reglage.volume, 0.0..=1.0).vertical().show_value(false))
                    .on_hover_text(format!("Volume piste {} : {texte} (double clic : 0 dB)", piste.nom));
                fader.widget_info(|| WidgetInfo::slider(true, reglage.volume as f64, format!("Volume piste {}", piste.nom)));
                // le curseur ne réagit qu'au glisser : le double clic se lit sur le pointeur
                let double = ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary) || i.pointer.button_triple_clicked(egui::PointerButton::Primary));
                if fader.hovered() && double {
                    reglage.volume = projet::mixage::FADER_UNITE;
                }
                vumetre(ui, self.niveaux.get(index).copied().unwrap_or([0.0; 2]));
            });
            ui.label(RichText::new(texte_db(gain_du_fader(reglage.volume))).monospace().small().color(theme::DISCRET));
            if reglage != piste {
                self.modifier(|p| {
                    if let Some(x) = p.mixage.get_mut(index) {
                        *x = reglage;
                    }
                });
            }
        });
    }
}
