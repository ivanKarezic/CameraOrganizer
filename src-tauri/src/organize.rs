use crate::media::{folder_kind_for_group, grouping_key, MediaKind};
use crate::metadata::DateSource;
use crate::paths::{build_organized_path, disambiguate_filename, is_organized_path};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferAction {
    Move,
    Copy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransferOp {
    pub source: String,
    pub destination: String,
    pub filename: String,
    pub action: TransferAction,
    pub warning: Option<String>,
    pub group_key: String,
}

#[derive(Debug, Clone)]
pub struct PlanFile {
    pub absolute_path: PathBuf,
    pub relative_path: PathBuf,
    pub filename: String,
    pub kind: MediaKind,
    pub captured_on: NaiveDate,
    #[allow(dead_code)]
    pub date_source: DateSource,
    #[allow(dead_code)]
    pub size: u64,
    #[allow(dead_code)]
    pub hash: Option<String>,
}

fn occupied_names_in_dest(dest_dir: &Path, existing: &HashSet<PathBuf>) -> HashSet<String> {
    existing
        .iter()
        .filter_map(|p| {
            if p.parent() == Some(dest_dir) {
                p.file_name()?.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Move unorganized library files into `YYYY/YYYY-MM-DD/Video|Photo`.
pub fn plan_organize(files: &[PlanFile], existing_destinations: &HashSet<PathBuf>) -> Vec<TransferOp> {
    let mut groups: HashMap<String, Vec<&PlanFile>> = HashMap::new();
    for file in files {
        groups.entry(grouping_key(&file.filename)).or_default().push(file);
    }

    let mut planned_dests = existing_destinations.clone();
    let mut ops = Vec::new();

    for (key, members) in groups {
        let kinds: Vec<MediaKind> = members.iter().map(|f| f.kind).collect();
        let dest_kind = folder_kind_for_group(kinds.iter());
        let date = members
            .iter()
            .find(|f| f.kind != MediaKind::Sidecar)
            .or_else(|| members.first())
            .map(|f| f.captured_on)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

        for file in members {
            if is_organized_path(&file.relative_path) {
                continue;
            }
            let dest_rel_dir = build_organized_path(date, dest_kind, "placeholder")
                .parent()
                .unwrap()
                .to_path_buf();
            let occupied = occupied_names_in_dest(&dest_rel_dir, &planned_dests);
            let (final_name, warning) = disambiguate_filename(&file.filename, &occupied);
            let dest_rel = dest_rel_dir.join(&final_name);
            planned_dests.insert(dest_rel.clone());
            ops.push(TransferOp {
                source: file.absolute_path.to_string_lossy().into_owned(),
                destination: dest_rel.to_string_lossy().into_owned(),
                filename: final_name,
                action: TransferAction::Move,
                warning,
                group_key: key.clone(),
            });
        }
    }

    ops.sort_by(|a, b| a.source.cmp(&b.source));
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(rel: &str, name: &str, kind: MediaKind, day: u32) -> PlanFile {
        PlanFile {
            absolute_path: PathBuf::from("/library").join(rel),
            relative_path: PathBuf::from(rel),
            filename: name.into(),
            kind,
            captured_on: NaiveDate::from_ymd_opt(2024, 8, day).unwrap(),
            date_source: DateSource::Filename,
            size: 10,
            hash: None,
        }
    }

    #[test]
    fn moves_only_unorganized_files() {
        let files = vec![
            file(
                "inbox/DJI_20240826_143022_000_0001.MP4",
                "DJI_20240826_143022_000_0001.MP4",
                MediaKind::Video,
                26,
            ),
            file(
                "2024/2024-08-26/Video/keep.MP4",
                "keep.MP4",
                MediaKind::Video,
                26,
            ),
        ];
        let ops = plan_organize(&files, &HashSet::new());
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, TransferAction::Move);
        assert_eq!(
            ops[0].destination,
            "2024/2024-08-26/Video/DJI_20240826_143022_000_0001.MP4"
        );
    }

    #[test]
    fn companions_follow_the_primary_folder() {
        let files = vec![
            file("inbox/clip.MP4", "clip.MP4", MediaKind::Video, 26),
            file("inbox/clip.SRT", "clip.SRT", MediaKind::Sidecar, 26),
        ];
        let ops = plan_organize(&files, &HashSet::new());
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().all(|op| op.destination.contains("/Video/")));
        assert!(ops.iter().any(|op| op.destination.ends_with("clip.SRT")));
    }

    #[test]
    fn never_overwrites_silently() {
        let files = vec![file("inbox/clip.MP4", "clip.MP4", MediaKind::Video, 26)];
        let existing = HashSet::from([PathBuf::from("2024/2024-08-26/Video/clip.MP4")]);
        let ops = plan_organize(&files, &existing);
        assert_eq!(ops[0].filename, "clip_2.MP4");
        assert!(ops[0].warning.as_ref().unwrap().contains("clip_2.MP4"));
    }
}
