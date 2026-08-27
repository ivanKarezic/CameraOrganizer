use crate::camorg;
use crate::error::AppResult;
use crate::media::MediaKind;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const MAX_EDGE: u32 = 320;

pub fn ensure_thumbnail(
    storage_root: &Path,
    abs_path: &Path,
    rel: &Path,
    kind: MediaKind,
) -> AppResult<Option<PathBuf>> {
    if matches!(kind, MediaKind::Sidecar) || !abs_path.is_file() {
        return Ok(None);
    }
    let dest = camorg::thumbnail_abs(storage_root, rel);
    if dest.is_file() {
        return Ok(Some(camorg::thumbnail_rel(rel)));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let ok = match kind {
        MediaKind::Photo => generate_photo(abs_path, &dest),
        MediaKind::Video => generate_video(abs_path, &dest),
        MediaKind::Sidecar => false,
    };
    if ok && dest.is_file() {
        Ok(Some(camorg::thumbnail_rel(rel)))
    } else {
        let _ = std::fs::remove_file(&dest);
        Ok(None)
    }
}

fn generate_photo(src: &Path, dest: &Path) -> bool {
    if encode_with_image(src, dest) {
        return true;
    }
    sips_convert(src, dest)
}

fn encode_with_image(src: &Path, dest: &Path) -> bool {
    let Ok(reader) = image::ImageReader::open(src) else {
        return false;
    };
    let Ok(reader) = reader.with_guessed_format() else {
        return false;
    };
    let Ok(img) = reader.decode() else {
        return false;
    };
    let thumb = img.thumbnail(MAX_EDGE, MAX_EDGE);
    let rgb = thumb.to_rgb8();
    let Ok(mut file) = File::create(dest) else {
        return false;
    };
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 72);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .is_ok()
}

fn sips_convert(src: &Path, dest: &Path) -> bool {
    if cfg!(not(target_os = "macos")) {
        return false;
    }
    Command::new("sips")
        .args(["-s", "format", "jpeg", "-Z", "320"])
        .arg(src)
        .arg("--out")
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success() && dest.is_file())
        .unwrap_or(false)
}

fn generate_video(src: &Path, dest: &Path) -> bool {
    if let Some(thm) = sibling_poster(src) {
        if generate_photo(&thm, dest) {
            return true;
        }
    }
    if let Some(ffmpeg) = find_ffmpeg() {
        if ffmpeg_frame(&ffmpeg, src, dest, true, "0.5")
            || ffmpeg_frame(&ffmpeg, src, dest, true, "0")
            || ffmpeg_frame(&ffmpeg, src, dest, false, "0")
        {
            return true;
        }
    }
    qlmanage_poster(src, dest)
}

fn sibling_poster(src: &Path) -> Option<PathBuf> {
    let stem = src.file_stem()?.to_string_lossy().into_owned();
    let dir = src.parent()?;
    for ext in ["THM", "thm", "JPG", "jpg", "JPEG", "jpeg"] {
        let candidate = dir.join(format!("{stem}.{ext}"));
        if candidate.is_file() && candidate != src {
            return Some(candidate);
        }
    }
    None
}

fn augmented_path() -> String {
    let extras = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/usr/bin",
        "/bin",
    ];
    let mut parts: Vec<String> = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect();
    for extra in extras {
        if !parts.iter().any(|part| part == extra) {
            parts.push(extra.to_string());
        }
    }
    parts.join(":")
}

fn find_ffmpeg() -> Option<PathBuf> {
    static BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
    BIN.get_or_init(|| {
        let path = augmented_path();
        if let Ok(output) = Command::new("/usr/bin/which")
            .env("PATH", &path)
            .arg("ffmpeg")
            .output()
        {
            if output.status.success() {
                let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !found.is_empty() && Path::new(&found).is_file() {
                    return Some(PathBuf::from(found));
                }
            }
        }
        for candidate in [
            "/opt/homebrew/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/usr/bin/ffmpeg",
        ] {
            if Path::new(candidate).is_file() {
                return Some(PathBuf::from(candidate));
            }
        }
        None
    })
    .clone()
}

fn ffmpeg_frame(bin: &Path, src: &Path, dest: &Path, seek_before: bool, ss: &str) -> bool {
    let mut cmd = Command::new(bin);
    cmd.env("PATH", augmented_path())
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    if seek_before {
        cmd.args(["-ss", ss, "-i"]).arg(src);
    } else {
        cmd.arg("-i").arg(src).args(["-ss", ss]);
    }
    cmd.args(["-frames:v", "1", "-vf", "scale=320:-2"])
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success() && dest.is_file() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false))
        .unwrap_or(false)
}

fn qlmanage_poster(src: &Path, dest: &Path) -> bool {
    if cfg!(not(target_os = "macos")) {
        return false;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out_dir = std::env::temp_dir().join(format!("camorg-ql-{stamp}"));
    if std::fs::create_dir_all(&out_dir).is_err() {
        return false;
    }
    let ok = Command::new("/usr/bin/qlmanage")
        .args(["-t", "-s", "320", "-o"])
        .arg(&out_dir)
        .arg(src)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    let mut converted = false;
    if ok {
        if let Ok(entries) = std::fs::read_dir(&out_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && generate_photo(&path, dest) {
                    converted = true;
                    break;
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&out_dir);
    converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use tempfile::tempdir;

    #[test]
    fn writes_jpeg_thumbnail_for_png_photo() {
        let dir = tempdir().unwrap();
        camorg::ensure_layout(dir.path()).unwrap();
        let src = dir.path().join("inbox");
        std::fs::create_dir_all(&src).unwrap();
        let photo = src.join("shot.png");
        let mut img = RgbImage::new(64, 48);
        for pixel in img.pixels_mut() {
            *pixel = Rgb([200, 80, 20]);
        }
        img.save(&photo).unwrap();

        let rel = Path::new("inbox/shot.png");
        let thumb_rel = ensure_thumbnail(dir.path(), &photo, rel, MediaKind::Photo)
            .unwrap()
            .expect("thumbnail");
        let dest = camorg::dir(dir.path()).join(&thumb_rel);
        assert!(dest.is_file());
        assert!(dest.metadata().unwrap().len() > 0);
        assert!(ensure_thumbnail(dir.path(), &photo, rel, MediaKind::Photo)
            .unwrap()
            .is_some());
    }

    #[test]
    fn skips_sidecars() {
        let dir = tempdir().unwrap();
        camorg::ensure_layout(dir.path()).unwrap();
        let srt = dir.path().join("clip.SRT");
        std::fs::write(&srt, b"1").unwrap();
        assert!(ensure_thumbnail(dir.path(), &srt, Path::new("clip.SRT"), MediaKind::Sidecar)
            .unwrap()
            .is_none());
    }

    #[test]
    fn uses_sibling_poster_for_video() {
        let dir = tempdir().unwrap();
        camorg::ensure_layout(dir.path()).unwrap();
        let video = dir.path().join("clip.MP4");
        std::fs::write(&video, b"not-a-real-video").unwrap();
        let poster = dir.path().join("clip.THM");
        let mut img = RgbImage::new(48, 32);
        for pixel in img.pixels_mut() {
            *pixel = Rgb([20, 80, 200]);
        }
        img.save_with_format(&poster, image::ImageFormat::Jpeg).unwrap();
        let thumb_rel = ensure_thumbnail(dir.path(), &video, Path::new("clip.MP4"), MediaKind::Video)
            .unwrap()
            .expect("video thumbnail from THM");
        assert!(camorg::dir(dir.path()).join(thumb_rel).is_file());
    }
}
