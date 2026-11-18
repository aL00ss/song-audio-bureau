//! song-audio, version de bureau : la fenêtre Tauri et, à venir, le moteur sonore en Rust.

/// Version de l'application, lisible depuis l'interface (vérifie que le pont Tauri répond).
#[tauri::command]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn lancer() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![version])
        .run(tauri::generate_context!())
        .expect("échec du lancement de l'application");
}
