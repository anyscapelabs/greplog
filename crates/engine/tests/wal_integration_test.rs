//! Integration tests for the Write-Ahead Log round-trip.
//!
//! Verifies the end-to-end contract: a batch sent through the ingest channel is
//! acknowledged only after the WAL file has bytes on disk.

use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;
use tokio::sync::{mpsc, oneshot};

use greplog_engine::config::EngineConfig;
use greplog_engine::error::EngineError;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use greplog_engine::worker::spawn_wal_worker;

fn sample(trace: &str, level: &str) -> LogRecord {
    LogRecord {
        timestamp_us: 1_723_000_000_123_456,
        trace_id: Some(trace.to_string()),
        level: level.to_string(),
        service: "payment-worker".to_string(),
        message: format!("order {trace} processed"),
        raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
    }
}

async fn send_and_await(
    sender: &mpsc::Sender<WalCommand>,
    records: Vec<LogRecord>,
) -> Result<Result<(), EngineError>, String> {
    let (responder, ack) = oneshot::channel();
    sender
        .send(WalCommand::Append(IngestBatch::new(records, responder)))
        .await
        .map_err(|error| format!("failed to enqueue batch: {error}"))?;
    tokio::time::timeout(Duration::from_secs(10), ack)
        .await
        .map_err(|_| "timed out waiting for WAL acknowledgement".to_string())?
        .map_err(|_| "WAL worker dropped its responder without an answer".to_string())
}

#[tokio::test]
async fn wal_worker_persists_batch_and_acknowledges() {
    let dir = tempdir().expect("create temp dir for wal");
    let wal_path = dir.path().join("current.wal");
    let config = Arc::new(EngineConfig {
        wal_path: wal_path.clone(),
        mpsc_buffer_size: 8,
        crossbeam_buffer_size: 8,
        ..EngineConfig::default()
    });

    let (sender, receiver) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, _handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);
    let handle = spawn_wal_worker(config, receiver, truncate_rx, handoff_tx);

    let records = vec![
        sample("a", "INFO"),
        sample("b", "WARN"),
        sample("c", "ERROR"),
    ];
    let outcome = send_and_await(&sender, records)
        .await
        .expect("channel must accept the batch");
    assert!(outcome.is_ok(), "wal append must succeed");

    let metadata = std::fs::metadata(&wal_path).expect("wal file must exist on disk");
    assert!(metadata.len() > 0, "wal file must have a non-zero size");

    drop(sender);
    drop(truncate_tx);
    handle.join().expect("wal worker thread must exit cleanly");
}

#[tokio::test]
async fn wal_worker_orders_batches_and_grows_file() {
    let dir = tempdir().expect("create temp dir for wal");
    let wal_path = dir.path().join("current.wal");
    let config = Arc::new(EngineConfig {
        wal_path: wal_path.clone(),
        mpsc_buffer_size: 8,
        crossbeam_buffer_size: 8,
        ..EngineConfig::default()
    });

    let (sender, receiver) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, _handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);
    let handle = spawn_wal_worker(config, receiver, truncate_rx, handoff_tx);

    let outcome = send_and_await(&sender, vec![sample("first", "INFO")])
        .await
        .expect("first batch must be accepted");
    assert!(outcome.is_ok(), "first batch must append cleanly");
    let size_after_first = std::fs::metadata(&wal_path)
        .expect("wal file must exist")
        .len();
    assert!(size_after_first > 0);

    let outcome = send_and_await(&sender, vec![sample("second", "ERROR")])
        .await
        .expect("second batch must be accepted");
    assert!(outcome.is_ok(), "second batch must append cleanly");
    let size_after_second = std::fs::metadata(&wal_path)
        .expect("wal file must exist")
        .len();
    assert!(
        size_after_second > size_after_first,
        "wal file must grow with each batch"
    );

    drop(sender);
    drop(truncate_tx);
    handle.join().expect("wal worker thread must exit cleanly");
}

#[tokio::test]
async fn wal_worker_reports_failure_when_wal_cannot_be_opened() {
    let dir = tempdir().expect("create temp dir for wal");
    let wal_path = dir.path().join("missing-dir").join("current.wal");
    let config = Arc::new(EngineConfig {
        wal_path,
        mpsc_buffer_size: 8,
        crossbeam_buffer_size: 8,
        ..EngineConfig::default()
    });

    let (sender, receiver) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, _handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);
    let handle = spawn_wal_worker(config, receiver, truncate_rx, handoff_tx);

    let outcome = send_and_await(&sender, vec![sample("a", "INFO")])
        .await
        .expect("channel must accept the batch");
    assert!(
        outcome.is_err(),
        "an unopenable wal must surface as an error"
    );

    drop(sender);
    drop(truncate_tx);
    handle.join().expect("wal worker thread must exit cleanly");
}

#[test]
fn wal_replay_recovers_after_writer_dropped() {
    use greplog_engine::wal::WalWriter;

    let dir = tempdir().expect("create temp dir for wal");
    let wal_path = dir.path().join("current.wal");

    {
        let mut wal = WalWriter::open(&wal_path).expect("open wal");
        wal.append_batch(&[sample("first", "INFO"), sample("second", "WARN")])
            .expect("first append");
        wal.append_batch(&[sample("third", "ERROR")])
            .expect("second append");
        // wal dropped here
    }

    let recovered = WalWriter::replay_wal(&wal_path).expect("replay wal");
    assert_eq!(recovered.len(), 3, "all records must be recovered");
    assert_eq!(recovered[0].trace_id.as_deref(), Some("first"));
    assert_eq!(recovered[1].trace_id.as_deref(), Some("second"));
    assert_eq!(recovered[2].trace_id.as_deref(), Some("third"));
}

#[test]
fn wal_replay_returns_empty_when_file_missing() {
    use greplog_engine::wal::WalWriter;

    let dir = tempdir().expect("create temp dir for wal");
    let wal_path = dir.path().join("nonexistent.wal");
    let recovered = WalWriter::replay_wal(&wal_path).expect("replay missing wal");
    assert!(recovered.is_empty(), "missing wal should return empty vec");
}
