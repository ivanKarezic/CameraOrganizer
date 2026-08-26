use crate::media::{folder_kind_for_group, grouping_key, MediaKind};
use crate::organize::{TransferAction, TransferOp};
use crate::paths::{build_organized_path, disambiguate_filename};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LibraryRecord {
    pub filename: String,
    pub size: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExternalFile {
    pub absolute_path: PathBuf,
    pub filename: String,
    pub kind: MediaKind,
    pub captured_on: NaiveDate,
    pub size: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Default)]
pub struct LibraryIndex {
    name_size: HashSet<(String, u64)>,
    hashes: HashSet<String>,
}

impl LibraryIndex {
    pub fn from_records(records: impl IntoIterator<Item = LibraryRecord>) -> Self {
        let mut index = Self::default();
        for record in records {
            index
                .name_size
                .insert((record.filename.to_ascii_lowercase(), record.size));
            if let Some(hash) = record.hash {
                index.hashes.insert(hash);
            }
        }
        index
    }

    pub fn contains_name_size(&self, filename: &str, size: u64) -> bool {
        self.name_size
            .contains(&(filename.to_ascii_lowercase(), size))
    }

    pub fn contains_hash(&self, hash: &str) -> bool {
        self.hashes.contains(hash)
    }
}

fn is_duplicate(file: &ExternalFile, library: &LibraryIndex) -> bool {
    if library.contains_name_size(&file.filename, file.size) {
        return true;
    }
    if let Some(hash) = &file.hash {
        if library.contains_hash(hash) {
            return true;
        }
    }
    false
}

/// Copy missing external files into date folders on the chosen library storage.
pub fn plan_sync(
    files: &[ExternalFile],
    library: &LibraryIndex,
    existing_destinations: &HashSet<PathBuf>,
) -> Vec<TransferOp> {
    let mut groups: HashMap<String, Vec<&ExternalFile>> = HashMap::new();
    for file in files {
        groups.entry(grouping_key(&file.filename)).or_default().push(file);
    }

    let mut planned = existing_destinations.clone();
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

        let all_dup = members.iter().all(|f| is_duplicate(f, library));
        if all_dup {
            continue;
        }

        for file in members {
            if is_duplicate(file, library) {
                continue;
            }
            let dest_rel_dir = build_organized_path(date, dest_kind, "placeholder")
                .parent()
                .unwrap()
                .to_path_buf();
            let occupied: HashSet<String> = planned
                .iter()
                .filter_map(|p| {
                    if p.parent() == Some(Path::new(&dest_rel_dir)) {
                        p.file_name()?.to_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            let (final_name, warning) = disambiguate_filename(&file.filename, &occupied);
            let dest_rel = dest_rel_dir.join(&final_name);
            planned.insert(dest_rel.clone());
            ops.push(TransferOp {
                source: file.absolute_path.to_string_lossy().into_owned(),
                destination: dest_rel.to_string_lossy().into_owned(),
                filename: final_name,
                action: TransferAction::Copy,
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

    fn ext(name: &str, kind: MediaKind, size: u64, hash: Option<&str>) -> ExternalFile {
        ExternalFile {
            absolute_path: PathBuf::from("/sd").join(name),
            filename: name.into(),
            kind,
            captured_on: NaiveDate::from_ymd_opt(2024, 8, 26).unwrap(),
            size,
            hash: hash.map(|h| h.to_string()),
        }
    }

    #[test]
    fn copies_missing_into_date_folders() {
        let library = LibraryIndex::from_records([LibraryRecord {
            filename: "keep.MP4".into(),
            size: 10,
            hash: Some("aaa".into()),
        }]);
        let files = vec![
            ext("keep.MP4", MediaKind::Video, 10, Some("aaa")),
            ext(
                "DJI_20240826_143022_000_0001.MP4",
                MediaKind::Video,
                20,
                Some("bbb"),
            ),
        ];
        let ops = plan_sync(&files, &library, &HashSet::new());
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].action, TransferAction::Copy);
        assert_eq!(
            ops[0].destination,
            "2024/2024-08-26/Video/DJI_20240826_143022_000_0001.MP4"
        );
    }

    #[test]
    fn skips_hash_duplicates_even_when_names_differ() {
        let library = LibraryIndex::from_records([LibraryRecord {
            filename: "library.MP4".into(),
            size: 99,
            hash: Some("same".into()),
        }]);
        let files = vec![ext("card.MP4", MediaKind::Video, 40, Some("same"))];
        let ops = plan_sync(&files, &library, &HashSet::new());
        assert!(ops.is_empty());
    }

    #[test]
    fn dest_name_collision_gets_suffix() {
        let library = LibraryIndex::from_records([]);
        let files = vec![ext("clip.MP4", MediaKind::Video, 1, Some("z"))];
        let existing = HashSet::from([PathBuf::from("2024/2024-08-26/Video/clip.MP4")]);
        let ops = plan_sync(&files, &library, &existing);
        assert_eq!(ops[0].filename, "clip_2.MP4");
        assert!(ops[0].warning.is_some());
    }

    #[test]
    fn never_dumps_into_storage_root() {
        let library = LibraryIndex::from_records([]);
        let files = vec![ext("shot.JPG", MediaKind::Photo, 8, None)];
        let ops = plan_sync(&files, &library, &HashSet::new());
        assert!(ops[0].destination.starts_with("2024/2024-08-26/Photo/"));
    }
}
