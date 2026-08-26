//! Integration tests for the background compactor.
//!
//! The spec scenario writes ten small chunks (one per `ParquetFlusher` call)
//! into a single service partition, then verifies that `Compactor` merges them
//! into one file with every row intact and no original chunk left behind.

use std::fs;
use std::path::{Path, PathBuf};

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::tempdir;

use greplog_engine::config::EngineConfig;
use greplog_engine::memtable::MemTable;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::Compactor;

/// Number of small chunks written during setup.
const CHUNKS: usize = 10;
/// Rows packed into each chunk.
const ROWS_PER_CHUNK: usize = 100;

fn mock_log(index: usize) -> LogRecord {
    LogRecord::new("ERROR", "auth-api", format!("order {index} failed"))
}

fn batch_of(count: usize) -> arrow::record_batch::RecordBatch {
    let mut table = MemTable::new(count);
    for index in 0..count {
        table.append_record(&mock_log(index));
    }
    table.finish().expect("finish memtable")
}

fn collect_leaves(dir: &Path, leaves: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut has_parquet = false;
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "parquet") {
                has_parquet = true;
            }
        }
        if has_parquet {
            leaves.push(dir.to_path_buf());
        }
        for subdir in subdirs {
            collect_leaves(&subdir, leaves);
        }
    }
}

fn leaf_dirs(root: &Path) -> Vec<PathBuf> {
    let mut leaves = Vec::new();
    collect_leaves(root, &mut leaves);
    leaves.sort_unstable();
    leaves
}

fn parquet_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read partition")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| !path.is_dir() && path.extension().is_some_and(|ext| ext == "parquet"))
        .collect();
    files.sort_unstable();
    files
}

fn rows_in(path: &Path) -> usize {
    let file = fs::File::open(path).expect("open parquet");
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("read parquet metadata");
    let reader = builder.build().expect("build record reader");
    reader
        .map(|batch| batch.expect("read batch").num_rows())
        .sum()
}

/// The spec scenario: one partition full of small chunks is merged atomically.
#[test]
fn compact_partition_merges_small_chunks_without_data_loss() {
    let dir = tempdir().expect("create temp dir");
    let root = dir.path().join("logs");
    let config = EngineConfig {
        data_dir: root.clone(),
        ..EngineConfig::default()
    };
    let flusher = ParquetFlusher::new(&config);
    for _ in 0..CHUNKS {
        flusher
            .flush(&batch_of(ROWS_PER_CHUNK))
            .expect("flush chunk");
    }

    let leaves = leaf_dirs(&root);
    assert_eq!(
        leaves.len(),
        1,
        "all chunks must land in one service partition"
    );
    let partition = &leaves[0];
    assert_eq!(
        parquet_files(partition).len(),
        CHUNKS,
        "setup must produce one small file per flush"
    );

    let compactor = Compactor::new(config.clone());
    let found = compactor
        .find_partitions_to_compact()
        .expect("scan partitions");
    assert_eq!(
        found,
        vec![partition.clone()],
        "the crowded partition must be flagged for compaction"
    );

    compactor
        .compact_partition(partition)
        .expect("compact partition");

    let merged = parquet_files(partition);
    assert_eq!(
        merged.len(),
        1,
        "partition must hold exactly one merged file"
    );
    let name = merged[0]
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .expect("merged file has a name");
    assert!(
        name.starts_with("compacted_"),
        "merged file must be the compacted_ output, got {name}"
    );
    assert_eq!(
        rows_in(&merged[0]),
        CHUNKS * ROWS_PER_CHUNK,
        "all rows must survive the merge"
    );
}

/// Partitions below the merge threshold are left untouched by the scanner.
#[test]
fn scanner_ignores_partitions_below_the_merge_threshold() {
    let dir = tempdir().expect("create temp dir");
    let root = dir.path().join("logs");
    let config = EngineConfig {
        data_dir: root.clone(),
        ..EngineConfig::default()
    };
    let flusher = ParquetFlusher::new(&config);
    for _ in 0..3 {
        flusher
            .flush(&batch_of(ROWS_PER_CHUNK))
            .expect("flush chunk");
    }

    let compactor = Compactor::new(config);
    let found = compactor
        .find_partitions_to_compact()
        .expect("scan partitions");
    assert!(
        found.is_empty(),
        "a partition holding 3 chunks must not be compacted"
    );
}
