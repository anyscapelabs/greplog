//! Integration tests for the Parquet storage layer and WAL truncation.
//!
//! Verifies the full write path end-to-end: appended records become a valid
//! Hive-partitioned Parquet chunk and, once the chunk is durable, `current.wal`
//! is truncated back to zero bytes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::tempdir;
use tokio::sync::{mpsc, oneshot};

use greplog_engine::config::EngineConfig;
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::memtable::MemTable;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::worker::{spawn_memtable_worker, spawn_wal_worker};

fn sample(index: usize) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
        trace_id: Some(format!("trace-{index}")),
        level: "ERROR".to_string(),
        service: "payment-worker".to_string(),
        message: format!("order {index} failed"),
        raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
    }
}

fn first_parquet(root: &Path) -> Option<PathBuf> {
    let year = fs::read_dir(root).ok()?.next()?.ok()?.path();
    let month = fs::read_dir(&year).ok()?.next()?.ok()?.path();
    let day = fs::read_dir(&month).ok()?.next()?.ok()?.path();
    for entry in fs::read_dir(&day).ok()? {
        let entry = entry.ok()?;
        if entry.path().extension().is_some_and(|ext| ext == "parquet") {
            return Some(entry.path());
        }
    }
    None
}

fn row_count(path: &Path) -> usize {
    let file = fs::File::open(path).expect("open parquet");
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("parse parquet");
    let reader = builder.build().expect("build record reader");
    reader
        .map(|batch| batch.expect("read batch").num_rows())
        .sum()
}

#[test]
fn flusher_writes_valid_partitioned_parquet() {
    let dir = tempdir().expect("create temp dir");
    let root = dir.path().join("logs");
    let config = EngineConfig {
        data_dir: root.clone(),
        ..EngineConfig::default()
    };
    let flusher = ParquetFlusher::new(&config);

    let mut table = MemTable::new(8);
    for index in 0..5 {
        table.append_record(&sample(index));
    }
    let batch = table.finish().expect("finish memtable");

    flusher.flush(&batch).expect("flush parquet");

    let chunk = first_parquet(&root).expect("a chunk must exist under year/month/day");
    assert_eq!(row_count(&chunk), 5, "parquet must round-trip every row");
}

#[test]
fn pipeline_persists_parquet_and_truncates_wal() {
    let dir = tempdir().expect("create temp dir");
    let logs_root = dir.path().join("logs");
    let wal_path = dir.path().join("current.wal");
    let config = Arc::new(EngineConfig {
        data_dir: logs_root.clone(),
        wal_path: wal_path.clone(),
        mpsc_buffer_size: 128,
        crossbeam_buffer_size: 128,
        ..EngineConfig::default()
    });

    let (wal_tx, wal_rx) = mpsc::channel::<WalCommand>(config.mpsc_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<()>(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);

    let flusher = ParquetFlusher::new(&config);
    let wal_handle = spawn_wal_worker(config.clone(), wal_rx, truncate_rx, handoff_tx);
    let memtable_handle = spawn_memtable_worker(handoff_rx, truncate_tx, flusher, config);

    let runtime = tokio::runtime::Runtime::new().expect("build runtime");
    runtime.block_on(async {
        let per_batch = 1_000;
        let limit = EngineConfig::default().flush_row_limit;
        for _ in 0..(limit / per_batch) {
            let records: Vec<LogRecord> = (0..per_batch).map(sample).collect();
            let (responder, ack) = oneshot::channel();
            wal_tx
                .send(WalCommand::Append(IngestBatch::new(records, responder)))
                .await
                .expect("enqueue append command");

            let outcome = tokio::time::timeout(Duration::from_secs(10), ack)
                .await
                .expect("ack must arrive within timeout")
                .expect("worker must respond");
            assert!(outcome.is_ok(), "every batch must be fsynced before ack");
        }
    });

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let has_parquet = first_parquet(&logs_root).is_some();
        let wal_empty = fs::metadata(&wal_path).map_or(true, |meta| meta.len() == 0);
        if has_parquet && wal_empty {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "pipeline did not flush Parquet and truncate the WAL in time"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    drop(wal_tx);
    wal_handle.join().expect("wal worker must exit cleanly");
    memtable_handle.join().expect("memtable worker must exit cleanly");

    let chunk = first_parquet(&logs_root).expect("parquet chunk must exist");
    let limit = EngineConfig::default().flush_row_limit;
    assert_eq!(
        row_count(&chunk),
        limit,
        "all appended records must land in Parquet"
    );
    let wal_len = fs::metadata(&wal_path).expect("wal file must exist").len();
    assert_eq!(wal_len, 0, "wal must be truncated to zero bytes after the flush");
}