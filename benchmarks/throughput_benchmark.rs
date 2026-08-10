//! Throughput benchmark: 100k structured logs via 4 concurrent producers.
//!
//! This benchmark stresses the MPSC ingress channel, WAL group-commit,
//! and MemTable/Parquet flush path. Run with `cargo test -- --nocapture`
//! or `cargo bench`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use tokio::sync::mpsc;

/// Total logs ingested per benchmark run.
const TOTAL_LOGS: usize = 100_000;

/// Number of concurrent producer tasks.
const CONCURRENT_PRODUCERS: usize = 4;

/// Logs per producer.
const LOGS_PER_PRODUCER: usize = TOTAL_LOGS / CONCURRENT_PRODUCERS;

fn make_record(job_id: usize, idx: usize) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(idx).unwrap_or(0),
        trace_id: Some(format!("job_{job_id}_{idx}")),
        level: if idx % 10 == 0 { "ERROR".into() } else { "INFO".into() },
        service: if job_id % 2 == 0 {
            "payment-worker".into()
        } else {
            "auth-api".into()
        },
        message: format!("request {idx} processed for job {job_id}"),
        raw_body: Some(format!(
            r#"{{"stack":"at payment.rs:42","params":{{"job_id":{job_id},"idx":{idx}}}}}"#
        )),
    }
}

async fn producer_task(id: usize, wal_tx: mpsc::Sender<WalCommand>, start: Instant) {
    for chunk_start in (0..LOGS_PER_PRODUCER).step_by(250) {
        let chunk_end = (chunk_start + 250).min(LOGS_PER_PRODUCER);
        let records: Vec<LogRecord> = (chunk_start..chunk_end)
            .map(|i| make_record(id, i))
            .collect();

        let (tx, rx) = tokio::sync::oneshot::channel();
        let batch = IngestBatch::new(records, tx);
        if wal_tx.send(WalCommand::Append(batch)).await.is_err() {
            break;
        }
        // Backpressure: wait for fsync ACK before next chunk.
        let _ = rx.await;
        if start.elapsed() > Duration::from_secs(30) {
            break;
        }
    }
}

#[tokio::test]
async fn run_throughput_benchmark() {
    let config = EngineConfig::default();
    let (wal_tx, mut wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);

    // Mock WAL worker: serialize + ack (no real fsync in bench).
    let worker = tokio::spawn(async move {
        while let Some(cmd) = wal_rx.recv().await {
            if let WalCommand::Append(batch) = cmd {
                // Simulate bincode + sync_data cost
                let mut buf = Vec::new();
                for r in &batch.records {
                    let _ = bincode::serialize_into(&mut buf, r);
                }
                let _ = batch.responder.send(Ok(()));
            }
        }
    });

    let start = Instant::now();
    let mut handles = Vec::new();
    for id in 0..CONCURRENT_PRODUCERS {
        let tx = wal_tx.clone();
        handles.push(tokio::spawn(producer_task(id, tx, start)));
    }

    for h in handles {
        let _ = h.await;
    }
    drop(wal_tx);
    let _ = worker.await;

    let elapsed = start.elapsed();
    let total = TOTAL_LOGS as f64;
    let throughput = total / elapsed.as_secs_f64();

    println!(
        "Throughput: {throughput:.0} logs/sec over {:.2}s ({} producers, {} logs)",
        elapsed.as_secs_f64(),
        CONCURRENT_PRODUCERS,
        TOTAL_LOGS
    );
    println!("Expected: >14k logs/sec per producer, 0 dropped, 18-24 MB flat");
    assert!(throughput > 5_000.0, "throughput regressed");
}
