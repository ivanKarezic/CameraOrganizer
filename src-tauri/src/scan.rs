use crate::camera::identify_camera;
use crate::catalog::MediaItem;
use crate::config::Storage;
use crate::error::AppResult;
use crate::media::{classify_filename, should_skip_name};
use crate::metadata::{resolve_capture_time, FileHints};
use crate::paths::is_organized_path;
use chrono::{Local, NaiveDateTime};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn hash_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn mtime_of(path: &Path) -> Option<NaiveDateTime> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(chrono::DateTime::<Local>::from(modified).naive_local())
}

fn location_label(lat: Option<f64>, lon: Option<f64>) -> Option<String> {
    match (lat, lon) {
        (Some(lat), Some(lon)) => Some(format!("{lat:.5}, {lon:.5}")),
        _ => None,
    }
}

pub fn walk_storage(storage: &Storage) -> AppResult<Vec<MediaItem>> {
    let root = PathBuf::from(&storage.path);
    if !root.exists() {
        return Err(crate::error::AppError::msg(format!(
            "Storage '{}' does not exist: {}",
            storage.name, storage.path
        )));
    }

    let mut items = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_name(&name) {
            continue;
        }
        let Some(kind) = classify_filename(&name) else {
            continue;
        };
        let path = entry.path();
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let camera = identify_camera(&name);
        let meta = std::fs::metadata(path)?;
        let hints = FileHints {
            mtime: mtime_of(path),
            ..Default::default()
        };
        let resolved = resolve_capture_time(&name, &hints);
        items.push(MediaItem {
            id: 0,
            storage_id: storage.id.clone(),
            path: path.to_string_lossy().into_owned(),
            filename: name,
            size: meta.len() as i64,
            hash: None,
            camera,
            kind,
            captured_at: resolved.captured_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            date_source: resolved.source,
            latitude: hints.gps.map(|g| g.0),
            longitude: hints.gps.map(|g| g.1),
            location_label: location_label(hints.gps.map(|g| g.0), hints.gps.map(|g| g.1)),
            organized: is_organized_path(rel),
            tags: Vec::new(),
        });
    }
    items.sort_by(|a, b| b.captured_at.cmp(&a.captured_at).then(a.filename.cmp(&b.filename)));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, StorageKind};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn walks_only_media_and_flags_unorganized() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("inbox")).unwrap();
        fs::create_dir_all(dir.path().join("2024/2024-08-26/Photo")).unwrap();
        fs::write(dir.path().join("inbox/DJI_20240826_143022_000_0001.MP4"), b"vid").unwrap();
        fs::write(dir.path().join("2024/2024-08-26/Photo/DJI_0001.JPG"), b"pic").unwrap();
        fs::write(dir.path().join("inbox/notes.txt"), b"nope").unwrap();

        let storage = AppConfig::new_storage(
            "Main".into(),
            dir.path().to_string_lossy().into_owned(),
            StorageKind::Local,
        );
        let items = walk_storage(&storage).unwrap();
        assert_eq!(items.len(), 2);
        let video = items.iter().find(|i| i.filename.ends_with(".MP4")).unwrap();
        let photo = items.iter().find(|i| i.filename.ends_with(".JPG")).unwrap();
        assert!(!video.organized);
        assert!(photo.organized);
        assert_eq!(video.camera.label(), "DJI");
    }
}
