use super::*;
use crate::store::flush::flush_parquet_only;
use greplog_core::gen::{LogEvent, Metric, Span};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "greplog_dedup_test_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn make_log(service: &str, timestamp_ns: i64, level: &str, msg: &str) -> LogEvent {
    LogEvent {
        service_name: service.into(),
        message: msg.into(),
        level: level.into(),
        timestamp_ns,
        logger_name: String::new(),
        file: String::new(),
        line: 0,
        correlation_id: String::new(),
        attributes: HashMap::new(),
        stack_trace: Vec::new(),
        exception_type: String::new(),
        exception_message: String::new(),
        event_id: String::new(),
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
        event_id: String::new(),
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
        event_id: String::new(),
    }
}

#[test]
fn test_empty_cache() {
    let cache = DedupCache::new(1000);
    assert_eq!(cache.len(), 0);
    assert!(!cache.contains("nonexistent"));
}

#[test]
fn test_seed_from_parquet() {
    let dir = test_dir("seed");

    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs: vec![make_log(
            "svc",
            1_700_000_000_000_000_000,
            "info",
            "seed me",
        )],
        spans: vec![make_span(
            "svc",
            1_700_000_000_000_000_000,
            1_700_000_000_010_000_000,
            "op",
        )],
        metrics: vec![make_metric("svc", 1_700_000_000_000_000_000, "cpu", 0.5)],
    };
    flush_parquet_only(&dir, &[batch]).expect("flush to seed");

    let mut cache = DedupCache::new(1000);
    cache.seed_from_recent_files(&dir).expect("seed");

    assert!(
        cache.contains(&crate::store::flush::log_content_id(&make_log(
            "svc",
            1_700_000_000_000_000_000,
            "info",
            "seed me",
        )))
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_filter_batch_dedup() {
    let mut cache = DedupCache::new(1000);

    let known_log = make_log("svc", 1_000_000, "info", "duplicate");
    let id = crate::store::flush::log_content_id(&known_log);
    cache.ids.insert(id.clone());
    assert!(cache.contains(&id));

    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs: vec![known_log, make_log("svc", 2_000_000, "debug", "unique")],
        spans: vec![],
        metrics: vec![],
    };

    let filtered = cache.filter_batch(&batch);
    assert_eq!(filtered.logs.len(), 1);
    assert_eq!(filtered.logs[0].level, "debug");
    assert_eq!(filtered.logs[0].timestamp_ns, 2_000_000);
}

#[test]
fn test_insert_batch_updates_cache() {
    let mut cache = DedupCache::new(1000);
    let log = make_log("svc", 1_000_000, "info", "new");
    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs: vec![log.clone()],
        spans: vec![],
        metrics: vec![],
    };
    cache.insert_batch(&batch);
    let id = crate::store::flush::log_content_id(&log);
    assert!(cache.contains(&id));
}

#[test]
fn test_seed_empty_dir() {
    let dir = test_dir("seed_empty");
    let mut cache = DedupCache::new(1000);
    cache.seed_from_recent_files(&dir).expect("seed from empty");
    assert_eq!(cache.len(), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_seed_respects_limit() {
    let dir = test_dir("seed_limit");

    let logs: Vec<LogEvent> = (0..10)
        .map(|i| {
            make_log(
                "svc",
                1_700_000_000_000_000_000 + i as i64,
                "info",
                &format!("m{i}"),
            )
        })
        .collect();
    let batch = IngestBatch {
        service_name: "svc".into(),
        instance_id: "i1".into(),
        batch_seq: 1,
        logs,
        spans: vec![],
        metrics: vec![],
    };
    flush_parquet_only(&dir, &[batch]).expect("flush");

    let mut cache = DedupCache::new(3);
    cache.seed_from_recent_files(&dir).expect("seed");
    assert_eq!(cache.len(), 3, "should load at most 3 IDs");

    let _ = fs::remove_dir_all(&dir);
}
