//! Le rack d'instruments et son séquenceur pas à pas : une ligne par canal (témoin, muet, solo,
//! volume, panoramique, piste de mixage, nom, son de batterie), puis ses pas (ou un aperçu de
//! ses notes s'il n'est pas en pas), et sous la liste l'éditeur de vélocité du canal choisi.
//! Liste virtualisée : seules les lignes visibles sont dessinées.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2, WidgetInfo, WidgetType};
use moteur::EtatTransport;
use projet::gm::nom_son_batterie;
use projet::pas::{PAS_MAX, definir_note_defaut, definir_nombre_de_pas, definir_pas, definir_velocite_pas, est_en_mode_pas, nombre_de_pas, velocites_des_pas};
use projet::temps::TICS_PAR_PAS;
use projet::canaux::{PALETTE, cloner_canal, colorer_canal, deplacer_canal, remplacer_son, renommer_canal, supprimer_canal};
use projet::gm::{FAMILLES, nom_instrument};
use projet::synthes::PREREGLAGES_SOUSTRACTIF;
use projet::{NoteMusicale, SourceCanal};

use crate::app::{Studio, couleur, famille_ou_synthe};
use crate::theme;

/// Taille d'un pas à l'écran.
pub const PAS: Vec2 = Vec2::new(16.0, 20.0);
const HAUTEUR_VELOCITES: f32 = 56.0;
const LARGEUR_SON: f32 = 130.0;
const LARGEUR_NOM: f32 = 140.0;
/// Écart entre deux pas, et en plus entre deux temps (groupes de 4 pas).
const ECART_PAS: f32 = 2.0;
const ECART_TEMPS: f32 = 4.0;

/// Peinture en cours au glisser : canal et état voulu des pas survolés.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Peinture {
    pub canal: String,
    pub actif: bool,
}

impl Studio {
    pub fn rack(&mut self, ui: &mut egui::Ui) {
        let pattern = self.projet.pattern_actif().clone();
        let nb_pas = nombre_de_pas(&pattern) as usize;
        let pas_courant = (self.etat_transport() == EtatTransport::Lecture).then(|| (self.position() / TICS_PAR_PAS as f64) as usize % nb_pas.max(1));
        let voix = self.mesures.voix.clone();

        // en-tête : longueur du pattern, swing
        ui.horizontal(|ui| {
            // le pattern édité : on en change ici, ou on en crée un
            let mut choisi = self.projet.pattern_actif.clone();
            egui::ComboBox::from_id_salt("pattern actif").selected_text(RichText::new(&pattern.nom).strong()).show_ui(ui, |ui| {
                for p in &self.projet.patterns {
                    ui.selectable_value(&mut choisi, p.id.clone(), &p.nom);
                }
            });
            if choisi != self.projet.pattern_actif {
                self.modifier_si(|p| projet::playlist::choisir_pattern(p, &choisi));
            }
            if ui.small_button("+ Pattern").on_hover_text("Nouveau pattern").clicked() {
                self.modifier(|p| {
                    projet::playlist::nouveau_pattern(p);
                });
            }
            ui.label(RichText::new(format!("{} instruments", self.projet.canaux.len())).color(theme::DISCRET));
            ui.separator();
            let mut pas = nb_pas as i64;
            ui.label("Pas");
            if ui.add(egui::DragValue::new(&mut pas).range(1..=PAS_MAX as i64)).on_hover_text("Longueur du pattern, en pas (doubles croches)").changed() {
                self.modifier(|p| definir_nombre_de_pas(p, pas));
            }
            ui.separator();
            let mut swing = (self.projet.swing * 100.0).round();
            ui.label("Swing");
            if ui.add(egui::Slider::new(&mut swing, 0.0..=100.0).suffix(" %").integer()).on_hover_text("Swing : retarde les pas impairs").changed() {
                self.modifier(|p| p.swing = swing / 100.0);
            }
        });
        ui.separator();

        // fin d'une peinture : bouton relâché
        if !ui.input(|i| i.pointer.any_down()) {
            self.peinture = None;
        }
        let pointeur = ui.input(|i| i.pointer.interact_pos());
        self.colonnes_pas.clear();

        let canaux = self.projet.canaux.clone();
        let nb_pistes = self.projet.mixage.len();
        let hauteur_ligne = PAS.y + ui.spacing().item_spacing.y + 2.0;
        let sortie = egui::ScrollArea::both().id_salt("rack").auto_shrink([false, false]).max_height((ui.available_height() - HAUTEUR_VELOCITES - 30.0).max(60.0)).show_rows(
            ui,
            hauteur_ligne,
            canaux.len(),
            |ui, lignes| {
                for rang in lignes {
                    let canal = &canaux[rang];
                    let notes = pattern.notes.get(&canal.id).cloned().unwrap_or_default();
                    let en_pas = est_en_mode_pas(&notes, canal.note_defaut);
                    ui.horizontal(|ui| {
                        // témoin d'activité : le canal du moteur est le rang du canal dans le projet
                        let actif = voix.get(rang).copied().unwrap_or(0) > 0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, if actif { theme::ACTIF } else { theme::BORD });
                        let id = canal.id.clone();
                        let mut muet = canal.muet;
                        if ui.toggle_value(&mut muet, "M").on_hover_text(format!("Muet : {}", canal.nom)).changed() {
                            self.modifier(|p| p.canal_mut(&id).iter_mut().for_each(|c| c.muet = muet));
                        }
                        let mut solo = canal.solo;
                        if ui.toggle_value(&mut solo, "S").on_hover_text(format!("Solo : {}", canal.nom)).changed() {
                            self.modifier(|p| p.canal_mut(&id).iter_mut().for_each(|c| c.solo = solo));
                        }
                        let mut volume = canal.volume;
                        ui.spacing_mut().slider_width = 60.0;
                        if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).on_hover_text(format!("Volume : {}", canal.nom)).changed() {
                            self.modifier(|p| p.canal_mut(&id).iter_mut().for_each(|c| c.volume = volume));
                        }
                        ui.spacing_mut().slider_width = 36.0;
                        let mut pan = canal.pan;
                        if ui.add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false)).on_hover_text(format!("Panoramique : {}", canal.nom)).changed() {
                            self.modifier(|p| p.canal_mut(&id).iter_mut().for_each(|c| c.pan = pan));
                        }
                        let mut piste = canal.piste_mixage;
                        if ui
                            .add(egui::DragValue::new(&mut piste).range(0..=nb_pistes.saturating_sub(1)))
                            .on_hover_text("Piste du mixer qui reçoit ce canal")
                            .changed()
                        {
                            self.modifier(|p| p.canal_mut(&id).iter_mut().for_each(|c| c.piste_mixage = piste));
                        }
                        // nom (sélection)
                        let selectionne = self.canal_selectionne.as_deref() == Some(canal.id.as_str());
                        let titre = if canal.batterie { format!("Batterie : {}", nom_son_batterie(canal.note_defaut)) } else { famille_ou_synthe(canal) };
                        if self.renommage.as_ref().is_some_and(|(r, _)| r == &canal.id) {
                            self.champ_renommage(ui);
                        } else {
                            let nom = egui::Button::new(RichText::new(&canal.nom).color(couleur(&canal.couleur))).selected(selectionne).truncate();
                            let reponse = ui.add_sized([LARGEUR_NOM, PAS.y], nom).on_hover_text(format!("{} : {titre}", canal.nom));
                            if reponse.clicked() || reponse.secondary_clicked() {
                                self.canal_selectionne = Some(canal.id.clone());
                            }
                            // un clic rapide de plus compte comme triple : on l'accepte aussi
                            if reponse.double_clicked() || reponse.triple_clicked() {
                                self.renommage = Some((canal.id.clone(), canal.nom.clone()));
                            }
                            reponse.context_menu(|ui| self.menu_canal(ui, canal, rang));
                        }
                        // son de batterie, si le canal est en pas
                        // colonne de largeur fixe : les grilles restent alignées d'une ligne à l'autre
                        let mut colonne = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(ui.cursor().min, Vec2::new(LARGEUR_SON, PAS.y))));
                        if canal.batterie && en_pas {
                            let ui = &mut colonne;
                            let mut note = canal.note_defaut;
                            egui::ComboBox::from_id_salt(("son", &canal.id))
                                .width(LARGEUR_SON - 8.0)
                                .truncate()
                                .selected_text(nom_son_batterie(note))
                                .show_ui(ui, |ui| {
                                    for n in 35..=81u8 {
                                        ui.selectable_value(&mut note, n, nom_son_batterie(n));
                                    }
                                });
                            if note != canal.note_defaut {
                                self.modifier(|p| definir_note_defaut(p, &id, note as i32));
                            }
                        }
                        ui.allocate_exact_size(Vec2::new(LARGEUR_SON, PAS.y), Sense::hover());
                        if en_pas {
                            self.grille(ui, canal, &velocites_des_pas(&pattern, &canal.id, canal.note_defaut), pas_courant, pointeur);
                        } else {
                            // un clic sur l'aperçu ouvre les notes dans le piano roll
                            if apercu(ui, &notes, pattern.longueur, couleur(&canal.couleur), (nb_pas as f32 * PAS.x).min(900.0)).clicked() {
                                self.canal_selectionne = Some(canal.id.clone());
                                self.montrer_panneau(crate::coquille::Panneau::PianoRoll);
                            }
                        }
                    });
                }
            },
        );
        let vue = sortie.inner_rect;

        // éditeur de vélocité du canal choisi, s'il est en pas
        let choisi = self.canal_selectionne.clone().and_then(|id| self.projet.canal(&id).cloned());
        if let Some(canal) = choisi {
            let notes = pattern.notes.get(&canal.id).cloned().unwrap_or_default();
            if est_en_mode_pas(&notes, canal.note_defaut) {
                ui.separator();
                ui.label(RichText::new(format!("Vélocité : {}", canal.nom)).small().color(theme::DISCRET));
                self.velocites(ui, &canal, &velocites_des_pas(&pattern, &canal.id, canal.note_defaut), vue);
            }
        }
    }

    /// Nom en cours de modification : Entrée valide, Échap annule.
    fn champ_renommage(&mut self, ui: &mut egui::Ui) {
        let Some((id, mut texte)) = self.renommage.take() else { return };
        let reponse = ui.add_sized([LARGEUR_NOM, PAS.y], egui::TextEdit::singleline(&mut texte).id_salt(("renommer", &id)));
        if !reponse.has_focus() && !reponse.lost_focus() {
            reponse.request_focus();
        }
        let (entree, echap) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if echap {
            return;
        }
        if entree || reponse.lost_focus() {
            self.modifier(|p| {
                renommer_canal(p, &id, &texte);
            });
            return;
        }
        self.renommage = Some((id, texte));
    }

    /// Menu d'un canal (clic droit sur son nom).
    fn menu_canal(&mut self, ui: &mut egui::Ui, canal: &projet::Canal, rang: usize) {
        let id = canal.id.clone();
        if ui.button("Ouvrir dans le piano roll").clicked() {
            self.canal_selectionne = Some(id.clone());
            self.montrer_panneau(crate::coquille::Panneau::PianoRoll);
            ui.close();
        }
        if ui.button("Renommer…").clicked() {
            self.renommage = Some((id.clone(), canal.nom.clone()));
            ui.close();
        }
        ui.menu_button("Couleur", |ui| {
            egui::Grid::new("palette").spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, teinte) in PALETTE.iter().enumerate() {
                    let (rect, reponse) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                    reponse.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Couleur {}", i + 1)));
                    ui.painter().rect_filled(rect, 3.0, couleur(teinte));
                    if canal.couleur == *teinte {
                        ui.painter().rect_stroke(rect, 3.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
                    }
                    if reponse.clicked() {
                        let id = id.clone();
                        self.modifier(|p| colorer_canal(p, &id, teinte));
                        ui.close();
                    }
                    if i % 8 == 7 {
                        ui.end_row();
                    }
                }
            });
        });
        if !canal.batterie {
            ui.menu_button("Remplacer le son", |ui| {
                ui.menu_button("Synthés intégrés", |ui| {
                    for (programme, prereglage) in PREREGLAGES_SOUSTRACTIF.iter().enumerate() {
                        if ui.button(prereglage.nom).clicked() {
                            let id = id.clone();
                            self.modifier(|p| remplacer_son(p, &id, SourceCanal::Soustractif, programme as u8));
                            ui.close();
                        }
                    }
                });
                for (f, famille) in FAMILLES.iter().enumerate() {
                    ui.menu_button(*famille, |ui| {
                        for programme in (f * 8) as u8..(f * 8 + 8) as u8 {
                            if ui.button(nom_instrument(programme)).clicked() {
                                let id = id.clone();
                                self.modifier(|p| remplacer_son(p, &id, SourceCanal::Soundfont, programme));
                                ui.close();
                            }
                        }
                    });
                }
            });
        }
        ui.separator();
        if ui.button("Cloner").clicked() {
            let mut copie = None;
            self.modifier(|p| copie = cloner_canal(p, &id));
            if copie.is_some() {
                self.canal_selectionne = copie;
            }
            ui.close();
        }
        if ui.add_enabled(rang > 0, egui::Button::new("Monter")).clicked() {
            self.modifier(|p| deplacer_canal(p, &id, rang - 1));
            ui.close();
        }
        if ui.add_enabled(rang + 1 < self.projet.canaux.len(), egui::Button::new("Descendre")).clicked() {
            self.modifier(|p| deplacer_canal(p, &id, rang + 1));
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new("Supprimer").color(theme::ACCENT)).clicked() {
            self.modifier(|p| {
                supprimer_canal(p, &id);
            });
            // la sélection passe au canal qui prend sa place (ou au précédent)
            if self.canal_selectionne.as_deref() == Some(id.as_str()) {
                let n = self.projet.canaux.len();
                self.canal_selectionne = (n > 0).then(|| self.projet.canaux[rang.min(n - 1)].id.clone());
            }
            ui.close();
        }
    }

    /// Les pas d'un canal : clic pour basculer, glisser pour peindre, clic droit pour effacer.
    fn grille(&mut self, ui: &mut egui::Ui, canal: &projet::Canal, velocites: &[u8], pas_courant: Option<usize>, pointeur: Option<Pos2>) {
        let teinte = couleur(&canal.couleur);
        let noter = self.colonnes_pas.is_empty();
        ui.spacing_mut().item_spacing.x = ECART_PAS;
        for (i, &v) in velocites.iter().enumerate() {
            if i > 0 && i % 4 == 0 {
                ui.add_space(ECART_TEMPS);
            }
            let (rect, reponse) = ui.allocate_exact_size(PAS, Sense::click_and_drag());
            if noter {
                self.colonnes_pas.push(rect.x_range());
            }
            let actif = v > 0;
            let nom = format!("Pas {} {}", i + 1, canal.nom);
            reponse.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, actif, &nom));
            // début d'une peinture : l'état voulu est l'inverse de celui du premier pas (ou
            // « éteint » au clic droit)
            if reponse.is_pointer_button_down_on() && self.peinture.is_none() {
                let gauche = ui.input(|i| i.pointer.primary_down());
                let voulu = gauche && !actif;
                self.peinture = Some(crate::rack::Peinture { canal: canal.id.clone(), actif: voulu });
                let id = canal.id.clone();
                self.modifier(|p| definir_pas(p, &id, i as u32, Some(voulu)));
            } else if let (Some(peinture), Some(pos)) = (&self.peinture, pointeur) {
                if peinture.canal == canal.id && rect.contains(pos) && actif != peinture.actif {
                    let (id, voulu) = (canal.id.clone(), peinture.actif);
                    self.modifier(|p| definir_pas(p, &id, i as u32, Some(voulu)));
                }
            }
            // clavier : Entrée ou Espace sur un pas qui a le focus
            if reponse.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let id = canal.id.clone();
                self.modifier(|p| definir_pas(p, &id, i as u32, None));
            }
            let fond = if actif {
                teinte.gamma_multiply(0.45 + (v as f32 / 127.0) * 0.55)
            } else if (i / 4) % 2 == 1 {
                theme::PANNEAU_2
            } else {
                theme::FOND
            };
            ui.painter().rect_filled(rect, 2.0, fond);
            if pas_courant == Some(i) {
                ui.painter().rect_stroke(rect.shrink(1.0), 2.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
            }
            if reponse.has_focus() {
                ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, theme::ACCENT), egui::StrokeKind::Outside);
            }
        }
    }

    /// Une barre par pas actif, sous la colonne de son pas : on règle la vélocité en cliquant ou
    /// en glissant verticalement. Les barres suivent le défilement horizontal du rack.
    fn velocites(&mut self, ui: &mut egui::Ui, canal: &projet::Canal, velocites: &[u8], vue: Rect) {
        let (zone, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HAUTEUR_VELOCITES), Sense::hover());
        if self.colonnes_pas.len() < velocites.len() {
            return; // aucune grille visible : rien à aligner
        }
        let clip = Rect::from_x_y_ranges(vue.x_range(), zone.y_range());
        let peintre = ui.painter_at(clip);
        for (i, &v) in velocites.iter().enumerate() {
            let x = self.colonnes_pas[i];
            let rect = Rect::from_x_y_ranges(x, zone.y_range());
            if !clip.intersects(rect) {
                continue;
            }
            let reponse = ui.interact(rect.intersect(clip), ui.id().with(("velocite", i)), Sense::click_and_drag());
            reponse.widget_info(|| WidgetInfo::slider(true, v as f64, format!("Vélocité du pas {}", i + 1)));
            peintre.rect_filled(rect.shrink2(Vec2::new(1.0, 0.0)), 1.0, theme::FOND);
            if v > 0 {
                let haut = rect.bottom() - rect.height() * v as f32 / 127.0;
                let barre = Rect::from_min_max(Pos2::new(rect.left() + 3.0, haut), Pos2::new(rect.right() - 3.0, rect.bottom()));
                peintre.rect_filled(barre, 1.0, couleur(&canal.couleur));
            }
            if v > 0 && (reponse.is_pointer_button_down_on() || reponse.dragged()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    let voulue = ((rect.bottom() - pos.y) / rect.height() * 127.0).round() as i32;
                    let id = canal.id.clone();
                    self.modifier(|p| definir_velocite_pas(p, &id, i as u32, voulue));
                }
            }
        }
    }
}

/// Aperçu des notes d'un canal sur toute la longueur du pattern (le piano roll les édite).
fn apercu(ui: &mut egui::Ui, notes: &[NoteMusicale], longueur: u32, teinte: Color32, largeur: f32) -> egui::Response {
    let (rect, reponse) = ui.allocate_exact_size(Vec2::new(largeur.max(40.0), PAS.y), Sense::click());
    reponse.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Aperçu des notes"));
    let reponse = reponse.on_hover_text("Ouvrir dans le piano roll");
    ui.painter().rect_filled(rect, 2.0, theme::FOND);
    let (Some(basse), Some(haute)) = (notes.iter().map(|n| n.hauteur).min(), notes.iter().map(|n| n.hauteur).max()) else { return reponse };
    let etendue = (haute - basse) as f32 + 1.0;
    let echelle = rect.width() / longueur.max(1) as f32;
    for n in notes.iter().filter(|n| n.tic < longueur) {
        let x = rect.left() + n.tic as f32 * echelle;
        let y = rect.bottom() - ((n.hauteur - basse) as f32 + 1.0) / etendue * rect.height();
        let largeur_note = (n.duree as f32 * echelle).max(1.0);
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(largeur_note, (rect.height() / etendue).max(1.5)));
        ui.painter().rect_filled(r, 0.0, teinte.gamma_multiply(0.4 + n.velocite as f32 / 127.0 * 0.6));
    }
    reponse
}
