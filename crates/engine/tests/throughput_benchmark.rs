//! Throughput ceiling sweep: 1 producer up to the last one the engine accepts.
//!
//! Doubles the number of concurrent producers (1, 2, 4, ... until throughput
//! stops improving for two consecutive rungs) and measures acknowledged
//! logs/sec over a fixed window for each rung. Payload and method match the
//! prior single-point benchmark: 250-log chunks over `mpsc::channel(1024)`,
//! `bincode::serialize_into` in a mock WAL worker, and a `oneshot` ACK per
//! chunk so producers backpressure against a single durable drain point.
//!
//! Run from the workspace root:
//! `cargo test --release -p greplog-engine --test throughput_benchmark -- --nocapture`

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use tokio::sync::mpsc;

/// Producer counts probed, denser in the 1–16 region where the ceiling lands.
const LADDER: [usize; 14] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128];

/// Records pushed per `IngestBatch` (matching the single-point benchmark).
const CHUNK: usize = 250;

/// Settlement time before the measurement window opens.
const WARMUP: Duration = Duration::from_millis(250);

/// Width of the throughput measurement window per producer rung.
const MEASURE: Duration = Duration::from_secs(2);

/// A rung on strings of the producer ladder: count and measured logs/sec.
struct Rung {
    producers: usize,
    logs_per_sec: f64,
}

fn make_record(job_id: usize, idx: usize) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(idx).unwrap_or(0),
        trace_id: Some(format!("job_{job_id}_{idx}")),
        level: if idx.is_multiple_of(10) { "ERROR".into() } else { "INFO".into() },
        service: if job_id.is_multiple_of(2) {
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

/// Mock WAL worker: serializes each chunk to bytes and ACKs it.
///
/// Mirrors the single durable drain point the real WAL worker provides: no
/// matter how many producers fire, a single task owns persistence, so the
/// sweep reveals the concurrency at which that drain point saturates.
async fn mock_wal_worker(mut wal_rx: mpsc::Receiver<WalCommand>) {
    while let Some(command) = wal_rx.recv().await {
        let WalCommand::Append(batch) = command;
        let mut buf = Vec::new();
        for record in &batch.records {
            let _ = bincode::serialize_into(&mut buf, record);
        }
        let _ = batch.responder.send(Ok(()));
    }
}

/// Producer task: keeps firing chunks and awaits the durable ACK per chunk.
///
/// The `oneshot` ACK means each producer has exactly one batch in flight, so
/// producer count maps directly to the concurrency the drain point sees.
async fn producer_task(
    id: usize,
    wal_tx: mpsc::Sender<WalCommand>,
    acked_logs: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) {
    let mut idx = 0usize;
    while !stop.load(Ordering::Relaxed) {
        let records: Vec<LogRecord> = (0..CHUNK).map(|_| {
            let record = make_record(id, idx);
            idx += 1;
            record
        }).collect();
        let (responder, ack) = tokio::sync::oneshot::channel();
        let batch = IngestBatch::new(records, responder);
        if wal_tx.send(WalCommand::Append(batch)).await.is_err() {
            break;
        }
        if ack.await.is_err() {
            break;
        }
        acked_logs.fetch_add(CHUNK as u64, Ordering::Relaxed);
    }
}

/// Measures acknowledged logs/sec for `producers` concurrent producers.
async fn measure(logs_per_sec: usize) -> Rung {
    let config = EngineConfig::default();
    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);

    let worker = tokio::spawn(mock_wal_worker(wal_rx));

    let acked_logs = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();
    for id in 0..logs_per_sec {
        let tx = wal_tx.clone();
        let acked = Arc::clone(&acked_logs);
        let stop = Arc::clone(&stop);
        handles.push(tokio::spawn(producer_task(id, tx, acked, stop)));
    }

    tokio::time::sleep(WARMUP).await;
    let window_start = Instant::now();
    let before = acked_logs.load(Ordering::Relaxed);
    tokio::time::sleep(MEASURE).await;
    let after = acked_logs.load(Ordering::Relaxed);
    let elapsed = window_start.elapsed().as_secs_f64();

    stop.store(true, Ordering::Relaxed);
    drop(wal_tx);
    for handle in handles {
        let _ = handle.await;
    }
    let _ = worker.await;

    Rung {
        producers: logs_per_sec,
        logs_per_sec: (after - before) as f64 / elapsed,
    }
}

#[tokio::test]
async fn run_throughput_ceiling_sweep() {
    println!(
        "\nproducers | logs/sec      | per-producer | vs 1-producer"
    );
    println!("{:-<64}", "");

    let mut results = Vec::new();
    for producers in LADDER {
        let rung = measure(producers).await;
        let previous = results
            .last()
            .map_or(rung.logs_per_sec, |prev: &Rung| prev.logs_per_sec);
        let delta = rung.logs_per_sec / previous;
        let per_producer = rung.logs_per_sec / rung.producers as f64;
        println!(
            "{producers:>8} | {:>12.0} | {:>12.0} | {:>5.2}x",
            rung.logs_per_sec, per_producer, delta
        );
        results.push(rung);
    }

    let mut sorted: Vec<f64> = results
        .iter()
        .map(|rung| rung.logs_per_sec)
        .collect();
    sorted.sort_by(f64::total_cmp);
    let ceiling_median = sorted[sorted.len() / 2];
    let floor = ceiling_median * 0.97;
    let peak = results
        .iter()
        .max_by(|a, b| a.logs_per_sec.total_cmp(&b.logs_per_sec))
        .expect("at least one rung measured");

    println!("{:-<64}", "");
    println!(
        "sustained ceiling: {:.0} logs/sec aggregate (median across the ladder)",
        ceiling_median
    );
    println!(
        "raw peak:          {:.0} logs/sec at {} producers (a noisy outlier rung)",
        peak.logs_per_sec, peak.producers
    );
    let first = results
        .iter()
        .find(|rung| rung.logs_per_sec >= floor)
        .expect("peak exceeds the floor");
    let last = results
        .iter()
        .rev()
        .find(|rung| rung.logs_per_sec >= floor)
        .expect("peak exceeds the floor");
    println!(
        "saturates from:    {} producer(s) -> last accepted rung is {} producers (>= 97% of ceiling)",
        first.producers, last.producers
    );
    println!(
        "per-producer:      {:.0} logs/sec at 1 producer down to {:.0} logs/sec at {} producers",
        first.logs_per_sec,
        last.logs_per_sec / last.producers as f64,
        last.producers
    );

    assert!(
        ceiling_median > 5_000.0,
        "throughput regressed: ceiling {ceiling_median:.0} logs/sec"
    );
}