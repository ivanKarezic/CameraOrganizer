use crate::media::MediaKind;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub const VIDEO_FOLDER: &str = "Video";
pub const PHOTO_FOLDER: &str = "Photo";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizedLocation {
    pub year: String,
    pub date: String,
    pub kind_folder: String,
    pub filename: String,
}

impl OrganizedLocation {
    #[allow(dead_code)]
    pub fn relative_path(&self) -> PathBuf {
        PathBuf::from(&self.year)
            .join(&self.date)
            .join(&self.kind_folder)
            .join(&self.filename)
    }
}

pub fn kind_folder(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Photo => PHOTO_FOLDER,
        MediaKind::Video | MediaKind::Sidecar => VIDEO_FOLDER,
    }
}

pub fn build_organized_path(date: NaiveDate, kind: MediaKind, filename: &str) -> PathBuf {
    let year = date.format("%Y").to_string();
    let day = date.format("%Y-%m-%d").to_string();
    PathBuf::from(year)
        .join(day)
        .join(kind_folder(kind))
        .join(filename)
}

pub fn is_organized_path(relative: &Path) -> bool {
    parse_organized_path(relative).is_some()
}

pub fn parse_organized_path(relative: &Path) -> Option<OrganizedLocation> {
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    if parts.len() != 4 {
        return None;
    }
    let year = parts[0];
    let date = parts[1];
    let kind_folder = parts[2];
    let filename = parts[3];
    if year.len() != 4 || year.chars().any(|c| !c.is_ascii_digit()) {
        return None;
    }
    if NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?.format("%Y").to_string() != year {
        return None;
    }
    if kind_folder != VIDEO_FOLDER && kind_folder != PHOTO_FOLDER {
        return None;
    }
    if filename.is_empty() || filename.contains('/') {
        return None;
    }
    Some(OrganizedLocation {
        year: year.to_string(),
        date: date.to_string(),
        kind_folder: kind_folder.to_string(),
        filename: filename.to_string(),
    })
}

pub fn split_filename(filename: &str) -> (String, String) {
    match filename.rfind('.') {
        Some(idx) if idx > 0 => (filename[..idx].to_string(), filename[idx + 1..].to_string()),
        _ => (filename.to_string(), String::new()),
    }
}

/// If `desired` is taken, produce `stem_2.ext`, `stem_3.ext`, …
pub fn disambiguate_filename(
    desired: &str,
    occupied: &HashSet<String>,
) -> (String, Option<String>) {
    if !occupied.contains(desired) {
        return (desired.to_string(), None);
    }
    let (stem, ext) = split_filename(desired);
    let mut n = 2u32;
    loop {
        let candidate = if ext.is_empty() {
            format!("{stem}_{n}")
        } else {
            format!("{stem}_{n}.{ext}")
        };
        if !occupied.contains(&candidate) {
            return (
                candidate.clone(),
                Some(format!("Renamed to avoid collision: {candidate}")),
            );
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_year_then_day_layout() {
        let date = NaiveDate::from_ymd_opt(2024, 8, 26).unwrap();
        let video = build_organized_path(date, MediaKind::Video, "GH010123.MP4");
        let photo = build_organized_path(date, MediaKind::Photo, "DJI_0001.JPG");
        assert_eq!(
            video,
            PathBuf::from("2024/2024-08-26/Video/GH010123.MP4")
        );
        assert_eq!(
            photo,
            PathBuf::from("2024/2024-08-26/Photo/DJI_0001.JPG")
        );
        assert!(is_organized_path(&video));
        assert!(is_organized_path(&photo));
    }

    #[test]
    fn rejects_unorganized_shapes() {
        assert!(!is_organized_path(Path::new("DJI_0001.JPG")));
        assert!(!is_organized_path(Path::new("2024/08/26/Photo/x.jpg")));
        assert!(!is_organized_path(Path::new(
            "2024/2024-08-26/Video/nested/x.mp4"
        )));
        assert!(!is_organized_path(Path::new("2023/2024-08-26/Video/x.mp4")));
    }

    #[test]
    fn disambiguates_collisions() {
        let occupied = HashSet::from(["clip.MP4".into(), "clip_2.MP4".into()]);
        let (name, warn) = disambiguate_filename("clip.MP4", &occupied);
        assert_eq!(name, "clip_3.MP4");
        assert!(warn.unwrap().contains("clip_3.MP4"));
    }
}
