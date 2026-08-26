use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraBrand {
    #[serde(rename = "DJI")]
    Dji,
    #[serde(rename = "GoPro")]
    Gopro,
    #[serde(rename = "Insta360")]
    Insta360,
    Unknown,
}

impl CameraBrand {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dji => "DJI",
            Self::Gopro => "GoPro",
            Self::Insta360 => "Insta360",
            Self::Unknown => "Unknown",
        }
    }
}

fn dji_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(?:ORG_)?DJI_").expect("dji regex"))
}

fn gopro_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:GOPR|GP[0-9]{2}|G[HXLSAB][0-9]{2})[0-9]{4}(?:\.[A-Za-z0-9]+)?$")
            .expect("gopro regex")
    })
}

fn insta_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:PRO_)?(?:VID|IMG|LRV)_\d{8}_\d{6}_\d{2}_").expect("insta regex")
    })
}

fn extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Identify a camera brand from a media file name.
pub fn identify_camera(filename: &str) -> CameraBrand {
    let name = std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);

    match extension(name).as_str() {
        "insv" | "insp" => return CameraBrand::Insta360,
        _ => {}
    }

    if insta_name_re().is_match(name) {
        return CameraBrand::Insta360;
    }
    if dji_re().is_match(name) {
        return CameraBrand::Dji;
    }
    if gopro_re().is_match(name) {
        return CameraBrand::Gopro;
    }

    CameraBrand::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_dji_names() {
        for name in [
            "DJI_20240826143022_0001_D.MP4",
            "DJI_20240826_143022_000_0001.MP4",
            "DJI_0001.JPG",
            "DJI_0001.DNG",
            "dji_0701.mp4",
            "ORG_DJI_20240826_143022_000_0001.MOV",
        ] {
            assert_eq!(identify_camera(name), CameraBrand::Dji, "{name}");
        }
    }

    #[test]
    fn recognizes_gopro_names() {
        for name in [
            "GH010123.MP4",
            "GX010123.MP4",
            "GL010123.LRV",
            "GOPR0123.JPG",
            "GOPR0123.MP4",
            "GP010123.MP4",
            "GS010123.360",
            "gx019876.mp4",
        ] {
            assert_eq!(identify_camera(name), CameraBrand::Gopro, "{name}");
        }
    }

    #[test]
    fn recognizes_insta360_names() {
        for name in [
            "VID_20240826_143022_00_010.insv",
            "IMG_20240826_143022_00_010.insp",
            "IMG_20240826_143022_00_010.jpg",
            "LRV_20240826_143022_00_010.lrv",
            "PRO_VID_20240826_143022_00_010.insv",
            "clip.insv",
        ] {
            assert_eq!(identify_camera(name), CameraBrand::Insta360, "{name}");
        }
    }

    #[test]
    fn unknown_for_generic_names() {
        for name in ["IMG_1234.JPG", "DSC0001.JPG", "vacation.mp4", "GHOST.mp4"] {
            assert_eq!(identify_camera(name), CameraBrand::Unknown, "{name}");
        }
    }
}
