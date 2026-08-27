//! End-to-end benchmark over the real write and query paths.
//!
//! Unlike `throughput_benchmark.rs` (a mock WAL used to find the ceiling of
//! the producer pipeline), this exercises what production runs: batches go
//! through the actual WAL worker with fsync-before-ack, land in Parquet via
//! the real MemTable worker, and are then queried through the DataFusion
//! engine exactly like the dashboard does. The numbers printed here are the
//! ones published as official benchmarks.
//!
//! Run from the workspace root:
//! `cargo test --release -p greplog-engine --test e2e_benchmark -- --nocapture`
//!
//! Every `RESULT ...` line is parsed by `.github/workflows/benchmarks.yml`.

use std::sync::Arc;
use std::time::Instant;

use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::worker::{spawn_memtable_worker_with_broadcast, spawn_wal_worker};
use tokio::sync::{broadcast, mpsc, oneshot};

/// Total records pushed through the durable path per run.
const RECORDS: usize = 100_000;

/// Concurrent producers, matching a small fleet of services ingesting at once.
const PRODUCERS: usize = 8;

/// Records per acknowledged batch, matching the SDK default payload size.
const BATCH: usize = 250;

/// Repetitions per query when measuring latency percentiles.
const QUERY_RUNS: usize = 25;

/// Base timestamp for generated records: 2023-11-14T22:13:20Z, spread one
/// millisecond apart so day partitions and time filters behave realistically.
const BASE_TS_US: i64 = 1_700_000_000_000_000;

fn make_record(seq: usize) -> LogRecord {
    let seq_i64 = i64::try_from(seq).unwrap_or(0);
    LogRecord {
        timestamp_us: BASE_TS_US + seq_i64 * 1_000,
        trace_id: Some(format!("job_{seq}")),
        span_id: None,
        parent_span_id: None,
        duration_ms: None,
        level: if seq % 10 == 0 { "ERROR" } else { "INFO" }.into(),
        service: if seq % 2 == 0 {
            "payment-worker"
        } else {
            "auth-api"
        }
        .into(),
        message: format!("request {seq} processed"),
        raw_body: Some(format!(
            r#"{{"stack":"at payment.rs:{seq}","params":{{"seq":{seq}}}}}"#
        )),
    }
}

#[tokio::test]
async fn end_to_end_ingest_and_query_benchmark() {
    let dir = std::env::temp_dir().join(format!("greplog-e2e-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut config = EngineConfig::default();
    config.data_dir = dir.join("logs");
    config.wal_path = dir.join("wal/current.wal");
    std::fs::create_dir_all(&config.data_dir).expect("create data dir");
    if let Some(parent) = config.wal_path.parent() {
        std::fs::create_dir_all(parent).expect("create wal dir");
    }
    let config = Arc::new(config);
    let live_buffer: LiveBuffer = LiveBuffer::default();

    // The exact wiring `greplog start` uses: WAL worker fsyncs and acks, the
    // MemTable worker hands rows into Parquet chunks.
    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) = crossbeam::channel::bounded::<Vec<LogRecord>>(
        config.crossbeam_buffer_size,
    );
    let (broadcast_tx, _) =
        broadcast::channel::<Vec<LogRecord>>(1_024);

    let wal_handle = spawn_wal_worker(Arc::clone(&config), wal_rx, truncate_rx, handoff_tx);
    let memtable_handle = spawn_memtable_worker_with_broadcast(
        handoff_rx,
        truncate_tx,
        ParquetFlusher::new(&config),
        Arc::clone(&live_buffer),
        Arc::clone(&config),
        Some(broadcast_tx),
    );

    // --- Ingestion: every batch is ACK'd after fsync, producers backpressure
    // against that ack exactly like the HTTP layer does.
    let start = Instant::now();
    let per_producer = RECORDS / PRODUCERS;
    let mut producers = Vec::new();
    for producer in 0..PRODUCERS {
        let wal_tx = wal_tx.clone();
        producers.push(tokio::spawn(async move {
            for offset in (0..per_producer).step_by(BATCH) {
                let records: Vec<LogRecord> = (offset..offset + BATCH)
                    .map(|row| make_record(producer * per_producer + row))
                    .collect();
                let (responder, rx) = oneshot::channel();
                wal_tx
                    .send(WalCommand::Append(IngestBatch::new(records, responder)))
                    .await
                    .expect("wal worker alive");
                rx.await.expect("ack delivered").expect("batch accepted");
            }
        }));
    }
    drop(wal_tx);
    for producer in producers {
        producer.await.expect("producer task");
    }
    let ingest_elapsed = start.elapsed();

    // Drain: dropping the last wal_tx above closes the WAL worker, which
    // closes the handoff channel and lets the MemTable worker flush its
    // remainder to Parquet and exit.
    let drain_start = Instant::now();
    let _ = tokio::task::spawn_blocking(move || {
        let _ = wal_handle.join();
        let _ = memtable_handle.join();
    })
    .await;
    let drain_elapsed = drain_start.elapsed();

    let logs_per_sec = f64::from(u32::try_from(RECORDS).unwrap()) / ingest_elapsed.as_secs_f64();
    println!(
        "RESULT kind=ingest records={RECORDS} producers={PRODUCERS} batch={BATCH} \
         logs_per_sec={logs_per_sec:.0} ingest_ms={} drain_ms={}",
        ingest_elapsed.as_millis(),
        drain_elapsed.as_millis()
    );

    // --- Query latency: fresh engine over the flushed Parquet + empty live
    // buffer, mirroring a restarted server answering dashboard queries.
    let engine = QueryEngine::new(&config, live_buffer)
        .await
        .expect("query engine");
    let cutoff_us = BASE_TS_US + (RECORDS as i64) * 1_000 / 2;
    // Same literal form crates/engine/src/search.rs emits for time filters:
    // timestamp_us is a Timestamp(microseconds) column, not plain integers.
    let cutoff = chrono::DateTime::from_timestamp(
        cutoff_us.div_euclid(1_000_000),
        u32::try_from(cutoff_us.rem_euclid(1_000_000))
            .unwrap_or(0)
            .saturating_mul(1_000),
    )
    .expect("cutoff timestamp");
    let queries: [(&str, String); 4] = [
        ("count_all", "SELECT count(*) FROM logs".to_string()),
        (
            "count_errors",
            "SELECT count(*) FROM logs WHERE level = 'ERROR'".to_string(),
        ),
        (
            "group_by_service",
            "SELECT service, count(*) FROM logs GROUP BY service ORDER BY count(*) DESC"
                .to_string(),
        ),
        (
            "time_window",
            format!(
                "SELECT count(*) FROM logs WHERE timestamp_us >= TIMESTAMP '{}'",
                cutoff.format("%Y-%m-%dT%H:%M:%S%.6fZ")
            ),
        ),
    ];

    // One warm-up pass so first-query planning costs don't skew percentiles.
    for (_, sql) in &queries {
        engine.execute_sql(sql).await.expect("warm-up query");
    }

    for (name, sql) in &queries {
        let mut samples_ms: Vec<f64> = Vec::with_capacity(QUERY_RUNS);
        for _ in 0..QUERY_RUNS {
            let began = Instant::now();
            let batches = engine.execute_sql(sql).await.expect("benchmark query");
            let _ = batches;
            samples_ms.push(began.elapsed().as_secs_f64() * 1_000.0);
        }
        samples_ms.sort_by(f64::total_cmp);
        let p50 = samples_ms[samples_ms.len() / 2];
        let p95 = samples_ms[(samples_ms.len() as f64 * 0.95) as usize % samples_ms.len()];
        println!("RESULT kind=query name={name} runs={QUERY_RUNS} p50_ms={p50:.2} p95_ms={p95:.2}");
    }

    // Sanity: the durable pipeline must not have lost anything it acked.
    let stored = engine
        .execute_sql("SELECT count(*) FROM logs")
        .await
        .expect("count");
    let values = stored[0]
        .column(0)
        .as_any()
        .downcast_ref::<arrow::array::Int64Array>()
        .expect("count(*) is an Int64 column");
    assert_eq!(
        values.value(0),
        i64::try_from(RECORDS).unwrap_or(-1),
        "every acknowledged record must be queryable after restart"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
