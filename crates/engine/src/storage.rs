//! The Parquet storage layer: Hive-partitioned columnar files.
//!
//! [`ParquetFlusher`] writes finished [`RecordBatch`]es into
//! `data_dir/year=YYYY/month=MM/day=DD/service=<name>/chunk_*.parquet` chunks
//! using [`ArrowWriter`] with Snappy compression. The base directory comes from
//! the engine configuration; no path is assumed.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use arrow::array::{AsArray, StringArray};
use arrow::compute::{cast, filter_record_batch};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use arrow_ord::cmp::eq;
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::config::EngineConfig;
use crate::error::EngineError;

/// Writes Arrow batches to Hive-partitioned Parquet chunks under the
/// configured `data_dir`.
pub struct ParquetFlusher {
    root_dir: PathBuf,
    properties: WriterProperties,
}

impl ParquetFlusher {
    /// Creates a flusher rooted at the configured `data_dir`.
    #[must_use]
    pub fn new(config: &EngineConfig) -> Self {
        Self {
            root_dir: config.data_dir.clone(),
            properties: WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        }
    }

    /// Writes `batch` to today's UTC partition, split by `service`.
    ///
    /// The unique `service` values present in the batch are discovered with a
    /// comparison kernel; each is filtered out of the batch with
    /// [`filter_record_batch`] and written to its own
    /// `service=<name>` directory. The chunk filename is derived from the wall
    /// clock at nanosecond resolution to avoid collisions between flushes.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if a partition cannot be created or a
    /// file opened, [`EngineError::ArrowError`] if the batch cannot be split, or
    /// [`EngineError::ParquetError`] if a chunk fails to encode. A failure after
    /// some services were already written surfaces as an error, which prevents
    /// the caller from truncating the WAL, so the remaining records are kept.
    pub fn flush(&self, batch: &RecordBatch) -> Result<(), EngineError> {
        let service_column = service_column(batch)?;
        let services = unique_services(service_column.as_ref());
        if services.is_empty() {
            return Ok(());
        }

        let now = Utc::now();
        let date_dir = self.partition_path(now);
        let nanos = now
            .timestamp_nanos_opt()
            .unwrap_or_else(|| now.timestamp_millis() * 1_000_000);

        for service in services {
            let directory = date_dir.join(format!("service={service}"));
            std::fs::create_dir_all(&directory)?;
            let split = filter_by_service(batch, service_column.as_ref(), &service)?;
            self.write_chunk(&directory, nanos, &split)?;
        }
        Ok(())
    }

    /// Writes `batch` to a new chunk file inside `directory`.
    fn write_chunk(
        &self,
        directory: &Path,
        nanos: i64,
        batch: &RecordBatch,
    ) -> Result<(), EngineError> {
        let path = directory.join(format!("chunk_{nanos}.parquet"));
        let file = File::create(&path)?;
        let mut writer =
            ArrowWriter::try_new(file, batch.schema().clone(), Some(self.properties.clone()))?;
        writer.write(batch)?;
        writer.close()?;
        Ok(())
    }

    /// Builds the Hive-style date partition directory for `time`.
    fn partition_path(&self, time: DateTime<Utc>) -> PathBuf {
        self.root_dir
            .join(format!("year={}", time.format("%Y")))
            .join(format!("month={}", time.format("%m")))
            .join(format!("day={}", time.format("%d")))
    }
}

/// Returns the `service` column cast to a plain [`StringArray`].
///
/// The schema's `service` field is dictionary-encoded; the UTF-8 cast makes it
/// directly comparable and iterable.
fn service_column(batch: &RecordBatch) -> Result<arrow::array::ArrayRef, EngineError> {
    let index = batch.schema().index_of("service")?;
    let column = batch.column(index);
    cast(column, &DataType::Utf8).map_err(EngineError::from)
}

/// Returns the sorted unique `service` names present in `service_values`.
///
/// `service_values` is the UTF-8-cast `service` column; it is iterated once and
/// the distinct names are deduplicated.
fn unique_services(service_values: &dyn arrow::array::Array) -> Vec<String> {
    let services = service_values.as_string::<i32>();
    let mut seen = HashSet::new();
    for value in services.iter().flatten() {
        seen.insert(value.to_string());
    }
    let mut names: Vec<String> = seen.into_iter().collect();
    names.sort_unstable();
    names
}

/// Filters out every row whose `service` matches `service_name`.
///
/// The mask comes from the `arrow_ord::cmp::eq` kernel comparing the `service`
/// column against a scalar needle; no row is assembled by hand.
fn filter_by_service(
    batch: &RecordBatch,
    service_values: &dyn arrow::array::Array,
    service_name: &str,
) -> Result<RecordBatch, EngineError> {
    let services = service_values.as_string::<i32>();
    let needle = StringArray::from(vec![service_name]);
    let mask = eq(services, &arrow::array::Scalar::new(&needle))?;
    filter_record_batch(batch, &mask).map_err(EngineError::from)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use chrono::{DateTime, Utc};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::tempdir;

    use super::ParquetFlusher;
    use crate::config::EngineConfig;
    use crate::memtable::MemTable;
    use crate::record::LogRecord;

    fn sample(index: usize, service: &str) -> LogRecord {
        LogRecord {
            timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
            trace_id: Some(format!("trace-{index}")),
            level: "ERROR".to_string(),
            service: service.to_string(),
            message: format!("order {index} failed"),
            raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
        }
    }

    fn build_mixed_batch() -> arrow::record_batch::RecordBatch {
        let mut table = MemTable::new(4);
        table.append_record(&sample(0, "auth-api"));
        table.append_record(&sample(1, "payment-worker"));
        table.append_record(&sample(2, "auth-api"));
        table.finish().expect("finish memtable")
    }

    fn parquet_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(parquet_files(&path));
                } else if path.extension().is_some_and(|ext| ext == "parquet") {
                    files.push(path);
                }
            }
        }
        files
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
    fn flush_splits_batch_by_service() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let config = EngineConfig {
            data_dir: root.clone(),
            ..EngineConfig::default()
        };
        let flusher = ParquetFlusher::new(&config);

        let batch = build_mixed_batch();
        flusher.flush(&batch).expect("flush parquet");

        let chunk = parquet_files(&root);
        assert_eq!(chunk.len(), 2, "one chunk per service must exist");
        for path in &chunk {
            let relative = path.strip_prefix(&root).expect("chunk under root").to_string_lossy();
            assert!(
                relative.contains("year=") && relative.contains("month=") && relative.contains("day="),
                "chunk must be date-partitioned, got {relative}"
            );
            assert!(
                relative.contains("service=auth-api") || relative.contains("service=payment-worker"),
                "chunk must be service-partitioned, got {relative}"
            );
        }

        let per_service = |name: &str| {
            chunk
                .iter()
                .filter(|path| path.to_string_lossy().contains(name))
                .map(|path| row_count(path))
                .sum::<usize>()
        };
        assert_eq!(per_service("service=auth-api"), 2, "auth-api rows must round-trip");
        assert_eq!(
            per_service("service=payment-worker"),
            1,
            "payment-worker rows must round-trip"
        );
    }

    #[test]
    fn partition_path_matches_hive_layout() {
        let config = EngineConfig {
            data_dir: PathBuf::from("data/logs"),
            ..EngineConfig::default()
        };
        let flusher = ParquetFlusher::new(&config);
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