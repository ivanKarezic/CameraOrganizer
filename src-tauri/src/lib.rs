mod camera;
mod camorg;
mod catalog;
mod commands;
mod config;
mod error;
mod locations;
mod media;
mod metadata;
mod organize;
mod paths;
mod progress;
mod scan;
mod sync;
mod tags;
mod thumbnails;
mod transfer;

use commands::AppState;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            fs::create_dir_all(&config_dir)?;
            let config_path = config::config_path_from(&config_dir);
            let config = config::load_or_default(&config_path)?;
            app.manage(AppState {
                config: Mutex::new(config),
                config_path,
                tags_path: crate::tags::tags_path_from(&config_dir),
                locations_path: crate::locations::locations_path_from(&config_dir),
                preload_epoch: Arc::new(AtomicU64::new(0)),
                app_started: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::scan_library,
            commands::search_media,
            commands::list_tags,
            commands::list_tag_categories,
            commands::save_tag_category,
            commands::delete_tag_category,
            commands::save_global_tag,
            commands::delete_global_tag,
            commands::set_media_tags,
            commands::list_locations,
            commands::save_location,
            commands::delete_location,
            commands::set_media_location,
            commands::delete_media,
            commands::delete_media_batch,
            commands::ensure_thumbnail,
            commands::preload_thumbnails,
            commands::preview_organize,
            commands::execute_organize,
            commands::preview_sync,
            commands::execute_sync,
            commands::app_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
