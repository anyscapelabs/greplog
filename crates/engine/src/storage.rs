//! The Parquet storage layer: Hive-partitioned columnar files.
//!
//! [`ParquetFlusher`] writes finished [`RecordBatch`]es into
//! `data_dir/year=YYYY/month=MM/day=DD/service=<name>/chunk_*.parquet` chunks
//! with Snappy compression.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use arrow::array::{AsArray, StringArray};
use arrow::compute::{cast, filter_record_batch};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use arrow_ord::cmp::eq;
use chrono::Utc;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::config::EngineConfig;
use crate::error::{parse_error, EngineError};

pub struct ParquetFlusher {
    root_dir: PathBuf,
    properties: WriterProperties,
}

impl ParquetFlusher {
    #[must_use]
    pub fn new(config: &EngineConfig) -> Self {
        Self {
            root_dir: config.data_dir.clone(),
            properties: WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        }
    }

    /// Writes `batch` split by `service` and by each row's own timestamp day.
    ///
    /// Partitioning follows the record, not the flush clock: a late record
    /// lands in the day directory its `timestamp_us` points at, so date
    /// windows stay truthful.
    pub fn flush(&self, batch: &RecordBatch) -> Result<(), EngineError> {
        let service_column = service_column(batch)?;
        let services = unique_services(service_column.as_ref());
        if services.is_empty() {
            return Ok(());
        }

        let nanos = now_nanos();

        for service in services {
            let split = filter_by_service(batch, service_column.as_ref(), &service)?;
            let split = strip_service_column(&split)?;
            for (year, month, day) in unique_days(&split)? {
                let day_batch = rows_for_day(&split, year, month, day)?;
                let directory = self
                    .partition_path(year, month, day)
                    .join(format!("service={service}"));
                std::fs::create_dir_all(&directory)?;
                self.write_chunk(&directory, nanos, &day_batch)?;
            }
        }
        Ok(())
    }

    fn write_chunk(
        &self,
        directory: &Path,
        nanos: i64,
        batch: &RecordBatch,
    ) -> Result<(), EngineError> {
        let final_path = directory.join(format!("chunk_{nanos}.parquet"));
        // Trailing `.part`: the listing table matches `*.parquet`, so a temp
        // file with that extension would be scanned (and fail to parse)
        // mid-write. Only the atomic rename below makes the chunk visible.
        let temp_path = directory.join(format!("chunk_{nanos}.parquet.part"));
        let file = File::create(&temp_path)?;
        let mut writer =
            ArrowWriter::try_new(file, batch.schema().clone(), Some(self.properties.clone()))?;
        writer.write(batch)?;
        writer.close()?;
        std::fs::rename(&temp_path, &final_path)?;
        Ok(())
    }

    /// Hive-style date partition directory for `time`.
    /// Hive-style `year=/month=/day=` directory for a record's own date.
    fn partition_path(&self, year: i32, month: u32, day: u32) -> PathBuf {
        self.root_dir
            .join(format!("year={year}"))
            .join(format!("month={month:02}"))
            .join(format!("day={day:02}"))
    }
}

/// Wall-clock nanos for chunk filenames — uniqueness only, never partitioning.
fn now_nanos() -> i64 {
    let now = Utc::now();
    now.timestamp_nanos_opt()
        .unwrap_or_else(|| now.timestamp_millis() * 1_000_000)
}

/// Distinct `(year, month, day)` dates present in the batch's timestamp
/// column, ascending.
fn unique_days(batch: &RecordBatch) -> Result<Vec<(i32, u32, u32)>, EngineError> {
    let timestamps = timestamp_micros(batch)?;
    let mut days: Vec<(i32, u32, u32)> = Vec::new();
    for micros in timestamps {
        let date = micros_to_date(micros);
        if !days.contains(&date) {
            days.push(date);
        }
    }
    days.sort_unstable();
    Ok(days)
}

/// The sub-batch whose rows fall on `year-month-day`, via an index take.
fn rows_for_day(
    batch: &RecordBatch,
    year: i32,
    month: u32,
    day: u32,
) -> Result<RecordBatch, EngineError> {
    let timestamps = timestamp_micros(batch)?;
    let indices: Vec<u32> = timestamps
        .iter()
        .enumerate()
        .filter(|(_, micros)| micros_to_date(**micros) == (year, month, day))
        .filter_map(|(index, _)| u32::try_from(index).ok())
        .collect();
    if indices.is_empty() {
        return Err(parse_error("no rows match the requested day"));
    }
    let indices_array = arrow::array::UInt32Array::from(indices);
    Ok(arrow::compute::take_record_batch(batch, &indices_array)?)
}

fn timestamp_micros(batch: &RecordBatch) -> Result<Vec<i64>, EngineError> {
    use arrow::array::AsArray;

    let index = batch.schema().index_of("timestamp_us")?;
    let column = batch.column(index);
    let micros: Vec<i64> = match column.data_type() {
        DataType::Timestamp(_, _) => (0..column.len())
            .map(|row| {
                let value = column.as_primitive::<arrow::datatypes::TimestampMicrosecondType>();
                value.value(row)
            })
            .collect(),
        DataType::Int64 => column.as_primitive::<arrow::datatypes::Int64Type>().values().to_vec(),
        other => return Err(parse_error(format!("unexpected timestamp type: {other}"))),
    };
    Ok(micros)
}

fn micros_to_date(micros: i64) -> (i32, u32, u32) {
    use chrono::Datelike;

    let seconds = micros.div_euclid(1_000_000);
    let nanos = u32::try_from(micros.rem_euclid(1_000_000)).unwrap_or(0) * 1_000;
    let date = chrono::DateTime::from_timestamp(seconds, nanos)
        .unwrap_or_else(Utc::now)
        .date_naive();
    (date.year(), date.month(), date.day())
}

fn service_column(batch: &RecordBatch) -> Result<arrow::array::ArrayRef, EngineError> {
    let index = batch.schema().index_of("service")?;
    let column = batch.column(index);
    cast(column, &DataType::Utf8).map_err(EngineError::from)
}

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

fn strip_service_column(batch: &RecordBatch) -> Result<RecordBatch, EngineError> {
    let service_index = batch.schema().index_of("service")?;
    let indices: Vec<usize> = (0..batch.num_columns())
        .filter(|&index| index != service_index)
        .collect();
    batch.project(&indices).map_err(EngineError::from)
}



/// Rows stored in a Parquet chunk, read from its footer metadata.
///
/// `None` when the file cannot be opened or parsed — callers treat that as
/// "unknown size" rather than zero.
#[must_use]
pub fn chunk_row_count(path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).ok()?;
    Some(u64::try_from(builder.metadata().file_metadata().num_rows()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use chrono::Utc;
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

    fn stored_field_names(path: &Path) -> Vec<String> {
        let file = fs::File::open(path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("parse parquet");
        builder
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect()
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
        for path in &chunk {
            assert!(
                !stored_field_names(path).iter().any(|name| name == "service"),
                "service must live only in the folder name"
            );
        }
    }

    #[test]
    fn flush_partitions_follow_the_record_timestamp_not_the_clock() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path().join("logs");
        let config = EngineConfig {
            data_dir: root.clone(),
            ..EngineConfig::default()
        };
        let flusher = ParquetFlusher::new(&config);

        // One row stamped five days ago, one now: the late row must land in
        // its own day directory, not today's.
        let now_us = Utc::now().timestamp_micros();
        let late_us = now_us - 5 * 86_400 * 1_000_000;
        let mut table = MemTable::new(2);
        table.append_record(&LogRecord {
            timestamp_us: late_us,
            trace_id: Some("late".into()),
            level: "ERROR".into(),
            service: "auth-api".into(),
            message: "old failure".into(),
            raw_body: None,
        });
        table.append_record(&LogRecord {
            timestamp_us: now_us,
            trace_id: Some("fresh".into()),
            level: "INFO".into(),
            service: "auth-api".into(),
            message: "current".into(),
            raw_body: None,
        });
        flusher.flush(&table.finish().expect("finish memtable")).expect("flush");

        let chunks = parquet_files(&root);
        assert_eq!(chunks.len(), 2, "one chunk per day");
        let late_date = chrono::DateTime::from_timestamp(
            late_us.div_euclid(1_000_000),
            0,
        )
        .expect("valid timestamp");
        let late_dir = format!("day={:02}", chrono::Datelike::day(&late_date.date_naive()));
        let late_path = chunks.iter().find(|path| path.to_string_lossy().contains(&late_dir))
            .expect("late row has its own day partition");
        assert_eq!(row_count(late_path), 1, "only the late row lives there");

        let fresh_date = Utc::now().date_naive();
        let fresh_dir = format!("day={:02}", chrono::Datelike::day(&fresh_date));
        assert!(
            chunks.iter().any(|path| path.to_string_lossy().contains(&fresh_dir)),
            "fresh row lands in today's partition"
        );
    }

    #[test]
    fn partition_path_matches_hive_layout() {
        let config = EngineConfig {
            data_dir: PathBuf::from("data/logs"),
            ..EngineConfig::default()
        };
        let flusher = ParquetFlusher::new(&config);

        let path = flusher.partition_path(2026, 8, 9);
        assert_eq!(
            path,
            PathBuf::from("data/logs/year=2026/month=08/day=09")
        );
    }
}
