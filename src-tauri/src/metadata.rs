use crate::camera::{identify_camera, CameraBrand};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateSource {
    Exif,
    Filename,
    Mtime,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct FileHints {
    pub exif_datetime: Option<NaiveDateTime>,
    pub gps: Option<(f64, f64)>,
    pub mtime: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTime {
    pub captured_at: NaiveDateTime,
    pub source: DateSource,
}

fn re_dji_split() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)DJI_(\d{8})_(\d{6})").expect("dji split"))
}

fn re_dji_compact() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)DJI_(\d{14})").expect("dji compact"))
}

fn re_insta() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:PRO_)?(?:VID|IMG|LRV)_(\d{8})_(\d{6})").expect("insta date")
    })
}

fn re_iso_date() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{4})-(\d{2})-(\d{2})").expect("iso date"))
}

fn parse_ymd_hms(date: &str, time: &str) -> Option<NaiveDateTime> {
    let d = NaiveDate::parse_from_str(date, "%Y%m%d").ok()?;
    let t = NaiveTime::parse_from_str(time, "%H%M%S").ok()?;
    Some(d.and_time(t))
}

fn parse_compact(stamp: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(stamp, "%Y%m%d%H%M%S").ok()
}

/// Date from EXIF, then filename, then file mtime.
pub fn resolve_capture_time(filename: &str, hints: &FileHints) -> ResolvedTime {
    if let Some(dt) = hints.exif_datetime {
        return ResolvedTime {
            captured_at: dt,
            source: DateSource::Exif,
        };
    }
    if let Some(dt) = parse_date_from_filename(filename) {
        return ResolvedTime {
            captured_at: dt,
            source: DateSource::Filename,
        };
    }
    if let Some(dt) = hints.mtime {
        return ResolvedTime {
            captured_at: dt,
            source: DateSource::Mtime,
        };
    }
    ResolvedTime {
        captured_at: NaiveDateTime::parse_from_str("1970-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
            .expect("epoch"),
        source: DateSource::Unknown,
    }
}

pub fn parse_date_from_filename(filename: &str) -> Option<NaiveDateTime> {
    if let Some(caps) = re_dji_split().captures(filename) {
        return parse_ymd_hms(caps.get(1)?.as_str(), caps.get(2)?.as_str());
    }
    if let Some(caps) = re_dji_compact().captures(filename) {
        return parse_compact(caps.get(1)?.as_str());
    }
    if let Some(caps) = re_insta().captures(filename) {
        return parse_ymd_hms(caps.get(1)?.as_str(), caps.get(2)?.as_str());
    }
    if let Some(caps) = re_iso_date().captures(filename) {
        let y = caps.get(1)?.as_str();
        let m = caps.get(2)?.as_str();
        let d = caps.get(3)?.as_str();
        let date = NaiveDate::parse_from_str(&format!("{y}-{m}-{d}"), "%Y-%m-%d").ok()?;
        return Some(date.and_hms_opt(0, 0, 0)?);
    }
    // Generic 8-digit date, but not GoPro chapter/file numbers like GH010123.
    if identify_camera(filename) == CameraBrand::Gopro {
        return None;
    }
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    for chunk in stem.split(|c: char| !c.is_ascii_digit()) {
        if chunk.len() == 8 {
            if let Ok(date) = NaiveDate::parse_from_str(chunk, "%Y%m%d") {
                if date.year() >= 1995 && date.year() <= 2100 {
                    return Some(date.and_hms_opt(0, 0, 0)?);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn prefers_exif_then_filename_then_mtime() {
        let filename = "DJI_20240826_143022_000_0001.MP4";
        let mtime =
            NaiveDateTime::parse_from_str("2020-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let exif =
            NaiveDateTime::parse_from_str("2024-07-01 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let from_exif = resolve_capture_time(
            filename,
            &FileHints {
                exif_datetime: Some(exif),
                mtime: Some(mtime),
                ..Default::default()
            },
        );
        assert_eq!(from_exif.source, DateSource::Exif);
        assert_eq!(from_exif.captured_at, exif);

        let from_name = resolve_capture_time(
            filename,
            &FileHints {
                mtime: Some(mtime),
                ..Default::default()
            },
        );
        assert_eq!(from_name.source, DateSource::Filename);
        assert_eq!(from_name.captured_at.date().year(), 2024);
        assert_eq!(from_name.captured_at.date().month(), 8);
        assert_eq!(from_name.captured_at.date().day(), 26);

        let from_mtime = resolve_capture_time(
            "GH010123.MP4",
            &FileHints {
                mtime: Some(mtime),
                ..Default::default()
            },
        );
        assert_eq!(from_mtime.source, DateSource::Mtime);
        assert_eq!(from_mtime.captured_at, mtime);
    }

    #[test]
    fn parses_insta_and_compact_dji() {
        let insta = parse_date_from_filename("VID_20240826_143022_00_010.insv").unwrap();
        assert_eq!(insta.date().day(), 26);
        let dji = parse_date_from_filename("DJI_20240826143022_0001_D.MP4").unwrap();
        assert_eq!(dji.time().hour(), 14);
    }
}
