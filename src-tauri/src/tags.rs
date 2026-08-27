use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CATEGORY: &str = "General";
const DEFAULT_COLOR: &str = "#e59a2a";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TagCategory {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GlobalTag {
    pub id: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub color: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TagStore {
    #[serde(default)]
    pub categories: Vec<TagCategory>,
    #[serde(default)]
    pub tags: Vec<GlobalTag>,
}

#[derive(Debug, Deserialize)]
struct LegacyTag {
    id: String,
    name: String,
    color: String,
}

pub fn tags_path_from(config_dir: &Path) -> PathBuf {
    config_dir.join("tags.json")
}

pub fn valid_color(color: &str) -> bool {
    let color = color.trim();
    color.len() == 7
        && color.starts_with('#')
        && color.chars().skip(1).all(|c| c.is_ascii_hexdigit())
}

pub fn load(path: &Path) -> AppResult<TagStore> {
    if !path.exists() {
        return Ok(ensure_default(TagStore::default()));
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(ensure_default(TagStore::default()));
    }
    let store = if let Ok(legacy) = serde_json::from_str::<Vec<LegacyTag>>(&raw) {
        migrate_legacy(legacy)
    } else {
        serde_json::from_str::<TagStore>(&raw).map_err(|e| AppError::msg(e.to_string()))?
    };
    Ok(hydrate(ensure_default(store)))
}

pub fn save(path: &Path, store: &TagStore) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut ordered = hydrate(store.clone());
    ordered
        .categories
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    ordered
        .tags
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    fs::write(
        path,
        serde_json::to_string_pretty(&ordered).map_err(|e| AppError::msg(e.to_string()))?,
    )?;
    Ok(())
}

fn migrate_legacy(legacy: Vec<LegacyTag>) -> TagStore {
    let mut store = ensure_default(TagStore::default());
    let general_id = store.categories[0].id.clone();
    for tag in legacy {
        store.tags.push(GlobalTag {
            id: tag.id,
            name: tag.name,
            category_id: general_id.clone(),
            category_name: DEFAULT_CATEGORY.into(),
            color: tag.color,
        });
    }
    hydrate(store)
}

fn ensure_default(mut store: TagStore) -> TagStore {
    if store.categories.is_empty() {
        store.categories.push(TagCategory {
            id: uuid::Uuid::new_v4().to_string(),
            name: DEFAULT_CATEGORY.into(),
            color: DEFAULT_COLOR.into(),
        });
    }
    store
}

pub fn hydrate(mut store: TagStore) -> TagStore {
    for tag in &mut store.tags {
        if let Some(category) = store.categories.iter().find(|c| c.id == tag.category_id) {
            tag.category_name = category.name.clone();
            tag.color = category.color.clone();
        } else if let Some(category) = store.categories.first() {
            tag.category_id = category.id.clone();
            tag.category_name = category.name.clone();
            tag.color = category.color.clone();
        }
    }
    store
}

pub fn upsert_category(
    store: &mut TagStore,
    id: Option<&str>,
    name: &str,
    color: &str,
) -> AppResult<TagCategory> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::msg("Category name is required"));
    }
    if !valid_color(color) {
        return Err(AppError::msg("Color must be #RRGGBB"));
    }
    if let Some(existing) = store.categories.iter().find(|category| {
        category.name.eq_ignore_ascii_case(name) && id.map(|id| category.id != id).unwrap_or(true)
    }) {
        return Err(AppError::msg(format!("Category '{}' already exists", existing.name)));
    }
    if let Some(id) = id.filter(|id| !id.is_empty()) {
        let category = store
            .categories
            .iter_mut()
            .find(|category| category.id == id)
            .ok_or_else(|| AppError::msg("Unknown category"))?;
        category.name = name.to_string();
        category.color = color.trim().to_string();
        let saved = category.clone();
        *store = hydrate(store.clone());
        return Ok(saved);
    }
    let category = TagCategory {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        color: color.trim().to_string(),
    };
    store.categories.push(category.clone());
    Ok(category)
}

pub fn remove_category(store: &mut TagStore, id: &str) -> AppResult<TagCategory> {
    if store.categories.len() <= 1 {
        return Err(AppError::msg("Keep at least one tag category"));
    }
    let index = store
        .categories
        .iter()
        .position(|category| category.id == id)
        .ok_or_else(|| AppError::msg("Unknown category"))?;
    let removed = store.categories.remove(index);
    let fallback = store.categories[0].id.clone();
    for tag in &mut store.tags {
        if tag.category_id == removed.id {
            tag.category_id = fallback.clone();
        }
    }
    *store = hydrate(store.clone());
    Ok(removed)
}

pub fn upsert_tag(
    store: &mut TagStore,
    id: Option<&str>,
    name: &str,
    category_id: &str,
) -> AppResult<GlobalTag> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::msg("Tag name is required"));
    }
    if !store.categories.iter().any(|category| category.id == category_id) {
        return Err(AppError::msg("Unknown tag category"));
    }
    if let Some(existing) = store.tags.iter().find(|tag| {
        tag.name.eq_ignore_ascii_case(name) && id.map(|id| tag.id != id).unwrap_or(true)
    }) {
        return Err(AppError::msg(format!("Tag '{}' already exists", existing.name)));
    }
    if let Some(id) = id.filter(|id| !id.is_empty()) {
        let tag = store
            .tags
            .iter_mut()
            .find(|tag| tag.id == id)
            .ok_or_else(|| AppError::msg("Unknown tag"))?;
        tag.name = name.to_string();
        tag.category_id = category_id.to_string();
        *store = hydrate(store.clone());
        return Ok(store
            .tags
            .iter()
            .find(|tag| tag.id == id)
            .cloned()
            .expect("saved tag"));
    }
    let tag = GlobalTag {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        category_id: category_id.to_string(),
        category_name: String::new(),
        color: String::new(),
    };
    store.tags.push(tag);
    *store = hydrate(store.clone());
    Ok(store.tags.last().cloned().expect("saved tag"))
}

pub fn remove_tag(store: &mut TagStore, id: &str) -> AppResult<GlobalTag> {
    let index = store
        .tags
        .iter()
        .position(|tag| tag.id == id)
        .ok_or_else(|| AppError::msg("Unknown tag"))?;
    Ok(store.tags.remove(index))
}

pub fn color_for(store: &TagStore, name: &str) -> String {
    store
        .tags
        .iter()
        .find(|tag| tag.name.eq_ignore_ascii_case(name))
        .map(|tag| tag.color.clone())
        .unwrap_or_else(|| DEFAULT_COLOR.into())
}

pub fn tag_by_name<'a>(store: &'a TagStore, name: &str) -> Option<&'a GlobalTag> {
    store.tags.iter().find(|tag| tag.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn categories_and_tag_assignment_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tags.json");
        let mut store = load(&path).unwrap();
        assert_eq!(store.categories[0].name, "General");
        let people = upsert_category(&mut store, None, "People", "#6a93c4").unwrap();
        let tag = upsert_tag(&mut store, None, "Alps", &people.id).unwrap();
        assert_eq!(tag.color, "#6a93c4");
        assert_eq!(tag.category_name, "People");
        save(&path, &store).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.tags[0].category_name, "People");
        assert!(upsert_tag(&mut store, None, "alps", &people.id).is_err());
        remove_category(&mut store, &people.id).unwrap();
        assert_eq!(store.tags[0].category_name, "General");
        let last_id = store.categories[0].id.clone();
        assert!(remove_category(&mut store, &last_id).is_err());
    }

    #[test]
    fn migrates_legacy_tag_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tags.json");
        fs::write(
            &path,
            r##"[{"id":"1","name":"Alps","color":"#6a93c4"}]"##,
        )
        .unwrap();
        let store = load(&path).unwrap();
        assert_eq!(store.tags[0].name, "Alps");
        assert_eq!(store.categories[0].name, "General");
    }
}
