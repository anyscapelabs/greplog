use anyhow::Result;
use arrow::array::*;

use arrow::record_batch::RecordBatch;
use greplog_core::arrow_schema;
use greplog_core::gen::IngestBatch;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DATA_DIR: &str = "data";

/// Result of a flush cycle.
#[derive(Debug, Default)]
pub struct FlushResult {
    pub files_written: usize,
}

struct LogRow {
    id: String,
    service: String,
    timestamp: i64,
    level: Option<String>,
    route: Option<String>,
    message: Option<String>,
    attributes: Option<String>,
}

struct SpanRow {
    id: String,
    correlation_id: Option<String>,
    parent_correlation_id: Option<String>,
    service: String,
    name: Option<String>,
    route: Option<String>,
    method: Option<String>,
    status_code: Option<i32>,
    latency_ms: Option<f64>,
    is_error: Option<bool>,
    start_time: i64,
    end_time: i64,
    attributes: Option<String>,
}

struct MetricRow {
    id: String,
    service: String,
    name: String,
    timestamp: i64,
    value: f64,
    labels: Option<String>,
}

/// Flush pending batches to Parquet files, then checkpoint the WAL.
///
/// Groups events by `(service, date)` — computed from each event's
/// `timestamp_ns`/`start_time_ns` — and writes one Parquet file per
/// group using the temp-file → fsync → rename → directory-fsync
/// sequence. Empty groups produce no file.
///
/// If `wal` is provided, WAL segments containing the flushed events are
/// deleted after all files are durably written.
///
/// `is_flushing` must be an `AtomicBool` initially `false`. The function
/// sets it to `true` on entry and clears it on exit. If already `true`,
/// returns an error to prevent concurrent flushes.
pub fn flush_to_parquet(
    base_dir: &Path,
    pending: &[IngestBatch],
    wal: Option<&mut crate::store::wal::WalWriter>,
    is_flushing: &AtomicBool,
) -> Result<FlushResult> {
    if is_flushing.load(Ordering::SeqCst) {
        anyhow::bail!("flush already in progress");
    }
    is_flushing.store(true, Ordering::SeqCst);

    let result = flush_inner(base_dir, pending, wal);

    is_flushing.store(false, Ordering::SeqCst);
    result
}

/// Delete any `.tmp` files found recursively under `base_dir/data/`.
/// These are left behind by a crash during the temp-file phase of a flush.
pub fn cleanup_temp_files(base_dir: &Path) -> Result<()> {
    let data_dir = base_dir.join(DATA_DIR);
    if !data_dir.exists() {
        return Ok(());
    }
    delete_tmp_recursive(&data_dir)?;
    Ok(())
}

fn delete_tmp_recursive(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            delete_tmp_recursive(&path)?;
        } else if path.extension().map_or(false, |ext| ext == "tmp") {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

fn flush_inner(
    base_dir: &Path,
    pending: &[IngestBatch],
    mut wal: Option<&mut crate::store::wal::WalWriter>,
) -> Result<FlushResult> {
    // Rotate the WAL before processing so the segments containing the
    // flushed data are sealed and can be deleted after a successful flush.
    if let Some(w) = wal.as_mut() {
        w.rotate()?;
    }

    let mut log_groups: Vec<((String, String), Vec<LogRow>)> = Vec::new();
    let mut span_groups: Vec<((String, String), Vec<SpanRow>)> = Vec::new();
    let mut metric_groups: Vec<((String, String), Vec<MetricRow>)> = Vec::new();

    for batch in pending {
        let service = if batch.service_name.is_empty() {
            "default"
        } else {
            &batch.service_name
        };

        for log in &batch.logs {
            let date = micros_to_date_string(ns_to_micros(log.timestamp_ns));
            let key = (service.to_string(), date);
            let id = uuid::Uuid::new_v4().to_string();
            let attrs = if log.attributes.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&log.attributes).unwrap_or_default())
            };
            push_group(&mut log_groups, key, LogRow {
                id,
                service: service.to_string(),
                timestamp: ns_to_micros(log.timestamp_ns),
                level: non_empty(&log.level),
                route: None,
                message: non_empty(&log.message),
                attributes: attrs,
            });
        }

        for span in &batch.spans {
            let date = micros_to_date_string(ns_to_micros(span.start_time_ns));
            let key = (service.to_string(), date);
            let id = uuid::Uuid::new_v4().to_string();
            let attrs = if span.attributes.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&span.attributes).unwrap_or_default())
            };
            push_group(&mut span_groups, key, SpanRow {
                id,
                correlation_id: non_empty(&span.correlation_id),
                parent_correlation_id: non_empty(&span.parent_correlation_id),
                service: service.to_string(),
                name: non_empty(&span.name),
                route: None,
                method: None,
                status_code: None,
                latency_ms: None,
                is_error: Some(!span.error.is_empty()),
                start_time: ns_to_micros(span.start_time_ns),
                end_time: ns_to_micros(span.end_time_ns),
                attributes: attrs,
            });
        }

        for metric in &batch.metrics {
            let date = micros_to_date_string(ns_to_micros(metric.timestamp_ns));
            let key = (service.to_string(), date);
            let id = uuid::Uuid::new_v4().to_string();
            let labels = if metric.labels.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&metric.labels).unwrap_or_default())
            };
            push_group(&mut metric_groups, key, MetricRow {
                id,
                service: service.to_string(),
                name: metric.name.clone(),
                timestamp: ns_to_micros(metric.timestamp_ns),
                value: metric.value,
                labels,
            });
        }
    }

    let mut files_written = 0;

    for ((service, date), rows) in &log_groups {
        let batch = build_log_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=logs")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    for ((service, date), rows) in &span_groups {
        let batch = build_span_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=spans")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    for ((service, date), rows) in &metric_groups {
        let batch = build_metric_batch(rows)?;
        let dir = base_dir
            .join(DATA_DIR)
            .join("table_type=metrics")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        write_parquet(&dir, &batch)?;
        files_written += 1;
    }

    if files_written > 0 {
        if let Some(wal) = wal {
            let checkpoint_seq = wal.current_seq();
            wal.delete_segments_up_to(checkpoint_seq)?;
        }
    }

    Ok(FlushResult { files_written })
}

fn push_group<T>(groups: &mut Vec<((String, String), Vec<T>)>, key: (String, String), row: T) {
    match groups.iter_mut().find(|(k, _)| *k == key) {
        Some((_, rows)) => rows.push(row),
        None => groups.push((key, vec![row])),
    }
}

fn build_log_batch(rows: &[LogRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::log_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.timestamp).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.level.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.route.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.message.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.attributes.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build log batch: {}", e))
}

fn build_span_batch(rows: &[SpanRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::span_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.correlation_id.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.parent_correlation_id.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.name.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.route.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.method.as_deref()).collect::<Vec<Option<&str>>>())),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.status_code).collect::<Vec<Option<i32>>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.latency_ms).collect::<Vec<Option<f64>>>())),
            Arc::new(BooleanArray::from(rows.iter().map(|r| r.is_error).collect::<Vec<Option<bool>>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.start_time).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.end_time).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.attributes.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build span batch: {}", e))
}

fn build_metric_batch(rows: &[MetricRow]) -> Result<RecordBatch> {
    let schema = arrow_schema::metric_schema();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.service.as_str()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>())),
            Arc::new(TimestampMicrosecondArray::from(rows.iter().map(|r| r.timestamp).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.value).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.labels.as_deref()).collect::<Vec<Option<&str>>>())),
        ],
    )
    .map_err(|e| anyhow::anyhow!("build metric batch: {}", e))
}

fn write_parquet(dir: &Path, batch: &RecordBatch) -> Result<()> {
    crate::store::io::write_parquet_atomic(dir, batch)?;
    Ok(())
}

/// Convert a microsecond-precision Unix timestamp to `YYYY-MM-DD` in UTC.
fn micros_to_date_string(micros: i64) -> String {
    let secs = if micros >= 0 {
        micros / 1_000_000
    } else {
        (micros + 1) / 1_000_000 - 1
    };
    let days = if secs >= 0 {
        secs / 86400
    } else {
        (secs + 1) / 86400 - 1
    };

    // Convert days since 1970-01-01 to (y, m, d) using Howard Hinnant's
    // public-domain civil-date algorithm.
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let day = doy - (153 * mp + 2) / 5 + 1;
    let year = if month <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn ns_to_micros(ns: i64) -> i64 {
    ns / 1000
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::wal::WalWriter;
    use greplog_core::gen::{LogEvent, Metric, Span};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_flush_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_log(service: &str, timestamp_ns: i64, level: &str, msg: &str) -> LogEvent {
        let mut attrs = HashMap::new();
        if !msg.is_empty() {
            attrs.insert("key".into(), "val".into());
        }
        LogEvent {
            service_name: service.into(),
            message: msg.into(),
            level: level.into(),
            timestamp_ns,
            logger_name: String::new(),
            file: String::new(),
            line: 0,
            correlation_id: String::new(),
            attributes: attrs,
            stack_trace: Vec::new(),
            exception_type: String::new(),
            exception_message: String::new(),
        }
    }

    fn make_span(service: &str, start_ns: i64, end_ns: i64, name: &str) -> Span {
        Span {
            service_name: service.into(),
            name: name.into(),
            start_time_ns: start_ns,
            end_time_ns: end_ns,
            correlation_id: String::new(),
            parent_correlation_id: String::new(),
            route: String::new(),
            method: String::new(),
            status_code: 0,
            error: String::new(),
            attributes: HashMap::new(),
            kind: 0,
        }
    }

    fn make_metric(service: &str, timestamp_ns: i64, name: &str, value: f64) -> Metric {
        Metric {
            service_name: service.into(),
            name: name.into(),
            value,
            timestamp_ns,
            labels: HashMap::new(),
            r#type: 1,
            bucket_values: Vec::new(),
        }
    }

    fn read_parquet(path: &Path) -> Vec<RecordBatch> {
        let file = File::open(path).expect("open parquet");
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .expect("build reader")
            .build()
            .expect("build record reader");
        reader.collect::<Result<Vec<_>, _>>().expect("read batches")
    }

    fn walkdir(dir: &Path) -> Vec<PathBuf> {
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    result.extend(walkdir(&path));
                }
                result.push(path);
            }
        }
        result
    }

    /// Test 1: Basic flush correctness.
    #[test]
    fn test_basic_flush() {
        let dir = temp_dir("basic_flush");
        let is_flushing = AtomicBool::new(false);

        let batch = IngestBatch {
            service_name: "api".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log("api", 1_700_000_000_000_000_000, "info", "hello")],
            spans: vec![],
            metrics: vec![],
        };

        let result = flush_to_parquet(&dir, &[batch], None, &is_flushing).expect("flush");
        assert_eq!(result.files_written, 1);

        let data_dir = dir.join("data");
        assert!(data_dir.exists(), "data dir should exist");

        let files: Vec<_> = walkdir(&data_dir)
            .into_iter()
            .filter(|p| p.extension().map_or(false, |e| e == "parquet"))
            .collect();
        assert_eq!(files.len(), 1, "exactly one parquet file");

        let parquet_path = &files[0];
        let batches = read_parquet(parquet_path);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 1);

        let batch_schema = batches[0].schema();
        let core_schema = arrow_schema::log_schema();
        assert_eq!(
            &*batch_schema,
            &core_schema,
            "schema should match core log schema"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Test 2: Multi-service grouping.
    #[test]
    fn test_multi_service_grouping() {
        let dir = temp_dir("multi_svc");
        let is_flushing = AtomicBool::new(false);

        let batch1 = IngestBatch {
            service_name: "api".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log("api", 1_700_000_000_000_000_000, "info", "api log")],
            spans: vec![],
            metrics: vec![],
        };
        let batch2 = IngestBatch {
            service_name: "worker".into(),
            instance_id: "i2".into(),
            batch_seq: 2,
            logs: vec![make_log("worker", 1_700_000_000_000_000_000, "debug", "worker log")],
            spans: vec![],
            metrics: vec![],
        };

        let result = flush_to_parquet(&dir, &[batch1, batch2], None, &is_flushing).expect("flush");
        assert_eq!(result.files_written, 2);

        let api_dir = dir.join("data").join("table_type=logs").join("service=api");
        let worker_dir = dir.join("data").join("table_type=logs").join("service=worker");
        assert!(api_dir.exists(), "api partition dir should exist");
        assert!(worker_dir.exists(), "worker partition dir should exist");

        let _ = fs::remove_dir_all(&dir);
    }

    /// Test 3: No .tmp files left behind on success.
    #[test]
    fn test_no_temp_files_after_success() {
        let dir = temp_dir("no_tmp");
        let is_flushing = AtomicBool::new(false);

        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log("svc", 1_700_000_000_000_000_000, "info", "msg")],
            spans: vec![],
            metrics: vec![],
        };

        flush_to_parquet(&dir, &[batch], None, &is_flushing).expect("flush");

        let data_dir = dir.join("data");
        if data_dir.exists() {
            for entry in walkdir(&data_dir) {
                assert!(
                    entry.extension().map_or(true, |e| e != "tmp"),
                    "no .tmp files should remain: {:?}",
                    entry
                );
            }
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// Test 4: Checkpoint correctness — WAL segments are deleted after flush.
    #[test]
    fn test_checkpoint_correctness() {
        let dir = temp_dir("checkpoint");
        let is_flushing = AtomicBool::new(false);

        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open wal");

        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log("svc", 1_700_000_000_000_000_000, "info", "flush me")],
            spans: vec![],
            metrics: vec![],
        };
        wal.append(&batch).expect("append");

        let result = flush_to_parquet(&dir, &[batch], Some(&mut wal), &is_flushing).expect("flush");
        assert_eq!(result.files_written, 1);

        let wal_dir = dir.join("wal");
        let remaining: Vec<_> = fs::read_dir(&wal_dir)
            .expect("read wal dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().map_or(false, |n| n.ends_with(".wal")))
            .collect();
        assert!(
            remaining.len() <= 1,
            "expected at most 1 WAL segment after checkpoint, got {}",
            remaining.len()
        );

        let replayed = crate::store::wal::replay_segments(&dir).expect("replay");
        assert!(
            replayed.is_empty(),
            "no events should remain in WAL after checkpoint"
        );

        wal.close().expect("close wal");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Test 5: Empty buffer produces no file.
    #[test]
    fn test_empty_buffer_no_file() {
        let dir = temp_dir("empty_buffer");
        let is_flushing = AtomicBool::new(false);

        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![],
            spans: vec![],
            metrics: vec![],
        };

        let result = flush_to_parquet(&dir, &[batch], None, &is_flushing).expect("flush");
        assert_eq!(result.files_written, 0);

        let data_dir = dir.join("data");
        if data_dir.exists() {
            let entries: Vec<_> = fs::read_dir(&data_dir)
                .expect("read data dir")
                .filter_map(|e| e.ok())
                .collect();
            assert!(entries.is_empty(), "no files should exist for empty buffer");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// Test 6: Crash simulation — .tmp file is cleaned up on startup,
    /// WAL survives for the unflushed data.
    #[test]
    fn test_crash_simulation() {
        let dir = temp_dir("crash_sim");

        let mut wal =
            WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open wal");

        let batch = IngestBatch {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            batch_seq: 1,
            logs: vec![make_log(
                "svc",
                1_700_000_000_000_000_000,
                "info",
                "crash me",
            )],
            spans: vec![],
            metrics: vec![],
        };
        wal.append(&batch).expect("append");

        // Manually create a .tmp file in the expected partition dir,
        // simulating a crash that happened after Parquet write but
        // before atomic rename.
        let log_date = micros_to_date_string(ns_to_micros(1_700_000_000_000_000_000));
        let partition_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=svc")
            .join(format!("date={}", log_date));
        fs::create_dir_all(&partition_dir).expect("create partition dir");

        let tmp_path = partition_dir.join("part-crash-test.parquet.tmp");
        fs::write(&tmp_path, b"partial parquet data").expect("write tmp");

        // Also leave a prior valid .parquet file to confirm cleanup
        // does not touch non-.tmp files.
        let old_path = partition_dir.join("part-old.parquet");
        fs::write(&old_path, b"old valid data").expect("write old");

        // Run startup cleanup
        cleanup_temp_files(&dir).expect("cleanup");

        assert!(!tmp_path.exists(), ".tmp file should be deleted by cleanup");
        assert!(old_path.exists(), "existing .parquet should survive cleanup");

        let wal_dir = dir.join("wal");
        let segments: Vec<_> = fs::read_dir(&wal_dir)
            .expect("read wal dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().map_or(false, |n| n.ends_with(".wal")))
            .collect();
        assert!(
            !segments.is_empty(),
            "WAL segments should survive when data was not flushed"
        );

        let replayed = crate::store::wal::replay_segments(&dir).expect("replay");
        assert_eq!(replayed.len(), 1, "WAL should still contain the unflushed batch");
        assert_eq!(replayed[0].0.batch_seq, 1);

        wal.close().expect("close wal");
        let _ = fs::remove_dir_all(&dir);
    }
}
