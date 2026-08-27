use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedLocation {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocationStore {
    #[serde(default)]
    pub locations: Vec<SavedLocation>,
}

pub fn locations_path_from(config_dir: &Path) -> PathBuf {
    config_dir.join("locations.json")
}

pub fn load(path: &Path) -> AppResult<LocationStore> {
    if !path.exists() {
        return Ok(LocationStore::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(LocationStore::default());
    }
    serde_json::from_str(&raw).map_err(|e| AppError::msg(e.to_string()))
}

pub fn save(path: &Path, store: &LocationStore) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut ordered = store.clone();
    ordered
        .locations
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    fs::write(
        path,
        serde_json::to_string_pretty(&ordered).map_err(|e| AppError::msg(e.to_string()))?,
    )?;
    Ok(())
}

pub fn upsert(
    store: &mut LocationStore,
    id: Option<&str>,
    name: &str,
) -> AppResult<SavedLocation> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::msg("Location name is required"));
    }
    if let Some(existing) = store.locations.iter().find(|location| {
        location.name.eq_ignore_ascii_case(name) && id.map(|id| location.id != id).unwrap_or(true)
    }) {
        return Ok(existing.clone());
    }
    if let Some(id) = id.filter(|id| !id.is_empty()) {
        let location = store
            .locations
            .iter_mut()
            .find(|location| location.id == id)
            .ok_or_else(|| AppError::msg("Unknown location"))?;
        location.name = name.to_string();
        return Ok(location.clone());
    }
    let location = SavedLocation {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
    };
    store.locations.push(location.clone());
    Ok(location)
}

pub fn remove(store: &mut LocationStore, id: &str) -> AppResult<SavedLocation> {
    let index = store
        .locations
        .iter()
        .position(|location| location.id == id)
        .ok_or_else(|| AppError::msg("Unknown location"))?;
    Ok(store.locations.remove(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trips_and_dedupes_names() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("locations.json");
        let mut store = load(&path).unwrap();
        let alps = upsert(&mut store, None, "Alps").unwrap();
        assert!(upsert(&mut store, None, "alps").unwrap().id == alps.id);
        upsert(&mut store, Some(&alps.id), "Chamonix").unwrap();
        save(&path, &store).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.locations[0].name, "Chamonix");
        assert!(remove(&mut store, &alps.id).is_ok());
        assert!(store.locations.is_empty());
    }
}
