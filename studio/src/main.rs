// Pas de fenêtre de console sous Windows en version publiée.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use studio::Studio;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("song-audio")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 640.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../src-tauri/icons/128x128.png")).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "song-audio",
        options,
        Box::new(|cc| {
            studio::app::appliquer_theme(&cc.egui_ctx);
            let mut studio = Studio::depuis_disque();
            studio.reprendre_disposition(cc.storage);
            Ok(Box::new(studio))
        }),
    )
}
