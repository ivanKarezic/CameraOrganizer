use crate::camera::CameraBrand;
use crate::error::AppResult;
use crate::media::MediaKind;
use crate::metadata::DateSource;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: i64,
    pub storage_id: String,
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub hash: Option<String>,
    pub camera: CameraBrand,
    pub kind: MediaKind,
    pub captured_at: String,
    pub date_source: DateSource,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_label: Option<String>,
    pub organized: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub tag: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub camera: Option<String>,
    pub location: Option<String>,
    pub unorganized_only: bool,
}

pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    init_schema(&conn)?;
    Ok(conn)
}

#[allow(dead_code)]
pub fn open_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS media (
            id INTEGER PRIMARY KEY,
            storage_id TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            filename TEXT NOT NULL,
            size INTEGER NOT NULL,
            hash TEXT,
            camera TEXT NOT NULL,
            kind TEXT NOT NULL,
            captured_at TEXT NOT NULL,
            date_source TEXT NOT NULL,
            latitude REAL,
            longitude REAL,
            location_label TEXT,
            organized INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL COLLATE NOCASE UNIQUE
        );
        CREATE TABLE IF NOT EXISTS media_tags (
            media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (media_id, tag_id)
        );
        CREATE INDEX IF NOT EXISTS idx_media_captured ON media(captured_at);
        CREATE INDEX IF NOT EXISTS idx_media_camera ON media(camera);
        CREATE INDEX IF NOT EXISTS idx_media_filename_size ON media(filename, size);
        "#,
    )?;
    Ok(())
}

fn camera_from_label(label: &str) -> CameraBrand {
    match label {
        "DJI" => CameraBrand::Dji,
        "GoPro" => CameraBrand::Gopro,
        "Insta360" => CameraBrand::Insta360,
        _ => CameraBrand::Unknown,
    }
}

fn kind_from_label(label: &str) -> MediaKind {
    match label {
        "photo" => MediaKind::Photo,
        "sidecar" => MediaKind::Sidecar,
        _ => MediaKind::Video,
    }
}

fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Photo => "photo",
        MediaKind::Video => "video",
        MediaKind::Sidecar => "sidecar",
    }
}

fn date_source_label(source: DateSource) -> &'static str {
    match source {
        DateSource::Exif => "exif",
        DateSource::Filename => "filename",
        DateSource::Mtime => "mtime",
        DateSource::Unknown => "unknown",
    }
}

fn date_source_from(label: &str) -> DateSource {
    match label {
        "exif" => DateSource::Exif,
        "filename" => DateSource::Filename,
        "mtime" => DateSource::Mtime,
        _ => DateSource::Unknown,
    }
}

pub fn upsert_media(conn: &Connection, item: &MediaItem) -> AppResult<i64> {
    conn.execute(
        r#"
        INSERT INTO media (
            storage_id, path, filename, size, hash, camera, kind,
            captured_at, date_source, latitude, longitude, location_label, organized
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(path) DO UPDATE SET
            storage_id = excluded.storage_id,
            filename = excluded.filename,
            size = excluded.size,
            hash = COALESCE(excluded.hash, media.hash),
            camera = excluded.camera,
            kind = excluded.kind,
            captured_at = excluded.captured_at,
            date_source = excluded.date_source,
            latitude = excluded.latitude,
            longitude = excluded.longitude,
            location_label = excluded.location_label,
            organized = excluded.organized
        "#,
        params![
            item.storage_id,
            item.path,
            item.filename,
            item.size,
            item.hash,
            item.camera.label(),
            kind_label(item.kind),
            item.captured_at,
            date_source_label(item.date_source),
            item.latitude,
            item.longitude,
            item.location_label,
            item.organized as i64,
        ],
    )?;
    let id = conn.query_row(
        "SELECT id FROM media WHERE path = ?1",
        params![item.path],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn update_path(conn: &Connection, from: &str, to: &str, organized: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE media SET path = ?1, filename = ?2, organized = ?3 WHERE path = ?4",
        params![
            to,
            Path::new(to)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(to),
            organized as i64,
            from
        ],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn set_hash(conn: &Connection, path: &str, hash: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE media SET hash = ?1 WHERE path = ?2",
        params![hash, path],
    )?;
    Ok(())
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<MediaItem>> {
    search(
        conn,
        &SearchQuery {
            unorganized_only: false,
            ..Default::default()
        },
    )
}

pub fn search(conn: &Connection, query: &SearchQuery) -> AppResult<Vec<MediaItem>> {
    let mut sql = String::from(
        r#"
        SELECT DISTINCT m.id, m.storage_id, m.path, m.filename, m.size, m.hash, m.camera, m.kind,
               m.captured_at, m.date_source, m.latitude, m.longitude, m.location_label, m.organized
        FROM media m
        LEFT JOIN media_tags mt ON mt.media_id = m.id
        LEFT JOIN tags t ON t.id = mt.tag_id
        WHERE 1=1
        "#,
    );
    let mut args: Vec<String> = Vec::new();

    if query.unorganized_only {
        sql.push_str(" AND m.organized = 0 ");
    }
    if let Some(text) = query.text.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(
            " AND (m.filename LIKE ? OR m.camera LIKE ? OR IFNULL(m.location_label,'') LIKE ? OR IFNULL(t.name,'') LIKE ?) ",
        );
        let like = format!("%{text}%");
        args.extend([like.clone(), like.clone(), like.clone(), like]);
    }
    if let Some(tag) = query.tag.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND t.name = ? ");
        args.push(tag.clone());
    }
    if let Some(from) = query.date_from.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND date(m.captured_at) >= date(?) ");
        args.push(from.clone());
    }
    if let Some(to) = query.date_to.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND date(m.captured_at) <= date(?) ");
        args.push(to.clone());
    }
    if let Some(camera) = query.camera.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND m.camera = ? ");
        args.push(camera.clone());
    }
    if let Some(location) = query.location.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND IFNULL(m.location_label,'') LIKE ? ");
        args.push(format!("%{location}%"));
    }
    sql.push_str(" ORDER BY m.captured_at DESC, m.filename ASC ");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |row| {
        Ok(MediaItem {
            id: row.get(0)?,
            storage_id: row.get(1)?,
            path: row.get(2)?,
            filename: row.get(3)?,
            size: row.get(4)?,
            hash: row.get(5)?,
            camera: camera_from_label(&row.get::<_, String>(6)?),
            kind: kind_from_label(&row.get::<_, String>(7)?),
            captured_at: row.get(8)?,
            date_source: date_source_from(&row.get::<_, String>(9)?),
            latitude: row.get(10)?,
            longitude: row.get(11)?,
            location_label: row.get(12)?,
            organized: row.get::<_, i64>(13)? != 0,
            tags: Vec::new(),
        })
    })?;

    let mut items = Vec::new();
    for row in rows {
        let mut item = row?;
        item.tags = tags_for(conn, item.id)?;
        items.push(item);
    }
    Ok(items)
}

fn tags_for(conn: &Connection, media_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN media_tags mt ON mt.tag_id = t.id WHERE mt.media_id = ?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map(params![media_id], |row| row.get(0))?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

pub fn list_tags(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM tags ORDER BY name")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

pub fn set_tags(conn: &Connection, media_id: i64, tags: &[String]) -> AppResult<Vec<String>> {
    conn.execute("DELETE FROM media_tags WHERE media_id = ?1", params![media_id])?;
    for tag in tags {
        let name = tag.trim();
        if name.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
            params![name],
        )?;
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |row| {
            row.get(0)
        })?;
        conn.execute(
            "INSERT OR IGNORE INTO media_tags (media_id, tag_id) VALUES (?1, ?2)",
            params![media_id, tag_id],
        )?;
    }
    tags_for(conn, media_id)
}

pub fn library_records(conn: &Connection) -> AppResult<Vec<crate::sync::LibraryRecord>> {
    let mut stmt = conn.prepare("SELECT filename, size, hash FROM media")?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::sync::LibraryRecord {
            filename: row.get(0)?,
            size: row.get::<_, i64>(1)? as u64,
            hash: row.get(2)?,
        })
    })?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }
    Ok(records)
}

#[allow(dead_code)]
pub fn paths_for_storage(conn: &Connection, storage_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM media WHERE storage_id = ?1")?;
    let rows = stmt.query_map(params![storage_id], |row| row.get(0))?;
    let mut paths = Vec::new();
    for row in rows {
        paths.push(row?);
    }
    Ok(paths)
}

#[allow(dead_code)]
pub fn media_id_by_path(conn: &Connection, path: &str) -> AppResult<Option<i64>> {
    let id = conn
        .query_row("SELECT id FROM media WHERE path = ?1", params![path], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_by_tag_and_unorganized() {
        let conn = open_memory().unwrap();
        let mut item = MediaItem {
            id: 0,
            storage_id: "s1".into(),
            path: "/lib/inbox/DJI_0001.JPG".into(),
            filename: "DJI_0001.JPG".into(),
            size: 12,
            hash: None,
            camera: CameraBrand::Dji,
            kind: MediaKind::Photo,
            captured_at: "2024-08-26T14:30:22".into(),
            date_source: DateSource::Filename,
            latitude: Some(47.1),
            longitude: Some(8.5),
            location_label: Some("47.1, 8.5".into()),
            organized: false,
            tags: vec![],
        };
        let id = upsert_media(&conn, &item).unwrap();
        set_tags(&conn, id, &["alps".into(), "drone".into()]).unwrap();
        item.path = "/lib/2024/2024-08-26/Photo/keep.JPG".into();
        item.filename = "keep.JPG".into();
        item.organized = true;
        upsert_media(&conn, &item).unwrap();

        let tagged = search(
            &conn,
            &SearchQuery {
                tag: Some("alps".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].filename, "DJI_0001.JPG");

        let unorganized = search(
            &conn,
            &SearchQuery {
                unorganized_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(unorganized.len(), 1);
    }
}
