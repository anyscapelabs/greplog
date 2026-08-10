//! Integration tests for the DataFusion-backed [`QueryEngine`] unified view.
//!
//! Verifies the dual-tier read path: Parquet chunks flushed to disk by
//! [`ParquetFlusher`] are fused with in-memory batches staged in a
//! [`LiveBuffer`] under the single `logs` view, and filters span both tiers.

use arrow::array::{Int64Array, RecordBatch};

use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use tempfile::tempdir;

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

/// Flushes `half * 2` records, `half` per service, to disk as partitioned Parquet.
fn seed_disk_logs(config: &EngineConfig, half: usize) {
    let flusher = ParquetFlusher::new(config);
    let mut table = MemTable::new(half * 2);
    for index in 0..half {
        table.append_record(&sample(index, "auth-api"));
    }
    for index in half..(half * 2) {
        table.append_record(&sample(index, "payment-worker"));
    }
    let batch = table.finish().expect("finish memtable");
    flusher.flush(&batch).expect("flush parquet");
}

/// Builds a single in-memory batch of `count` records, all from one service.
fn live_batch(count: usize, service: &str) -> RecordBatch {
    let mut table = MemTable::new(count);
    for index in 0..count {
        table.append_record(&sample(index, service));
    }
    table.finish().expect("finish live memtable")
}

/// Extracts the single scalar from `SELECT count(*) ...` result batches.
fn count_value(batches: &[RecordBatch]) -> i64 {
    let batch = batches.first().expect("count query produces a batch");
    let column = batch.column(0);
    let values = column
        .as_any()
        .downcast_ref::<Int64Array>()
        .expect("count(*) is an Int64 column");
    values.value(0)
}

#[tokio::test]
async fn unified_view_counts_live_and_parquet_rows() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk_logs(&config, 2_500);

    let live_buffer = LiveBuffer::default();
    live_buffer
        .write()
        .expect("lock live buffer")
        .push(live_batch(2_000, "auth-api"));

    let engine = QueryEngine::new(&config, live_buffer)
        .await
        .expect("build query engine");
    let batches = engine
        .execute_sql("SELECT count(*) FROM logs")
        .await
        .expect("run count query");

    assert_eq!(
        count_value(&batches),
        7_000,
        "5,000 parquet rows plus 2,000 live rows must be visible together"
    );
}

#[tokio::test]
async fn unified_view_filters_across_both_tiers() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk_logs(&config, 2_500);

    let live_buffer = LiveBuffer::default();
    live_buffer
        .write()
        .expect("lock live buffer")
        .push(live_batch(2_000, "auth-api"));

    let engine = QueryEngine::new(&config, live_buffer)
        .await
        .expect("build query engine");
    let batches = engine
        .execute_sql("SELECT count(*) FROM logs WHERE service = 'auth-api'")
        .await
        .expect("run filtered count query");

    assert_eq!(
        count_value(&batches),
        4_500,
        "2,500 parquet plus 2,000 live auth-api rows must match"
    );
}