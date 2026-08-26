mod camera;
mod catalog;
mod commands;
mod config;
mod error;
mod media;
mod metadata;
mod organize;
mod paths;
mod scan;
mod sync;
mod transfer;

use commands::AppState;
use std::fs;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let config_dir = app.path().app_config_dir()?;
            fs::create_dir_all(&config_dir)?;
            let config_path = config::config_path_from(&config_dir);
            let config = config::load_or_default(&config_path)?;
            let db = catalog::open(&data_dir.join("catalog.sqlite"))?;
            app.manage(AppState {
                db: Mutex::new(db),
                config: Mutex::new(config),
                config_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::scan_library,
            commands::search_media,
            commands::list_tags,
            commands::set_media_tags,
            commands::preview_organize,
            commands::execute_organize,
            commands::preview_sync,
            commands::execute_sync,
            commands::app_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
