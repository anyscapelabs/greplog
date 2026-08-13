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

    #[tokio::test]
    async fn query_engine_sees_parquet_flushed_after_cold_start() {
        // This test verifies the fix for the cold-start bug where an empty
        // data_dir at registration time caused parquet_logs to be registered as
        // an empty MemTable instead of a ListingTable, making subsequent flushes
        // invisible to queries until restart.
        let dir = tempdir().expect("create temp dir");
        let config = EngineConfig {
            data_dir: dir.path().join("logs"),
            ..EngineConfig::default()
        };

        // Create QueryEngine against an EMPTY data_dir (no parquet files yet).
        let live_buffer = LiveBuffer::default();
        let engine = QueryEngine::new(&config, live_buffer)
            .await
            .expect("build query engine on empty data_dir");

        // Verify it starts empty.
        let batches = engine
            .execute_sql("SELECT count(*) FROM logs")
            .await
            .expect("run count query on empty engine");
        assert_eq!(count_value(&batches), 0, "engine must start with zero rows");

        // Now flush some data to disk — this should become immediately visible
        // because parquet_logs is a ListingTable that re-lists on each scan.
        let flusher = ParquetFlusher::new(&config);
        let mut table = MemTable::new(100);
        for index in 0..100 {
            table.append_record(&sample(index, "payment-worker"));
        }
        let batch = table.finish().expect("finish memtable");
        flusher.flush(&batch).expect("flush parquet");

        // Query again — the flushed rows must be visible without restarting.
        let batches = engine
            .execute_sql("SELECT count(*) FROM logs")
            .await
            .expect("run count query after flush");
        assert_eq!(
            count_value(&batches),
            100,
            "flushed parquet rows must be queryable immediately after cold start"
        );
    }

    #[tokio::test]
    async fn execute_sql_rejects_non_select_statements() {
        let dir = tempdir().expect("create temp dir");
        let config = EngineConfig {
            data_dir: dir.path().join("logs"),
            ..EngineConfig::default()
        };
        let live_buffer = LiveBuffer::default();
        let engine = QueryEngine::new(&config, live_buffer)
            .await
            .expect("build query engine");

        for sql in [
            "CREATE EXTERNAL TABLE hacked (a INT) STORED AS PARQUET LOCATION '/etc'",
            "COPY logs TO '/tmp/leak' STORED AS PARQUET",
            "INSERT INTO logs VALUES (1)",
            "DELETE FROM logs",
            "SET datafusion.execution.batch_size = 100",
            "SELECT 1; SELECT 2",
        ] {
            let err = engine
                .execute_sql(sql)
                .await
                .expect_err("non-read-only SQL must be rejected");
            assert!(
                matches!(err, greplog_engine::error::EngineError::QueryRejected(_)),
                "expected QueryRejected, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn execute_sql_accepts_select_and_with() {
        let dir = tempdir().expect("create temp dir");
        let config = EngineConfig {
            data_dir: dir.path().join("logs"),
            ..EngineConfig::default()
        };
        let live_buffer = LiveBuffer::default();
        let engine = QueryEngine::new(&config, live_buffer)
            .await
            .expect("build query engine");

        engine
            .execute_sql("SELECT count(*) FROM logs")
            .await
            .expect("plain SELECT must pass");

        engine
            .execute_sql(
                "WITH recent AS (SELECT * FROM logs) SELECT count(*) FROM recent",
            )
            .await
            .expect("WITH query must pass");
    }

    #[tokio::test]
    async fn execute_sql_caps_rows_at_max_query_rows() {
        let dir = tempdir().expect("create temp dir");
        let config = EngineConfig {
            data_dir: dir.path().join("logs"),
            max_query_rows: 5,
            ..EngineConfig::default()
        };
        let live_buffer = LiveBuffer::default();
        live_buffer
            .write()
            .expect("lock live buffer")
            .push(live_batch(10, "auth-api"));

        let engine = QueryEngine::new(&config, live_buffer)
            .await
            .expect("build query engine");
        let batches = engine
            .execute_sql("SELECT * FROM logs")
            .await
            .expect("run capped query");

        let total: usize = batches.iter().map(RecordBatch::num_rows).sum();
        assert_eq!(total, 5, "result must be capped at max_query_rows");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn execute_sql_times_out_a_runaway_query() {
        let dir = tempdir().expect("create temp dir");
        let config = EngineConfig {
            data_dir: dir.path().join("logs"),
            query_timeout_secs: 1,
            ..EngineConfig::default()
        };
        let live_buffer = LiveBuffer::default();
        // Seed enough rows that the 3-way cartesian product cannot finish in 1s.
        // A non-equijoin predicate defeats DataFusion's count(*) over cross-join
        // shortcut, and the row-cap limit cannot short-circuit an aggregate, so
        // the nested-loop join must actually visit ~8e12 triples.
        live_buffer
            .write()
            .expect("lock live buffer")
            .push(live_batch(20_000, "auth-api"));
        let engine = QueryEngine::new(&config, live_buffer)
            .await
            .expect("build query engine");

        let err = engine
            .execute_sql(
                "SELECT count(*) FROM logs a, logs b, logs c \
                 WHERE a.timestamp_us < b.timestamp_us AND b.timestamp_us < c.timestamp_us",
            )
            .await
            .expect_err("runaway query must time out");
        assert!(
            matches!(
                err,
                greplog_engine::error::EngineError::QueryTimeout
            ),
            "expected QueryTimeout, got: {err}"
        );
    }