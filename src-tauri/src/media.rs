use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Photo,
    Video,
    Sidecar,
}

const PHOTO_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "dng", "raw", "arw", "cr2", "cr3", "nef", "raf", "orf",
    "rw2", "insp", "tif", "tiff", "webp", "gif", "bmp",
];
const VIDEO_EXT: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "insv", "360", "braw", "mts", "m2ts"];
const SIDECAR_EXT: &[&str] = &["srt", "lrv", "thm", "xmp", "aae", "wav", "lrf", "html"];

pub fn classify_filename(filename: &str) -> Option<MediaKind> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    if PHOTO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Photo)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else if SIDECAR_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Sidecar)
    } else {
        None
    }
}

pub fn should_skip_name(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.starts_with("._") {
        return true;
    }
    matches!(
        name,
        "Thumbs.db" | "desktop.ini" | "Index.db" | "MISC" | "SYSTEM VOLUME INFORMATION"
    )
}

fn insta_group_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:PRO_)?(?:VID|IMG|LRV)_(\d{8})_(\d{6})_\d{2}_(.+)$").expect("insta group")
    })
}

pub fn file_stem(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .to_string()
}

/// Files that should travel together (proxies, subtitles, RAW+JPEG, Insta360 pairs).
pub fn grouping_key(filename: &str) -> String {
    let stem = file_stem(filename);
    if let Some(caps) = insta_group_re().captures(&stem) {
        let date = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let time = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let rest = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
        return format!("insta:{date}:{time}:{rest}").to_ascii_lowercase();
    }
    stem.to_ascii_lowercase()
}

pub fn folder_kind_for_group<'a>(kinds: impl IntoIterator<Item = &'a MediaKind>) -> MediaKind {
    let mut has_video = false;
    let mut has_photo = false;
    for kind in kinds {
        match kind {
            MediaKind::Video => has_video = true,
            MediaKind::Photo => has_photo = true,
            MediaKind::Sidecar => {}
        }
    }
    if has_video {
        MediaKind::Video
    } else if has_photo {
        MediaKind::Photo
    } else {
        MediaKind::Video
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_extensions() {
        assert_eq!(classify_filename("DJI_0001.JPG"), Some(MediaKind::Photo));
        assert_eq!(classify_filename("clip.MP4"), Some(MediaKind::Video));
        assert_eq!(classify_filename("clip.SRT"), Some(MediaKind::Sidecar));
        assert_eq!(classify_filename("GH010123.LRV"), Some(MediaKind::Sidecar));
        assert_eq!(classify_filename("shot.insv"), Some(MediaKind::Video));
        assert_eq!(classify_filename("notes.txt"), None);
    }

    #[test]
    fn groups_insta_pairs_and_raw_jpeg() {
        assert_eq!(
            grouping_key("VID_20240826_143022_00_010.insv"),
            grouping_key("LRV_20240826_143022_00_010.lrv")
        );
        assert_eq!(
            grouping_key("VID_20240826_143022_00_010.insv"),
            grouping_key("VID_20240826_143022_10_010.insv")
        );
        assert_eq!(grouping_key("DJI_0001.DNG"), grouping_key("DJI_0001.JPG"));
        assert_eq!(grouping_key("clip.MP4"), grouping_key("clip.SRT"));
    }

    #[test]
    fn skips_junk_names() {
        assert!(should_skip_name(".DS_Store"));
        assert!(should_skip_name("._DJI_0001.JPG"));
        assert!(!should_skip_name("DJI_0001.JPG"));
    }
}
