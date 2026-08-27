use crate::camorg;
use crate::catalog::{self, AssignedTag, MediaItem, SearchQuery};
use crate::config::{self, AppConfig, Storage};
use crate::media::{classify_filename, file_stem, MediaKind};
use crate::organize::{self, PlanFile, TransferOp};
use crate::paths::is_organized_path;
use crate::progress;
use crate::scan::{hash_file, walk_storage, walk_storage_with_progress};
use crate::sync::{self, ExternalFile, LibraryIndex};
use crate::locations::{self, SavedLocation};
use crate::tags::{self, GlobalTag, TagCategory, TagStore};
use crate::thumbnails;
use crate::transfer;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, State};

const PRELOAD_YIELD: Duration = Duration::from_millis(30);

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
    pub tags_path: PathBuf,
    pub locations_path: PathBuf,
    pub preload_epoch: Arc<AtomicU64>,
    pub app_started: Arc<AtomicBool>,
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

fn open_storage_db(storage: &Storage) -> Result<Connection, String> {
    let root = Path::new(&storage.path);
    if !root.exists() {
        return Err(format!(
            "Storage '{}' is offline: {}",
            storage.name, storage.path
        ));
    }
    let catalog = camorg::catalog_path(root);
    if !catalog.exists() {
        camorg::ensure_layout(root).map_err(String::from)?;
    }
    catalog::open(&catalog).map_err(String::from)
}

fn storage_by_id<'a>(config: &'a AppConfig, storage_id: &str) -> Result<&'a Storage, String> {
    config
        .storages
        .iter()
        .find(|s| s.id == storage_id)
        .ok_or_else(|| "Unknown storage".to_string())
}

fn collect_library(config: &AppConfig, query: &SearchQuery) -> Result<Vec<MediaItem>, String> {
    let mut items = Vec::new();
    for storage in &config.storages {
        let root = PathBuf::from(&storage.path);
        if !root.exists() {
            continue;
        }
        let db = open_storage_db(storage)?;
        items.extend(catalog::search(&db, &root, query).map_err(String::from)?);
    }
    items.sort_by(|a, b| b.captured_at.cmp(&a.captured_at).then(a.filename.cmp(&b.filename)));
    Ok(items)
}

fn paint_tags(tags: &mut [AssignedTag], store: &TagStore) {
    for tag in tags {
        if let Some(global) = tags::tag_by_name(store, &tag.name) {
            tag.color = global.color.clone();
            tag.category_id = global.category_id.clone();
            tag.category_name = global.category_name.clone();
        } else {
            tag.color = tags::color_for(store, &tag.name);
        }
    }
}

fn paint_items(items: &mut [MediaItem], store: &TagStore) {
    for item in items {
        paint_tags(&mut item.tags, store);
    }
}

fn load_globals(state: &AppState) -> Result<TagStore, String> {
    tags::load(&state.tags_path).map_err(String::from)
}

fn save_globals(state: &AppState, store: &TagStore) -> Result<(), String> {
    tags::save(&state.tags_path, store).map_err(String::from)
}

fn load_locations(state: &AppState) -> Result<locations::LocationStore, String> {
    locations::load(&state.locations_path).map_err(String::from)
}

fn save_locations(state: &AppState, store: &locations::LocationStore) -> Result<(), String> {
    locations::save(&state.locations_path, store).map_err(String::from)
}

fn for_each_online_db(config: &AppConfig, mut visit: impl FnMut(&Storage, &Connection) -> Result<(), String>) -> Result<(), String> {
    for storage in &config.storages {
        if !Path::new(&storage.path).exists() {
            continue;
        }
        let db = open_storage_db(storage)?;
        visit(storage, &db)?;
    }
    Ok(())
}

fn delete_media_on_disk(storage: &Storage, item: &MediaItem) {
    let root = Path::new(&storage.path);
    let rel = relative_to(root, &item.path);
    let thumb = camorg::thumbnail_abs(root, &rel);
    let _ = std::fs::remove_file(thumb);
    let path = Path::new(&item.path);
    if let (Some(dir), Some(stem)) = (path.parent(), path.file_stem().and_then(|s| s.to_str())) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if file_stem(&name) == stem && matches!(classify_filename(&name), Some(MediaKind::Sidecar)) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn ensure_online_camorg(config: &AppConfig) {
    for storage in &config.storages {
        if let Ok(Some(paths)) = camorg::ensure_if_online(Path::new(&storage.path)) {
            let _ = catalog::open(&paths.catalog);
        }
    }
}

fn relative_to<'a>(root: &'a Path, path: &'a str) -> PathBuf {
    Path::new(path)
        .strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| PathBuf::from(Path::new(path).file_name().unwrap_or_default()))
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    Ok(config)
}

#[tauri::command]
pub fn save_config(state: State<AppState>, config: AppConfig) -> Result<AppConfig, String> {
    config.validate().map_err(String::from)?;
    let path = state.config_path.clone();
    config::save_config(&path, &config).map_err(String::from)?;
    ensure_online_camorg(&config);
    *state.config.lock().map_err(|e| e.to_string())? = config.clone();
    Ok(config)
}

#[tauri::command]
pub async fn scan_library(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<MediaItem>, String> {
    state.preload_epoch.fetch_add(1, Ordering::SeqCst);
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let tags_path = state.tags_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut items = run_scan(app, config)?;
        let globals = tags::load(&tags_path).map_err(String::from)?;
        paint_items(&mut items, &globals);
        Ok(items)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run_scan(app: AppHandle, config: AppConfig) -> Result<Vec<MediaItem>, String> {
    let mut all = Vec::new();
    for storage in &config.storages {
        let root = PathBuf::from(&storage.path);
        if !root.exists() {
            continue;
        }
        camorg::ensure_layout(&root).map_err(String::from)?;
        progress::emit(
            &app,
            "scan",
            0,
            0,
            "",
            &format!("Scanning {}…", storage.name),
        );
        let walked = walk_storage_with_progress(storage, |count, filename| {
            progress::emit(
                &app,
                "scan",
                count,
                0,
                filename,
                &format!("Finding files in {}…", storage.name),
            );
        })
        .map_err(String::from)?;
        let db = catalog::open(&camorg::catalog_path(&root)).map_err(String::from)?;
        let total = walked.len() as u64;
        for (index, item) in walked.iter().enumerate() {
            progress::emit(
                &app,
                "scan",
                index as u64 + 1,
                total,
                &item.filename,
                "Indexing files",
            );
            catalog::upsert_media(&db, item).map_err(String::from)?;
        }
        all.extend(catalog::list_all(&db, &root).map_err(String::from)?);
    }
    all.sort_by(|a, b| b.captured_at.cmp(&a.captured_at).then(a.filename.cmp(&b.filename)));
    Ok(all)
}

#[tauri::command]
pub async fn search_media(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> Result<Vec<MediaItem>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let tags_path = state.tags_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut items = collect_library(&config, &query)?;
        let globals = tags::load(&tags_path).map_err(String::from)?;
        paint_items(&mut items, &globals);
        Ok(items)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_tags(state: State<AppState>) -> Result<Vec<GlobalTag>, String> {
    Ok(load_globals(&state)?.tags)
}

#[tauri::command]
pub fn list_tag_categories(state: State<AppState>) -> Result<Vec<TagCategory>, String> {
    Ok(load_globals(&state)?.categories)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCategoryInput {
    pub id: Option<String>,
    pub name: String,
    pub color: String,
}

#[tauri::command]
pub fn save_tag_category(state: State<AppState>, category: TagCategoryInput) -> Result<TagCategory, String> {
    let mut store = load_globals(&state)?;
    let saved = tags::upsert_category(
        &mut store,
        category.id.as_deref(),
        &category.name,
        &category.color,
    )
    .map_err(String::from)?;
    save_globals(&state, &store)?;
    Ok(saved)
}

#[tauri::command]
pub fn delete_tag_category(state: State<AppState>, category_id: String) -> Result<(), String> {
    let mut store = load_globals(&state)?;
    tags::remove_category(&mut store, &category_id).map_err(String::from)?;
    save_globals(&state, &store)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalTagInput {
    pub id: Option<String>,
    pub name: String,
    pub category_id: String,
}

#[tauri::command]
pub fn save_global_tag(state: State<AppState>, tag: GlobalTagInput) -> Result<GlobalTag, String> {
    let mut store = load_globals(&state)?;
    let previous = tag
        .id
        .as_ref()
        .and_then(|id| store.tags.iter().find(|t| &t.id == id).cloned());
    let saved =
        tags::upsert_tag(&mut store, tag.id.as_deref(), &tag.name, &tag.category_id).map_err(String::from)?;
    if let Some(prev) = previous {
        if !prev.name.eq_ignore_ascii_case(&saved.name) {
            let config = state.config.lock().map_err(|e| e.to_string())?.clone();
            for_each_online_db(&config, |_, db| {
                catalog::rename_tag_assignments(db, &prev.name, &saved.name).map_err(String::from)
            })?;
        }
    }
    save_globals(&state, &store)?;
    Ok(saved)
}

#[tauri::command]
pub fn delete_global_tag(state: State<AppState>, tag_id: String) -> Result<(), String> {
    let mut store = load_globals(&state)?;
    let removed = tags::remove_tag(&mut store, &tag_id).map_err(String::from)?;
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    for_each_online_db(&config, |_, db| {
        catalog::delete_tag_assignments(db, &removed.name).map_err(String::from)
    })?;
    save_globals(&state, &store)?;
    Ok(())
}

#[tauri::command]
pub fn set_media_tags(
    state: State<AppState>,
    storage_id: String,
    media_id: i64,
    tags: Vec<String>,
) -> Result<Vec<AssignedTag>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = storage_by_id(&config, &storage_id)?;
    let db = open_storage_db(storage)?;
    let mut assigned = catalog::set_tags(&db, media_id, &tags).map_err(String::from)?;
    let globals = load_globals(&state)?;
    paint_tags(&mut assigned, &globals);
    Ok(assigned)
}

#[tauri::command]
pub fn list_locations(state: State<AppState>) -> Result<Vec<SavedLocation>, String> {
    Ok(load_locations(&state)?.locations)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInput {
    pub id: Option<String>,
    pub name: String,
}

#[tauri::command]
pub fn save_location(state: State<AppState>, location: LocationInput) -> Result<SavedLocation, String> {
    let mut store = load_locations(&state)?;
    let previous = location
        .id
        .as_ref()
        .and_then(|id| store.locations.iter().find(|item| &item.id == id).cloned());
    let saved = locations::upsert(&mut store, location.id.as_deref(), &location.name).map_err(String::from)?;
    if let Some(prev) = previous {
        if !prev.name.eq_ignore_ascii_case(&saved.name) {
            let config = state.config.lock().map_err(|e| e.to_string())?.clone();
            for_each_online_db(&config, |_, db| {
                catalog::rename_location_assignments(db, &prev.name, &saved.name).map_err(String::from)
            })?;
        }
    }
    save_locations(&state, &store)?;
    Ok(saved)
}

#[tauri::command]
pub fn delete_location(state: State<AppState>, location_id: String) -> Result<(), String> {
    let mut store = load_locations(&state)?;
    locations::remove(&mut store, &location_id).map_err(String::from)?;
    save_locations(&state, &store)?;
    Ok(())
}

#[tauri::command]
pub fn set_media_location(
    state: State<AppState>,
    storage_id: String,
    media_id: i64,
    location: Option<String>,
) -> Result<Option<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = storage_by_id(&config, &storage_id)?;
    let db = open_storage_db(storage)?;
    let saved = catalog::set_location(&db, media_id, location.as_deref()).map_err(String::from)?;
    if let Some(name) = saved.as_deref() {
        let mut store = load_locations(&state)?;
        locations::upsert(&mut store, None, name).map_err(String::from)?;
        save_locations(&state, &store)?;
    }
    Ok(saved)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    pub storage_id: String,
    pub media_id: i64,
}

#[tauri::command]
pub fn delete_media(state: State<AppState>, storage_id: String, media_id: i64) -> Result<(), String> {
    delete_media_batch(state, vec![MediaRef { storage_id, media_id }])
}

#[tauri::command]
pub fn delete_media_batch(state: State<AppState>, items: Vec<MediaRef>) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    for item_ref in items {
        let storage = storage_by_id(&config, &item_ref.storage_id)?.clone();
        let root = PathBuf::from(&storage.path);
        let db = open_storage_db(&storage)?;
        let Some(item) = catalog::get_by_id(&db, &root, item_ref.media_id).map_err(String::from)? else {
            continue;
        };
        delete_media_on_disk(&storage, &item);
        catalog::delete_id(&db, item_ref.media_id).map_err(String::from)?;
    }
    Ok(())
}

#[tauri::command]
pub fn ensure_thumbnail(
    state: State<AppState>,
    storage_id: String,
    media_id: i64,
) -> Result<Option<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let storage = storage_by_id(&config, &storage_id)?.clone();
    let root = PathBuf::from(&storage.path);
    let db = open_storage_db(&storage)?;
    let item = catalog::get_by_id(&db, &root, media_id)
        .map_err(String::from)?
        .ok_or_else(|| "Unknown media".to_string())?;
    if let Some(path) = item.thumbnail_path.clone() {
        return Ok(Some(path));
    }
    let rel = relative_to(&root, &item.path);
    let Some(thumb_rel) = thumbnails::ensure_thumbnail(&root, Path::new(&item.path), &rel, item.kind)
        .map_err(String::from)?
    else {
        return Ok(None);
    };
    catalog::set_thumbnail_rel(&db, &item.path, &thumb_rel.to_string_lossy()).map_err(String::from)?;
    Ok(Some(
        camorg::dir(&root)
            .join(thumb_rel)
            .to_string_lossy()
            .into_owned(),
    ))
}

fn thumb_kind_rank(kind: MediaKind) -> u8 {
    match kind {
        MediaKind::Photo => 0,
        MediaKind::Video => 1,
        MediaKind::Sidecar => 2,
    }
}

#[tauri::command]
pub async fn preload_thumbnails(app: AppHandle, state: State<'_, AppState>) -> Result<u64, String> {
    if !state.app_started.load(Ordering::SeqCst) {
        return Ok(0);
    }
    let epoch = state.preload_epoch.fetch_add(1, Ordering::SeqCst) + 1;
    let epoch_flag = Arc::clone(&state.preload_epoch);
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if epoch_flag.load(Ordering::SeqCst) != epoch {
            return Ok(0);
        }
        let mut dbs: HashMap<String, (PathBuf, Connection)> = HashMap::new();
        let mut missing = Vec::new();
        let mut had_media = false;
        for storage in &config.storages {
            let root = PathBuf::from(&storage.path);
            if !root.exists() {
                continue;
            }
            let Ok(db) = open_storage_db(storage) else {
                continue;
            };
            if !had_media {
                had_media = db
                    .query_row(
                        "SELECT 1 FROM media WHERE kind != 'sidecar' LIMIT 1",
                        [],
                        |_| Ok(()),
                    )
                    .optional()
                    .ok()
                    .flatten()
                    .is_some();
            }
            if let Ok(rows) = catalog::list_missing_thumbnails(&db) {
                missing.extend(rows);
            }
            dbs.insert(storage.id.clone(), (root, db));
        }
        missing.sort_by(|a, b| {
            thumb_kind_rank(a.kind)
                .cmp(&thumb_kind_rank(b.kind))
                .then(b.captured_at.cmp(&a.captured_at))
                .then(a.filename.cmp(&b.filename))
        });
        let total = missing.len() as u64;
        if total == 0 {
            if had_media {
                progress::emit(&app, "preload", 0, 0, "", "Thumbnails ready");
            }
            return Ok(0);
        }
        progress::emit(&app, "preload", 0, total, "", "Preloading thumbnails");
        let mut done = 0u64;
        for item in missing {
            if epoch_flag.load(Ordering::SeqCst) != epoch {
                return Ok(done);
            }
            thread::sleep(PRELOAD_YIELD);
            if epoch_flag.load(Ordering::SeqCst) != epoch {
                return Ok(done);
            }
            let Some((root, db)) = dbs.get(&item.storage_id) else {
                continue;
            };
            progress::emit(
                &app,
                "preload",
                done + 1,
                total,
                &item.filename,
                "Preloading thumbnails",
            );
            let rel = relative_to(root, &item.path);
            if let Ok(Some(thumb_rel)) =
                thumbnails::ensure_thumbnail(root, Path::new(&item.path), &rel, item.kind)
            {
                let _ = catalog::set_thumbnail_rel(db, &item.path, &thumb_rel.to_string_lossy());
                let path = camorg::dir(root)
                    .join(&thumb_rel)
                    .to_string_lossy()
                    .into_owned();
                progress::emit_thumb_ready(
                    &app,
                    &progress::ThumbnailReady {
                        storage_id: item.storage_id.clone(),
                        media_id: item.id,
                        path,
                    },
                );
            }
            done += 1;
        }
        if epoch_flag.load(Ordering::SeqCst) == epoch {
            progress::emit(&app, "preload", total, total, "", "Thumbnails ready");
        }
        Ok(done)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn preview_organize(
    state: State<AppState>,
    storage_id: Option<String>,
) -> Result<Vec<TransferOp>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let items = collect_library(&config, &SearchQuery::default())?;
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
                relative_path: relative_to(&root, &item.path),
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
pub async fn execute_organize(
    app: AppHandle,
    state: State<'_, AppState>,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    state.preload_epoch.fetch_add(1, Ordering::SeqCst);
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tauri::async_runtime::spawn_blocking(move || run_organize(app, config, storage_id, operations))
        .await
        .map_err(|e| e.to_string())?
}

fn run_organize(
    app: AppHandle,
    config: AppConfig,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    let storage = storage_by_id(&config, &storage_id)?.clone();
    let root = PathBuf::from(&storage.path);
    progress::emit(&app, "organize", 0, operations.len() as u64, "", "Moving files");
    let destinations = transfer::execute_ops_with_progress(&root, &operations, |current, total, filename| {
        progress::emit(
            &app,
            "organize",
            current as u64,
            total as u64,
            filename,
            "Moving files",
        );
    })
    .map_err(String::from)?;
    let db = open_storage_db(&storage)?;
    for (op, dest) in operations.iter().zip(destinations.iter()) {
        let old_rel = relative_to(&root, &op.source);
        let new_rel = relative_to(&root, dest);
        let thumb_rel = camorg::relocate_thumbnail(&root, &old_rel, &new_rel).map_err(String::from)?;
        catalog::update_path(
            &db,
            &op.source,
            dest,
            is_organized_path(Path::new(
                Path::new(dest)
                    .strip_prefix(&root)
                    .unwrap_or(Path::new(dest)),
            )),
        )
        .map_err(String::from)?;
        if let Some(thumb_rel) = thumb_rel {
            catalog::set_thumbnail_rel(&db, dest, &thumb_rel.to_string_lossy()).map_err(String::from)?;
        }
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
    let storage = storage_by_id(&config, &request.storage_id)?.clone();
    let db = open_storage_db(&storage)?;
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
    let items = catalog::list_all(&db, Path::new(&storage.path)).map_err(String::from)?;
    let root = PathBuf::from(&storage.path);
    let existing = existing_rel_paths(&root, &items);
    Ok(absolute_ops(
        &root,
        sync::plan_sync(&files, &library, &existing),
    ))
}

#[tauri::command]
pub async fn execute_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    state.preload_epoch.fetch_add(1, Ordering::SeqCst);
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tauri::async_runtime::spawn_blocking(move || run_sync(app, config, storage_id, operations))
        .await
        .map_err(|e| e.to_string())?
}

fn run_sync(
    app: AppHandle,
    config: AppConfig,
    storage_id: String,
    operations: Vec<TransferOp>,
) -> Result<Vec<String>, String> {
    let storage = storage_by_id(&config, &storage_id)?.clone();
    let root = PathBuf::from(&storage.path);
    camorg::ensure_layout(&root).map_err(String::from)?;
    progress::emit(&app, "import", 0, operations.len() as u64, "", "Copying files");
    transfer::execute_ops_with_progress(&root, &operations, |current, total, filename| {
        progress::emit(
            &app,
            "import",
            current as u64,
            total as u64,
            filename,
            "Copying files",
        );
    })
    .map_err(String::from)
}

#[tauri::command]
pub fn app_ready(state: State<AppState>) -> Result<bool, String> {
    state.app_started.store(true, Ordering::SeqCst);
    Ok(true)
}
