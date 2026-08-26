use crate::catalog::{self, MediaItem, SearchQuery};
use crate::config::{self, AppConfig};
use crate::organize::{self, PlanFile, TransferOp};
use crate::paths::is_organized_path;
use crate::scan::{hash_file, walk_storage};
use crate::sync::{self, ExternalFile, LibraryIndex};
use crate::transfer;
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, State};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
}

fn parse_date(captured_at: &str) -> NaiveDate {
    NaiveDate::parse_from_str(&captured_at.chars().take(10).collect::<String>(), "%Y-%m-%d")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}

fn existing_rel_paths(root: &Path, files: &[MediaItem]) -> HashSet<PathBuf> {
    files
        .iter()
        .filter_map(|item| {
            Path::new(&item.path)
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_path_buf())
        })
        .collect()
}

fn absolute_ops(root: &Path, mut ops: Vec<TransferOp>) -> Vec<TransferOp> {
    for op in &mut ops {
        let dest = Path::new(&op.destination);
        if !dest.is_absolute() {
            op.destination = root.join(dest).to_string_lossy().into_owned();
        }
    }
    ops
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    Ok(state.config.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub fn save_config(state: State<AppState>, config: AppConfig) -> Result<AppConfig, String> {
    config.validate().map_err(String::from)?;
    let path = state.config_path.clone();
    config::save_config(&path, &config).map_err(String::from)?;
    *state.config.lock().map_err(|e| e.to_string())? = config.clone();
    Ok(config)
}

#[tauri::command]
pub fn scan_library(state: State<AppState>) -> Result<Vec<MediaItem>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    for storage in &config.storages {
        let items = walk_storage(storage).map_err(String::from)?;
        for item in items {
            catalog::upsert_media(&db, &item).map_err(String::from)?;
            all.push(item);
        }
    }
    catalog::list_all(&db).map_err(String::from)
}

#[tauri::command]
pub fn search_media(state: State<AppState>, query: SearchQuery) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    catalog::search(&db, &query).map_err(String::from)
}

#[tauri::command]
pub fn list_tags(state: State<AppState>) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    catalog::list_tags(&db).map_err(String::from)
}

#[tauri::command]
pub fn set_media_tags(
    state: State<AppState>,
    media_id: i64,
    tags: Vec<String>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    catalog::set_tags(&db, media_id, &tags).map_err(String::from)
}

#[tauri::command]
pub fn preview_organize(
    state: State<AppState>,
    storage_id: Option<String>,
) -> Result<Vec<TransferOp>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let items = catalog::list_all(&db).map_err(String::from)?;
    let mut ops = Vec::new();
    for storage in &config.storages {
        if let Some(id) = &storage_id {
            if &storage.id != id {
                continue;
            }
        }
        let root = PathBuf::from(&storage.path);
        let files: Vec<PlanFile> = items
            .iter()
            .filter(|item| item.storage_id == storage.id)
            .map(|item| PlanFile {
                absolute_path: PathBuf::from(&item.path),
                relative_path: Path::new(&item.path)
                    .strip_prefix(&root)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| PathBuf::from(&item.filename)),
                filename: item.filename.clone(),
                kind: item.kind,
                captured_on: parse_date(&item.captured_at),
                date_source: item.date_source,
                size: item.size as u64,
                hash: item.hash.clone(),
            })
            .collect();
        let existing = existing_rel_paths(&root, &items);
        ops.extend(absolute_ops(&root, organize::plan_organize(&files, &existing)));
    }
    Ok(ops)
}

#[tauri::command]
pub fn execute_organize(
    state: State<AppState>,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = config
        .storages
        .iter()
        .find(|s| s.id == storage_id)
        .ok_or_else(|| "Unknown storage".to_string())?;
    let root = PathBuf::from(&storage.path);
    let destinations = transfer::execute_ops(&root, &operations).map_err(String::from)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    for (op, dest) in operations.iter().zip(destinations.iter()) {
        catalog::update_path(&db, &op.source, dest, is_organized_path(Path::new(
            Path::new(dest).strip_prefix(&root).unwrap_or(Path::new(dest)),
        )))
        .map_err(String::from)?;
    }
    Ok(destinations)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewRequest {
    pub source_path: String,
    pub storage_id: String,
}

#[tauri::command]
pub fn preview_sync(state: State<AppState>, request: SyncPreviewRequest) -> Result<Vec<TransferOp>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = config
        .storages
        .iter()
        .find(|s| s.id == request.storage_id)
        .ok_or_else(|| "Unknown storage".to_string())?
        .clone();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let records = catalog::library_records(&db).map_err(String::from)?;
    let library = LibraryIndex::from_records(records);
    let fake_source = crate::config::Storage {
        id: "external".into(),
        name: "External".into(),
        path: request.source_path.clone(),
        kind: crate::config::StorageKind::External,
    };
    let scanned = walk_storage(&fake_source).map_err(String::from)?;
    let files: Vec<ExternalFile> = scanned
        .into_iter()
        .map(|item| {
            let hash = item.hash.or_else(|| hash_file(Path::new(&item.path)).ok());
            ExternalFile {
                absolute_path: PathBuf::from(&item.path),
                filename: item.filename,
                kind: item.kind,
                captured_on: parse_date(&item.captured_at),
                size: item.size as u64,
                hash,
            }
        })
        .collect();
    let items = catalog::list_all(&db).map_err(String::from)?;
    let root = PathBuf::from(&storage.path);
    let existing = existing_rel_paths(&root, &items);
    Ok(absolute_ops(
        &root,
        sync::plan_sync(&files, &library, &existing),
    ))
}

#[tauri::command]
pub fn execute_sync(
    state: State<AppState>,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = config
        .storages
        .iter()
        .find(|s| s.id == storage_id)
        .cloned()
        .ok_or_else(|| "Unknown storage".to_string())?;
    let root = PathBuf::from(&storage.path);
    let destinations = transfer::execute_ops(&root, &operations).map_err(String::from)?;
    drop(state.db.lock().map_err(|e| e.to_string())?);
    let _ = storage;
    Ok(destinations)
}

#[tauri::command]
pub fn app_ready(_app: AppHandle) -> Result<bool, String> {
    Ok(true)
}
