//! Vue de l'instrument choisi : elle s'allume pendant la lecture et se joue à la souris et au
//! clavier de l'ordinateur. Un clavier de 88 touches pour les instruments mélodiques (la
//! vélocité dépend de la hauteur où l'on frappe la touche, glisser fait un glissando) ; des pads
//! disposés comme une batterie pour les canaux de batterie. Les vues dédiées (manche, harpe,
//! pistons) arrivent à la phase N7.

use std::collections::BTreeSet;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType};
use moteur::EtatTransport;
use projet::Canal;
use projet::clavier::{HAUTEUR_NOIRE, NOTE_MAX, NOTE_MIN, Touche, caractere, disposition, etiquettes, nombre_de_blanches, touche_sous};
use projet::edition::notes_au_tic;
use projet::gm::{famille_instrument, nom_instrument, nom_note, nom_son_batterie};
use projet::lecture::ModeLecture;

use crate::app::{Studio, couleur, famille_ou_synthe};
use crate::theme;

/// Pads disposés comme une batterie : cymbales, charleston, toms, caisses, grosse caisse, percussions.
pub const KIT: [(&str, &[u8]); 6] = [
    ("Cymbales", &[49, 57, 55, 52, 51, 53]),
    ("Charleston", &[42, 44, 46]),
    ("Toms", &[50, 48, 47, 45, 43, 41]),
    ("Caisses", &[38, 40, 37, 39]),
    ("Grosse caisse", &[36, 35]),
    ("Percussions", &[54, 56, 69, 70, 75, 76, 77, 60, 61, 62, 63, 64, 80, 81]),
];

const HAUTEUR_CLAVIER_MIN: f32 = 90.0;

#[derive(Debug)]
pub struct EtatInstrument {
    /// Touche tenue à la souris sur le clavier, et son canal.
    touche: Option<(u8, String)>,
    /// Pad tenu, et le canal qu'il joue.
    pad: Option<(u8, String)>,
    /// Octave du clavier de l'ordinateur déjà amenée à l'écran.
    base_montree: Option<i32>,
    /// Zone du clavier et largeur d'une touche blanche (dernière image).
    pub zone_clavier: Rect,
    pub largeur_blanche: f32,
}

impl Default for EtatInstrument {
    fn default() -> Self {
        EtatInstrument { touche: None, pad: None, base_montree: None, zone_clavier: Rect::NOTHING, largeur_blanche: 16.0 }
    }
}

impl EtatInstrument {
    /// Point de l'écran au milieu d'une touche, à une hauteur relative (0 en haut, 1 en bas).
    pub fn vers_ecran(&self, midi: u8, y: f32) -> Option<Pos2> {
        let t = disposition(NOTE_MIN, NOTE_MAX).into_iter().find(|t| t.midi == midi)?;
        Some(Pos2::new(self.zone_clavier.left() + (t.x + t.largeur / 2.0) * self.largeur_blanche, self.zone_clavier.top() + y * self.zone_clavier.height()))
    }
}

impl Studio {
    /// Hauteurs qui sonnent sur ces canaux : le pattern à la position de lecture, et le jeu en
    /// direct.
    pub fn notes_actives(&self, canaux: &[&str]) -> BTreeSet<u8> {
        let mut hauteurs = BTreeSet::new();
        if self.etat_transport() == EtatTransport::Lecture && self.mode == ModeLecture::Pattern {
            let actives = notes_au_tic(self.projet.pattern_actif(), self.position());
            for c in canaux {
                hauteurs.extend(actives.get(c).into_iter().flatten().map(|n| n.hauteur));
            }
        }
        for c in canaux {
            hauteurs.extend(self.notes_en_direct(c));
        }
        hauteurs
    }

    pub fn vue_instrument(&mut self, ui: &mut egui::Ui) {
        let Some(canal) = self.canal_selectionne.as_deref().and_then(|id| self.projet.canal(id)).cloned() else {
            ui.label(RichText::new("Choisissez un instrument dans le rack.").color(theme::DISCRET));
            return;
        };
        if canal.batterie { self.pads(ui, &canal) } else { self.clavier(ui, &canal) }
    }

    fn clavier(&mut self, ui: &mut egui::Ui, canal: &Canal) {
        let touches = disposition(NOTE_MIN, NOTE_MAX);
        let blanches = nombre_de_blanches(&touches) as f32;
        let disposition_clavier = self.jeu.disposition;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&canal.nom).strong().color(couleur(&canal.couleur)));
            let son = match canal.source {
                projet::SourceCanal::Soundfont => format!("{} ({})", nom_instrument(canal.programme), famille_instrument(canal.programme)),
                projet::SourceCanal::Soustractif | projet::SourceCanal::Echantillons | projet::SourceCanal::Couche => famille_ou_synthe(canal),
            };
            ui.label(RichText::new(son).color(theme::DISCRET));
            ui.separator();
            ui.label("Clavier");
            if ui.button(disposition_clavier.nom()).on_hover_text("Changer la disposition du clavier").clicked() {
                self.jeu.disposition = disposition_clavier.autre();
            }
            let d = self.jeu.disposition;
            ui.label(
                RichText::new(format!(
                    "{}…{} et {}…{} · flèches gauche et droite : octave",
                    caractere("KeyZ", d),
                    caractere("Slash", d),
                    caractere("KeyQ", d),
                    caractere("BracketRight", d)
                ))
                .color(theme::DISCRET),
            );
        });
        let lettres = etiquettes(self.jeu.base, self.jeu.disposition);
        let actives = self.notes_actives(&[canal.id.as_str()]);
        let hauteur = ui.available_height().max(HAUTEUR_CLAVIER_MIN);
        let largeur = (ui.available_width() / blanches).max(16.0);
        // garder visible l'octave jouée au clavier de l'ordinateur
        let mut defiler = None;
        if self.instrument.base_montree != Some(self.jeu.base) {
            self.instrument.base_montree = Some(self.jeu.base);
            if let Some(t) = touches.iter().find(|t| t.midi as i32 == self.jeu.base + 12) {
                defiler = Some((t.x * largeur - ui.available_width() / 2.0 + 3.5 * largeur).max(0.0));
            }
        }
        let mut zone_defilement = egui::ScrollArea::horizontal().id_salt("clavier").auto_shrink([false, true]);
        if let Some(x) = defiler {
            zone_defilement = zone_defilement.horizontal_scroll_offset(x);
        }
        zone_defilement.show(ui, |ui| {
            let (zone, reponse) = ui.allocate_exact_size(Vec2::new(largeur * blanches, hauteur), Sense::click_and_drag());
            self.instrument.zone_clavier = zone;
            self.instrument.largeur_blanche = largeur;
            // une étiquette par touche (accessibilité) ; le jeu se fait par la zone entière
            for t in &touches {
                let rect = rect_touche(zone, t, largeur);
                let r = ui.interact(rect, reponse.id.with(t.midi), Sense::hover());
                let nom = nom_note(t.midi);
                let active = actives.contains(&t.midi);
                r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, active, &nom));
            }
            self.jouer_clavier(ui, &reponse, zone, &touches, largeur, canal);
            let peintre = ui.painter_at(zone);
            let teinte = couleur(&canal.couleur);
            for noire in [false, true] {
                for t in touches.iter().filter(|t| t.noire == noire) {
                    let rect = rect_touche(zone, t, largeur);
                    let active = actives.contains(&t.midi);
                    let fond = match (active, noire) {
                        (true, _) => teinte,
                        (false, true) => Color32::from_rgb(0x1c, 0x1e, 0x22),
                        (false, false) => Color32::from_rgb(0xee, 0xea, 0xe2),
                    };
                    let arrondi = egui::CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 };
                    peintre.rect_filled(rect, arrondi, fond);
                    peintre.rect_stroke(rect, arrondi, Stroke::new(1.0, if noire { Color32::BLACK } else { Color32::from_rgb(0x88, 0x88, 0x88) }), StrokeKind::Inside);
                    let encre = if noire && !active { Color32::from_rgb(0xbb, 0xbb, 0xbb) } else { Color32::from_rgb(0x55, 0x55, 0x55) };
                    let mut bas = rect.bottom() - 3.0;
                    if t.midi % 12 == 0 {
                        peintre.text(Pos2::new(rect.center().x, bas), Align2::CENTER_BOTTOM, nom_note(t.midi), FontId::proportional(9.0), encre);
                        bas -= 12.0;
                    }
                    if let Some(l) = lettres.get(&t.midi) {
                        peintre.text(Pos2::new(rect.center().x, bas), Align2::CENTER_BOTTOM, l, FontId::proportional(10.0), encre);
                    }
                }
            }
        });
    }

    /// Souris sur le clavier : appui = note, glisser = glissando, relâcher = fin de la note.
    fn jouer_clavier(&mut self, ui: &egui::Ui, reponse: &egui::Response, zone: Rect, touches: &[Touche], largeur: f32, canal: &Canal) {
        let (pos, enfonce, appui) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down(), i.pointer.primary_pressed()));
        let cible = pos.and_then(|p| {
            let x = (p.x - zone.left()) / largeur;
            let y = ((p.y - zone.top()) / zone.height()).clamp(0.0, 1.0);
            let midi = touche_sous(touches, x, y)?;
            let noire = touches.iter().any(|t| t.midi == midi && t.noire);
            let velocite = 40.0 + 87.0 * (y / if noire { HAUTEUR_NOIRE } else { 1.0 }).clamp(0.0, 1.0);
            Some((midi, velocite.round() as u8))
        });
        if appui && reponse.contains_pointer() {
            if let Some((midi, v)) = cible {
                self.instrument.touche = Some((midi, canal.id.clone()));
                self.jouer_note(midi, v, &canal.id);
            }
        } else if enfonce {
            if let (Some((actuelle, id)), Some((midi, v))) = (self.instrument.touche.clone(), cible) {
                if midi != actuelle && zone.contains(pos.unwrap_or_default()) {
                    self.relacher_note(actuelle, &id);
                    self.instrument.touche = Some((midi, id.clone()));
                    self.jouer_note(midi, v, &id);
                }
            }
        }
        if !enfonce {
            if let Some((midi, id)) = self.instrument.touche.take() {
                self.relacher_note(midi, &id);
            }
        }
    }

    fn pads(&mut self, ui: &mut egui::Ui, canal: &Canal) {
        let batteries: Vec<(String, u8)> = self.projet.canaux.iter().filter(|c| c.batterie).map(|c| (c.id.clone(), c.note_defaut)).collect();
        let ids: Vec<&str> = batteries.iter().map(|(id, _)| id.as_str()).collect();
        let actives = self.notes_actives(&ids);
        // un pad joue la ligne de batterie de ce son si elle existe, sinon le canal choisi
        let canal_pour = |note: u8| batteries.iter().find(|(_, n)| *n == note).map(|(id, _)| id.clone()).unwrap_or_else(|| canal.id.clone());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Batterie :").color(theme::DISCRET));
            ui.label(RichText::new(&canal.nom).color(couleur(&canal.couleur)));
            ui.label(RichText::new("· les pads jouent le kit General MIDI").color(theme::DISCRET));
        });
        let enfonce = ui.input(|i| i.pointer.primary_down());
        let teinte = couleur(&canal.couleur);
        let mut appuye = None;
        egui::ScrollArea::vertical().id_salt("pads").auto_shrink([false, false]).show(ui, |ui| {
            // groupes alignés en haut de leur rangée
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), |ui| {
                for (titre, sons) in KIT {
                    // taille connue d'avance (au plus 4 pads par rangée) : les groupes passent à la
                    // ligne quand le panneau est étroit, aucun pad ne sort de la vue
                    let espace = ui.spacing().item_spacing;
                    let par_rangee = sons.len().min(4) as f32;
                    let rangees = sons.len().div_ceil(4) as f32;
                    let taille = Vec2::new(par_rangee * (72.0 + espace.x), 16.0 + rangees * (48.0 + espace.y));
                    ui.allocate_ui_with_layout(taille, egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_min_size(taille);
                        ui.label(RichText::new(titre.to_uppercase()).small().color(theme::DISCRET));
                        ui.horizontal_wrapped(|ui| {
                            ui.set_max_width(taille.x);
                            for &note in sons {
                                let active = actives.contains(&note);
                                let texte = RichText::new(nom_son_batterie(note)).small().color(if active { Color32::BLACK } else { theme::TEXTE });
                                let mut bouton = egui::Button::new(texte).min_size(Vec2::new(72.0, 48.0)).wrap();
                                if active {
                                    bouton = bouton.fill(teinte);
                                }
                                let r = ui.add_sized([72.0, 48.0], bouton).on_hover_text(format!("{} ({note})", nom_son_batterie(note)));
                                // nom distinct du canal de batterie du même son, pour l'accessibilité
                                r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, active, format!("Pad {}", nom_son_batterie(note))));
                                if r.is_pointer_button_down_on() {
                                    appuye = Some(note);
                                }
                            }
                        });
                    });
                }
            });
        });
        // un pad joue tant qu'on le tient
        let tenu = self.instrument.pad.clone();
        match (appuye.filter(|_| enfonce), tenu) {
            (Some(note), Some((avant, id))) if note != avant => {
                self.relacher_note(avant, &id);
                let id = canal_pour(note);
                self.instrument.pad = Some((note, id.clone()));
                self.jouer_note(note, 110, &id);
            }
            (Some(note), None) => {
                let id = canal_pour(note);
                self.instrument.pad = Some((note, id.clone()));
                self.jouer_note(note, 110, &id);
            }
            (None, Some((avant, id))) => {
                self.instrument.pad = None;
                self.relacher_note(avant, &id);
            }
            _ => {}
        }
    }
}

fn rect_touche(zone: Rect, t: &Touche, largeur: f32) -> Rect {
    let h = if t.noire { zone.height() * HAUTEUR_NOIRE } else { zone.height() };
    Rect::from_min_size(Pos2::new(zone.left() + t.x * largeur, zone.top()), Vec2::new(t.largeur * largeur, h))
}
