//! Integration tests for the DataFusion-backed [`QueryEngine`].
//!
//! Verifies the read path end-to-end: Parquet chunks flushed by
//! [`ParquetFlusher`] are registered as a partitioned `logs` table and queried
//! with SQL, including a `service` filter that prunes by directory.

use arrow::array::{Int64Array, RecordBatch};

use greplog_engine::config::EngineConfig;
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

/// Flushes `5000 * 2` records (`auth-api` and `payment-worker`) to disk.
fn seed_partitioned_logs(config: &EngineConfig) {
    let flusher = ParquetFlusher::new(config);
    let mut table = MemTable::new(10_000);
    let half = 5_000usize;
    for index in 0..half {
        table.append_record(&sample(index, "auth-api"));
    }
    for index in half..(half * 2) {
        table.append_record(&sample(index, "payment-worker"));
    }
    let batch = table.finish().expect("finish memtable");
    flusher.flush(&batch).expect("flush parquet");
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
async fn total_rows_are_queryable() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_partitioned_logs(&config);

    let engine = QueryEngine::new(&config).await.expect("build query engine");
    let batches = engine
        .execute_sql("SELECT count(*) FROM logs")
        .await
        .expect("run count query");

    assert_eq!(count_value(&batches), 10_000, "all rows must be visible");
}

#[tokio::test]
async fn service_filter_prunes_partitioned_chunks() {
    let dir = tempdir().expect("create temp dir");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_partitioned_logs(&config);

    let engine = QueryEngine::new(&config).await.expect("build query engine");
    let batches = engine
        .execute_sql("SELECT count(*) FROM logs WHERE service = 'auth-api'")
        .await
        .expect("run filtered count query");

    assert_eq!(
        count_value(&batches),
        5_000,
        "only the auth-api partition must match"
    );
}