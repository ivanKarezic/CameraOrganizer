use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageKind {
    Local,
    Network,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: StorageKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub storage_mode: StorageMode,
    pub storages: Vec<Storage>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage_mode: StorageMode::Single,
            storages: Vec::new(),
        }
    }
}

impl AppConfig {
    #[allow(dead_code)]
    pub fn new_storage(name: String, path: String, kind: StorageKind) -> Storage {
        Storage {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            path,
            kind,
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.storage_mode == StorageMode::Single && self.storages.len() > 1 {
            return Err(AppError::msg(
                "Single-location mode allows only one storage. Switch to multiple locations or remove extras.",
            ));
        }
        for storage in &self.storages {
            if storage.path.trim().is_empty() {
                return Err(AppError::msg(format!(
                    "Storage '{}' is missing a path",
                    storage.name
                )));
            }
        }
        Ok(())
    }
}

pub fn load_or_default(path: &Path) -> AppResult<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(AppConfig::default());
    }
    let config: AppConfig = toml::from_str(&raw)?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &AppConfig) -> AppResult<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}

pub fn config_path_from(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trips_single_and_multiple_storages() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut single = AppConfig::default();
        single.storages.push(AppConfig::new_storage(
            "Main".into(),
            "/Volumes/Media".into(),
            StorageKind::Local,
        ));
        save_config(&path, &single).unwrap();
        let loaded = load_or_default(&path).unwrap();
        assert_eq!(loaded.storage_mode, StorageMode::Single);
        assert_eq!(loaded.storages.len(), 1);
        assert_eq!(loaded.storages[0].path, "/Volumes/Media");

        let mut multiple = loaded.clone();
        multiple.storage_mode = StorageMode::Multiple;
        multiple.storages.push(AppConfig::new_storage(
            "NAS".into(),
            "/Volumes/NAS/Cameras".into(),
            StorageKind::Network,
        ));
        save_config(&path, &multiple).unwrap();
        let loaded = load_or_default(&path).unwrap();
        assert_eq!(loaded.storage_mode, StorageMode::Multiple);
        assert_eq!(loaded.storages.len(), 2);
        assert_eq!(loaded.storages[1].kind, StorageKind::Network);
    }

    #[test]
    fn rejects_two_storages_in_single_mode() {
        let mut config = AppConfig::default();
        config.storages.push(AppConfig::new_storage(
            "A".into(),
            "/a".into(),
            StorageKind::Local,
        ));
        config.storages.push(AppConfig::new_storage(
            "B".into(),
            "/b".into(),
            StorageKind::External,
        ));
        assert!(config.validate().is_err());
    }
}
