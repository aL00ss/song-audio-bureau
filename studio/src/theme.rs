//! Habillage du studio : sombre, contrasté, accent orange (comme la version web, sans copier
//! aucun logiciel commercial).

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

pub const ENREGISTREMENT: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const FOND: Color32 = Color32::from_rgb(0x1b, 0x1e, 0x23);
pub const PANNEAU: Color32 = Color32::from_rgb(0x26, 0x2b, 0x32);
pub const PANNEAU_2: Color32 = Color32::from_rgb(0x30, 0x36, 0x3e);
pub const BORD: Color32 = Color32::from_rgb(0x3d, 0x44, 0x4d);
pub const TEXTE: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xeb);
pub const DISCRET: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xad);
pub const ACCENT: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
pub const ACTIF: Color32 = Color32::from_rgb(0x4c, 0xc3, 0x8a);
pub const ECRAN: Color32 = Color32::from_rgb(0x0d, 0x10, 0x13);

pub fn appliquer(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANNEAU;
    v.window_fill = PANNEAU;
    v.extreme_bg_color = ECRAN;
    v.faint_bg_color = PANNEAU_2;
    v.override_text_color = Some(TEXTE);
    v.selection.bg_fill = ACCENT.linear_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(3);
    }
    v.widgets.inactive.weak_bg_fill = PANNEAU_2;
    v.widgets.inactive.bg_fill = PANNEAU_2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORD);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, DISCRET);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORD);
    ctx.set_visuals(v);
}
