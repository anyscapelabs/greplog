//! The Parquet storage layer: Hive-partitioned columnar files.
//!
//! [`ParquetFlusher`] writes finished [`RecordBatch`]es into
//! `root/year=YYYY/month=MM/day=DD/chunk_*.parquet` chunks using
//! [`ArrowWriter`] with Snappy compression.

use std::fs::File;
use std::path::PathBuf;

use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::EngineError;

/// Writes Arrow batches to Hive-partitioned Parquet chunks under a root dir.
pub struct ParquetFlusher {
    root_dir: PathBuf,
    properties: WriterProperties,
}

impl ParquetFlusher {
    /// Creates a flusher that writes into `root_dir` (e.g. `data/logs/`).
    #[must_use]
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            properties: WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        }
    }

    /// Writes `batch` to a new chunk under today's UTC partition.
    ///
    /// The partition directory is created on demand and the chunk filename is
    /// derived from the wall clock at nanosecond resolution to avoid collisions
    /// between consecutive flushes within the same millisecond.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the partition cannot be created or
    /// the file opened, or [`EngineError::ParquetError`] if the batch fails to
    /// encode or the writer cannot be closed cleanly.
    pub fn flush(&self, batch: &RecordBatch) -> Result<(), EngineError> {
        let now = Utc::now();
        let directory = self.partition_path(now);
        std::fs::create_dir_all(&directory)?;

        let nanos = now
            .timestamp_nanos_opt()
            .unwrap_or_else(|| now.timestamp_millis() * 1_000_000);
        let path = directory.join(format!("chunk_{nanos}.parquet"));

        let file = File::create(&path)?;
        let mut writer =
            ArrowWriter::try_new(file, batch.schema().clone(), Some(self.properties.clone()))?;
        writer.write(batch)?;
        writer.close()?;
        Ok(())
    }

    /// Builds the Hive-style partition directory for `time`.
    fn partition_path(&self, time: DateTime<Utc>) -> PathBuf {
        self.root_dir
            .join(format!("year={}", time.format("%Y")))
            .join(format!("month={}", time.format("%m")))
            .join(format!("day={}", time.format("%d")))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use chrono::{DateTime, Utc};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::tempdir;

    use super::ParquetFlusher;
    use crate::memtable::MemTable;
    use crate::record::LogRecord;

    fn sample(index: usize) -> LogRecord {
        LogRecord {
            timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
            trace_id: Some(format!("trace-{index}")),
            level: "ERROR".to_string(),
            service: "payment-worker".to_string(),
            message: format!("order {index} failed"),
            raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
        }
    }

    fn build_batch(count: usize) -> arrow::record_batch::RecordBatch {
        let mut table = MemTable::new();
        for index in 0..count {
            table.append_record(&sample(index));
        }
        table.finish().expect("finish memtable")
    }

    fn first_parquet(root: &Path) -> Option<PathBuf> {
        let year = fs::read_dir(root).ok()?.next()?.ok()?.path();
        let month = fs::read_dir(&year).ok()?.next()?.ok()?.path();
        let day = fs::read_dir(&month).ok()?.next()?.ok()?.path();
        for entry in fs::read_dir(&day).ok()? {
            let entry = entry.ok()?;
            if entry.path().extension().is_some_and(|ext| ext == "parquet") {
                return Some(entry.path());
            }
        }
        None
    }

    fn row_count(path: &Path) -> usize {
        let file = fs::File::open(path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("parse parquet");
        let reader = builder.build().expect("build record reader");
        reader
            .map(|batch| batch.expect("read batch").num_rows())
            .sum()
    }

    #[test]
    fn flush_writes_valid_hive_partitioned_parquet() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let flusher = ParquetFlusher::new(&root);

        let batch = build_batch(3);
        flusher.flush(&batch).expect("flush parquet");

        let chunk = first_parquet(&root).expect("a chunk must exist under year/month/day");
        let relative = chunk
            .strip_prefix(&root)
            .expect("chunk lives under root")
            .to_string_lossy();
        assert!(
            relative.contains("year=") && relative.contains("month=") && relative.contains("day="),
            "chunk must be Hive-partitioned, got {relative}"
        );

        assert_eq!(row_count(&chunk), 3, "parquet must round-trip all rows");
    }

    #[test]
    fn partition_path_matches_hive_layout() {
        let flusher = ParquetFlusher::new("data/logs");
        let time: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-08-09T12:00:00Z")
            .expect("parse rfc3339")
            .with_timezone(&Utc);

        let path = flusher.partition_path(time);
        assert_eq!(
            path,
            PathBuf::from("data/logs/year=2026/month=08/day=09")
        );
    }
}