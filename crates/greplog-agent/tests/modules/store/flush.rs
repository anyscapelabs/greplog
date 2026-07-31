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
    let dir = std::env::temp_dir().join(format!(
        "greplog_flush_test_{}_{}",
        name,
        std::process::id()
    ));
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
        event_id: String::new(),
    }
}

#[allow(dead_code)]
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
        event_id: String::new(),
    }
}

#[allow(dead_code)]
fn make_metric(service: &str, timestamp_ns: i64, name: &str, value: f64) -> Metric {
    Metric {
        service_name: service.into(),
        name: name.into(),
        value,
        timestamp_ns,
        labels: HashMap::new(),
        r#type: 1,
        bucket_values: Vec::new(),
        event_id: String::new(),
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
        .filter(|p| p.extension().is_some_and(|e| e == "parquet"))
        .collect();
    assert_eq!(files.len(), 1, "exactly one parquet file");

    let parquet_path = &files[0];
    let batches = read_parquet(parquet_path);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 1);

    let batch_schema = batches[0].schema();
    let core_schema = arrow_schema::log_schema();
    assert_eq!(
        &*batch_schema, &core_schema,
        "schema should match core log schema"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_multi_service_grouping() {
    let dir = temp_dir("multi_svc");
    let is_flushing = AtomicBool::new(false);

    let batch1 = IngestBatch {
        service_name: "api".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs: vec![make_log(
            "api",
            1_700_000_000_000_000_000,
            "info",
            "api log",
        )],
        spans: vec![],
        metrics: vec![],
    };
    let batch2 = IngestBatch {
        service_name: "worker".into(),
        instance_id: "i2".into(),
        batch_seq: 2,
        logs: vec![make_log(
            "worker",
            1_700_000_000_000_000_000,
            "debug",
            "worker log",
        )],
        spans: vec![],
        metrics: vec![],
    };

    let result = flush_to_parquet(&dir, &[batch1, batch2], None, &is_flushing).expect("flush");
    assert_eq!(result.files_written, 2);

    let api_dir = dir.join("data").join("table_type=logs").join("service=api");
    let worker_dir = dir
        .join("data")
        .join("table_type=logs")
        .join("service=worker");
    assert!(api_dir.exists(), "api partition dir should exist");
    assert!(worker_dir.exists(), "worker partition dir should exist");

    let _ = fs::remove_dir_all(&dir);
}

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
                entry.extension().is_none_or(|e| e != "tmp"),
                "no .tmp files should remain: {:?}",
                entry
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_checkpoint_correctness() {
    let dir = temp_dir("checkpoint");
    let is_flushing = AtomicBool::new(false);

    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open wal");

    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs: vec![make_log(
            "svc",
            1_700_000_000_000_000_000,
            "info",
            "flush me",
        )],
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
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".wal")))
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

fn make_log_no_event_id(service: &str, ts: i64, msg: &str) -> LogEvent {
    let mut attrs = HashMap::new();
    attrs.insert("k".into(), "v".into());
    LogEvent {
        service_name: service.into(),
        message: msg.into(),
        level: "info".into(),
        timestamp_ns: ts,
        logger_name: "logger".into(),
        file: "f.rs".into(),
        line: 42,
        correlation_id: "corr".into(),
        attributes: attrs,
        stack_trace: Vec::new(),
        exception_type: String::new(),
        exception_message: String::new(),
        event_id: String::new(),
    }
}

fn make_log_with_event_id(service: &str, ts: i64, msg: &str, eid: &str) -> LogEvent {
    let mut log = make_log_no_event_id(service, ts, msg);
    log.event_id = eid.to_string();
    log
}

#[test]
fn test_event_id_present_used_directly() {
    let dir = std::env::temp_dir().join(format!("test_eid_present_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create dir");
    let is_flushing = AtomicBool::new(false);

    let log = make_log_with_event_id("svc", 1_700_000_000_000_000_000, "hello", "my-stable-id-42");
    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i".into(),
        batch_seq: 1,
        logs: vec![log],
        spans: vec![],
        metrics: vec![],
    };

    flush_to_parquet(&dir, &[batch], None, &is_flushing).expect("flush");

    let data_dir = dir.join("data");
    let parquet_files: Vec<_> = walkdir(&data_dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "parquet"))
        .collect();
    assert_eq!(parquet_files.len(), 1);
    let batches = read_parquet(&parquet_files[0]);
    assert_eq!(batches.len(), 1);
    let id_col = batches[0].column(0);
    let id_arr = id_col
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .expect("id col");
    assert_eq!(id_arr.value(0), "my-stable-id-42");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_event_id_absent_falls_back_to_content_hash() {
    let dir = std::env::temp_dir().join(format!("test_eid_absent_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create dir");
    let is_flushing = AtomicBool::new(false);

    let log = make_log_no_event_id("svc", 1_700_000_000_000_000_000, "hello");
    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i".into(),
        batch_seq: 1,
        logs: vec![log],
        spans: vec![],
        metrics: vec![],
    };

    flush_to_parquet(&dir, &[batch], None, &is_flushing).expect("flush");

    let data_dir = dir.join("data");
    let parquet_files: Vec<_> = walkdir(&data_dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "parquet"))
        .collect();
    assert_eq!(parquet_files.len(), 1);
    let batches = read_parquet(&parquet_files[0]);
    assert_eq!(batches.len(), 1);
    let id_col = batches[0].column(0);
    let id_arr = id_col
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .expect("id col");
    let id_value = id_arr.value(0);

    assert_eq!(
        id_value.len(),
        16,
        "content-hash ID should be 16 hex chars, got: {id_value}"
    );
    assert!(
        id_value.chars().all(|c| c.is_ascii_hexdigit()),
        "ID {id_value} should contain only hex characters"
    );
    assert!(
        !id_value.contains('-'),
        "ID {id_value} should not contain UUID dashes"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_crash_simulation() {
    let dir = temp_dir("crash_sim");

    let mut wal = WalWriter::with_config(&dir, Duration::from_secs(60), 999_999).expect("open wal");

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

    let log_date = micros_to_date_string(ns_to_micros(1_700_000_000_000_000_000));
    let partition_dir = dir
        .join("data")
        .join("table_type=logs")
        .join("service=svc")
        .join(format!("date={}", log_date));
    fs::create_dir_all(&partition_dir).expect("create partition dir");

    let tmp_path = partition_dir.join("part-crash-test.parquet.tmp");
    fs::write(&tmp_path, b"partial parquet data").expect("write tmp");

    let old_path = partition_dir.join("part-old.parquet");
    fs::write(&old_path, b"old valid data").expect("write old");

    cleanup_temp_files(&dir).expect("cleanup");

    assert!(!tmp_path.exists(), ".tmp file should be deleted by cleanup");
    assert!(
        old_path.exists(),
        "existing .parquet should survive cleanup"
    );

    let wal_dir = dir.join("wal");
    let segments: Vec<_> = fs::read_dir(&wal_dir)
        .expect("read wal dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".wal")))
        .collect();
    assert!(
        !segments.is_empty(),
        "WAL segments should survive when data was not flushed"
    );

    let replayed = crate::store::wal::replay_segments(&dir).expect("replay");
    assert_eq!(
        replayed.len(),
        1,
        "WAL should still contain the unflushed batch"
    );
    assert_eq!(replayed[0].0.batch_seq, 1);

    wal.close().expect("close wal");
    let _ = fs::remove_dir_all(&dir);
}
