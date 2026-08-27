use crate::error::AppResult;
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR_NAME: &str = "CamOrg";
pub const THUMBS_DIR: &str = "Thumbnails";
pub const CATALOG_FILE: &str = "catalog.sqlite";

#[derive(Debug, Clone)]
pub struct CamOrgPaths {
    pub catalog: PathBuf,
}

pub fn dir(storage_root: &Path) -> PathBuf {
    storage_root.join(DIR_NAME)
}

pub fn catalog_path(storage_root: &Path) -> PathBuf {
    dir(storage_root).join(CATALOG_FILE)
}

pub fn thumbnails_dir(storage_root: &Path) -> PathBuf {
    dir(storage_root).join(THUMBS_DIR)
}

pub fn ensure_layout(storage_root: &Path) -> AppResult<CamOrgPaths> {
    std::fs::create_dir_all(thumbnails_dir(storage_root))?;
    Ok(CamOrgPaths {
        catalog: catalog_path(storage_root),
    })
}

pub fn ensure_if_online(storage_root: &Path) -> AppResult<Option<CamOrgPaths>> {
    if !storage_root.exists() {
        return Ok(None);
    }
    Ok(Some(ensure_layout(storage_root)?))
}

pub fn should_descend(entry: &walkdir::DirEntry) -> bool {
    entry.depth() == 0 || entry.file_name() != DIR_NAME
}

pub fn relative_key(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

pub fn thumbnail_rel(rel: &Path) -> PathBuf {
    PathBuf::from(THUMBS_DIR).join(format!("{}.jpg", blake3::hash(relative_key(rel).as_bytes()).to_hex()))
}

pub fn thumbnail_abs(storage_root: &Path, rel: &Path) -> PathBuf {
    dir(storage_root).join(thumbnail_rel(rel))
}

pub fn relocate_thumbnail(
    storage_root: &Path,
    old_rel: &Path,
    new_rel: &Path,
) -> AppResult<Option<PathBuf>> {
    let from = thumbnail_abs(storage_root, old_rel);
    let to_rel = thumbnail_rel(new_rel);
    let to = dir(storage_root).join(&to_rel);
    if !from.exists() {
        return Ok(None);
    }
    if from == to {
        return Ok(Some(to_rel));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(&from, &to) {
        Ok(()) => {}
        Err(_) => {
            fs::copy(&from, &to)?;
            fs::remove_file(&from)?;
        }
    }
    Ok(Some(to_rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn creates_camorg_layout_once() {
        let dir = tempdir().unwrap();
        let paths = ensure_layout(dir.path()).unwrap();
        assert!(paths.catalog.ends_with("catalog.sqlite"));
        assert!(thumbnails_dir(dir.path()).is_dir());
        ensure_layout(dir.path()).unwrap();
        assert!(super::dir(dir.path()).is_dir());
    }

    #[test]
    fn skips_offline_storage() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("gone");
        assert!(ensure_if_online(&missing).unwrap().is_none());
    }

    #[test]
    fn thumbnail_names_are_stable_and_relocate() {
        let dir = tempdir().unwrap();
        ensure_layout(dir.path()).unwrap();
        let old = Path::new("inbox/DJI_0001.JPG");
        let new = Path::new("2024/2024-08-26/Photo/DJI_0001.JPG");
        let from = thumbnail_abs(dir.path(), old);
        fs::write(&from, b"thumb").unwrap();
        let moved = relocate_thumbnail(dir.path(), old, new).unwrap().unwrap();
        assert!(!from.exists());
        assert!(dir.path().join("CamOrg").join(&moved).exists());
        assert_eq!(thumbnail_rel(old), thumbnail_rel(Path::new("inbox/DJI_0001.JPG")));
        assert_ne!(thumbnail_rel(old), thumbnail_rel(new));
    }
}
