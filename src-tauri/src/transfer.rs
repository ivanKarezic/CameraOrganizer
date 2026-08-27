use crate::error::AppResult;
use crate::organize::{TransferAction, TransferOp};
use std::fs;
use std::path::Path;

#[allow(dead_code)]
pub fn execute_ops(storage_root: &Path, ops: &[TransferOp]) -> AppResult<Vec<String>> {
    execute_ops_with_progress(storage_root, ops, |_, _, _| {})
}

pub fn execute_ops_with_progress<F>(
    storage_root: &Path,
    ops: &[TransferOp],
    mut on_progress: F,
) -> AppResult<Vec<String>>
where
    F: FnMut(usize, usize, &str),
{
    let mut destinations = Vec::new();
    let total = ops.len();
    for (index, op) in ops.iter().enumerate() {
        on_progress(index + 1, total, &op.filename);
        let dest = {
            let raw = Path::new(&op.destination);
            if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                storage_root.join(raw)
            }
        };
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        match op.action {
            TransferAction::Move => move_file(Path::new(&op.source), &dest)?,
            TransferAction::Copy => {
                fs::copy(&op.source, &dest)?;
            }
        }
        destinations.push(dest.to_string_lossy().into_owned());
    }
    Ok(destinations)
}

pub fn move_file(from: &Path, to: &Path) -> AppResult<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to)?;
            fs::remove_file(from)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organize::TransferOp;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn moves_existing_and_copies_new() {
        let dir = tempdir().unwrap();
        let library = dir.path().join("library");
        let card = dir.path().join("card");
        fs::create_dir_all(library.join("inbox")).unwrap();
        fs::create_dir_all(&card).unwrap();
        let existing = library.join("inbox/DJI_20240826_143022_000_0001.MP4");
        let incoming = card.join("GH010123.MP4");
        fs::write(&existing, b"lib").unwrap();
        fs::write(&incoming, b"sd").unwrap();

        execute_ops(
            &library,
            &[TransferOp {
                source: existing.to_string_lossy().into_owned(),
                destination: "2024/2024-08-26/Video/DJI_20240826_143022_000_0001.MP4".into(),
                filename: "DJI_20240826_143022_000_0001.MP4".into(),
                action: TransferAction::Move,
                warning: None,
                group_key: "a".into(),
            }],
        )
        .unwrap();
        assert!(!existing.exists());
        assert!(library
            .join("2024/2024-08-26/Video/DJI_20240826_143022_000_0001.MP4")
            .exists());

        execute_ops(
            &library,
            &[TransferOp {
                source: incoming.to_string_lossy().into_owned(),
                destination: "2024/2024-08-26/Video/GH010123.MP4".into(),
                filename: "GH010123.MP4".into(),
                action: TransferAction::Copy,
                warning: None,
                group_key: "b".into(),
            }],
        )
        .unwrap();
        assert!(incoming.exists());
        assert!(library.join("2024/2024-08-26/Video/GH010123.MP4").exists());
    }

    #[test]
    fn reports_progress_for_each_file() {
        let dir = tempdir().unwrap();
        let library = dir.path().join("library");
        fs::create_dir_all(library.join("inbox")).unwrap();
        let a = library.join("inbox/a.MP4");
        let b = library.join("inbox/b.MP4");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let mut seen = Vec::new();
        execute_ops_with_progress(
            &library,
            &[
                TransferOp {
                    source: a.to_string_lossy().into_owned(),
                    destination: "2024/2024-08-26/Video/a.MP4".into(),
                    filename: "a.MP4".into(),
                    action: TransferAction::Move,
                    warning: None,
                    group_key: "a".into(),
                },
                TransferOp {
                    source: b.to_string_lossy().into_owned(),
                    destination: "2024/2024-08-26/Video/b.MP4".into(),
                    filename: "b.MP4".into(),
                    action: TransferAction::Move,
                    warning: None,
                    group_key: "b".into(),
                },
            ],
            |current, total, filename| seen.push((current, total, filename.to_string())),
        )
        .unwrap();
        assert_eq!(
            seen,
            vec![
                (1, 2, "a.MP4".into()),
                (2, 2, "b.MP4".into())
            ]
        );
    }
}
