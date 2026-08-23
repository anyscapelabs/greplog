//! Storage statistics for the dashboard, walked straight from the filesystem.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageStats {
    pub bytes: u64,
    pub partitions: usize,
    pub chunks: usize,
}

#[must_use]
pub fn storage_stats(data_dir: &Path) -> StorageStats {
    let mut stats = StorageStats {
        bytes: 0,
        partitions: 0,
        chunks: 0,
    };
    walk_usage(data_dir, &mut stats);
    stats
}

fn walk_usage(dir: &Path, stats: &mut StorageStats) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if is_day_partition(&path) {
                stats.partitions += 1;
            }
            walk_usage(&path, stats);
        } else if path.extension().is_some_and(|ext| ext == "parquet") {
            stats.chunks += 1;
            if let Ok(meta) = fs::metadata(&path) {
                stats.bytes += meta.len();
            }
        }
    }
}

fn is_day_partition(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("day="))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::storage_stats;

    #[test]
    fn missing_root_is_an_empty_snapshot() {
        let stats = storage_stats(std::path::Path::new("/nonexistent/greplog"));
        assert_eq!(stats.bytes, 0);
        assert_eq!(stats.partitions, 0);
        assert_eq!(stats.chunks, 0);
    }

    #[test]
    fn counts_only_day_partitions_and_parquet_chunks() {
        let dir = tempfile::tempdir().expect("tmp");
        let root = dir.path().join("logs");
        let day = root.join("year=2026").join("month=08").join("day=14");
        fs::create_dir_all(day.join("service=auth-api")).expect("dirs");
        fs::write(day.join("service=auth-api").join("chunk_1.parquet"), [0u8; 128]).expect("chunk");
        fs::write(day.join("service=auth-api").join("chunk_1.parquet.part"), [0u8; 5]).expect("part");
        fs::create_dir_all(root.join("year=9999")).expect("decoy");

        let stats = storage_stats(&root);
        assert_eq!(stats.partitions, 1, "only real day= directories count");
        assert_eq!(stats.chunks, 1, "in-progress .part files are not chunks");
        assert_eq!(stats.bytes, 128);
    }
}
