//! Integration tests for segment-scoped WAL reclamation (rotation).
//!
//! Regression coverage for the data-loss race where a truncation for flush N
//! is processed *after* a later append N+1 has already landed on disk. The fix
//! rotates the active segment instead of emptying it, so acknowledged bytes
//! always survive a misordered confirmation.

use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;
use tokio::sync::{mpsc, oneshot};

use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::record::LogRecord;
use greplog_engine::wal::{record_count, replay_all, sealed_segments};
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

/// Spawns a WAL worker over a fresh temp directory and returns the pieces it
/// needs: the append sender, the confirmation sender, and the active WAL path.
fn harness(
    dir: &tempfile::TempDir,
) -> (
    mpsc::Sender<WalCommand>,
    mpsc::Sender<usize>,
    std::path::PathBuf,
    std::thread::JoinHandle<()>,
) {
    let wal_path = dir.path().join("current.wal");
    let config = Arc::new(EngineConfig {
        wal_path: wal_path.clone(),
        mpsc_buffer_size: 64,
        crossbeam_buffer_size: 64,
        ..EngineConfig::default()
    });
    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);
    let (handoff_tx, _handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);
    let handle = spawn_wal_worker(config, wal_rx, truncate_rx, handoff_tx);
    (wal_tx, truncate_tx, wal_path, handle)
}

async fn append_and_wait(
    sender: &mpsc::Sender<WalCommand>,
    records: Vec<LogRecord>,
) -> Result<(), String> {
    let (responder, ack) = oneshot::channel();
    sender
        .send(WalCommand::Append(IngestBatch::new(records, responder)))
        .await
        .map_err(|error| format!("failed to enqueue append: {error}"))?;
    tokio::time::timeout(Duration::from_secs(10), ack)
        .await
        .map_err(|_| "timed out waiting for WAL acknowledgement".to_string())?
        .map_err(|_| "worker dropped responder".to_string())?
        .map_err(|error| error.to_string())
}

fn recovered_traces(wal_path: &std::path::Path) -> Vec<String> {
    replay_all(wal_path)
        .expect("replay all segments")
        .into_iter()
        .map(|record| record.trace_id.clone().unwrap_or_default())
        .collect()
}

/// The precise race from the bug report: a confirmation for flush N is
/// processed *after* a later append N+1 has already been fsynced and
/// acknowledged. The acknowledged record must survive.
#[tokio::test]
async fn truncate_processed_after_later_append_keeps_acknowledged_record() {
    let dir = tempdir().expect("create temp dir");
    let (wal_tx, truncate_tx, wal_path, handle) = harness(&dir);

    append_and_wait(&wal_tx, vec![sample("a-1", "INFO"), sample("a-2", "WARN")])
        .await
        .expect("flush N batch must append");
    append_and_wait(&wal_tx, vec![sample("n-plus-1", "ERROR")])
        .await
        .expect("N+1 batch must append and be acknowledged");

    // Now the confirmation for flush N lands — after N+1 is already on disk.
    truncate_tx
        .send(2)
        .await
        .expect("confirmation channel open");
    // Give the worker time to process the confirmation (and rotate).
    tokio::time::sleep(Duration::from_millis(400)).await;

    let traces = recovered_traces(&wal_path);
    assert!(
        traces.iter().any(|trace| trace == "n-plus-1"),
        "acknowledged append N+1 must survive a misordered confirmation; got {traces:?}"
    );
    assert!(
        traces.iter().any(|trace| trace == "a-1"),
        "the pre-confirmation records stay covered (never deleted) while unconfirmed; got {traces:?}"
    );
    // N+1 lives either in a still-covered sealed segment or in the fresh
    // active file — both are durable on disk.
    assert!(!traces.is_empty());

    drop(wal_tx);
    drop(truncate_tx);
    handle.join().expect("wal worker must exit cleanly");
}

/// Once a confirmation covers a sealed segment entirely, it is deleted so the
/// WAL directory is reclaimed.
#[tokio::test]
async fn fully_confirmed_segments_are_reclaimed() {
    let dir = tempdir().expect("create temp dir");
    let (wal_tx, truncate_tx, wal_path, handle) = harness(&dir);

    append_and_wait(&wal_tx, vec![sample("a", "INFO")])
        .await
        .expect("append a");
    append_and_wait(&wal_tx, vec![sample("b", "ERROR")])
        .await
        .expect("append b");
    truncate_tx.send(2).await.expect("confirm both records");

    tokio::time::sleep(Duration::from_millis(400)).await;

    let segments = sealed_segments(&wal_path).expect("list sealed segments");
    assert!(
        segments.is_empty(),
        "fully confirmed segments must be reclaimed"
    );
    assert_eq!(
        record_count(&wal_path).expect("count active"),
        0,
        "the active segment starts empty after a full rotation"
    );

    drop(wal_tx);
    drop(truncate_tx);
    handle.join().expect("wal worker must exit cleanly");
}

/// A storm of interleaved appends and lagging confirmations must never lose an
/// acknowledged record. Each confirmation is for a *prior* round's record, but
/// is delivered after the next round's append has already landed on disk — the
/// exact out-of-order pattern that used to wipe the file. The final, never-
/// confirmed record must survive in a sealed segment.
#[tokio::test]
async fn interleaved_storm_never_loses_unconfirmed_records() {
    let dir = tempdir().expect("create temp dir");
    let (wal_tx, truncate_tx, wal_path, handle) = harness(&dir);

    let rounds = 40u32;
    for round in 0..rounds {
        let trace = format!("storm-{round:03}");
        append_and_wait(&wal_tx, vec![sample(&trace, "INFO")])
            .await
            .expect("append must be acknowledged");
        // Confirm the *previous* round's record — deliberately not the one just
        // appended, so the newest record always races ahead of its confirmation.
        if round > 0 {
            truncate_tx.send(1).await.expect("confirm prior record");
        }
    }

    drop(wal_tx);
    drop(truncate_tx);
    handle.join().expect("wal worker must exit cleanly");

    let traces = recovered_traces(&wal_path);
    assert!(
        traces.iter().any(|trace| trace == "storm-039"),
        "the last acknowledged-but-unconfirmed record must survive; got {traces:?}"
    );
    assert!(
        !traces.iter().any(|trace| trace == "storm-000"),
        "fully confirmed records are reclaimed, not retained forever; got {traces:?}"
    );
}
