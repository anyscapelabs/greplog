//! Integration tests for the MemTable and the WAL → MemTable handoff.
//!
//! Verifies the full write path: ingest channel → WAL worker (`fsync`) →
//! crossbeam handoff → MemTable worker (Arrow columnar conversion + flush).

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::tempdir;
use tokio::sync::{mpsc, oneshot};

use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::ingest::{IngestBatch, WalCommand};
use greplog_engine::memtable::MemTable;
use greplog_engine::record::LogRecord;
use greplog_engine::storage::ParquetFlusher;
use greplog_engine::worker::{spawn_memtable_worker, spawn_wal_worker};

fn sample(index: usize, level: &str) -> LogRecord {
    LogRecord {
        timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
        trace_id: Some(format!("trace-{index}")),
        span_id: None,
        parent_span_id: None,
        duration_ms: None,
        level: level.to_string(),
        service: "payment-worker".to_string(),
        message: format!("order {index} processed"),
        raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
    }
}

/// A minimal test subscriber sink that accumulates formatted output.
#[derive(Clone)]
struct Capture {
    buffer: Arc<Mutex<String>>,
}

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer
            .lock()
            .expect("capture buffer poisoned")
            .push_str(&String::from_utf8_lossy(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn memtable_produces_expected_batch_shape() {
    let mut table = MemTable::new(8);
    for index in 0..5 {
        table.append_record(&sample(index, "INFO"));
    }

    let batch = table.finish().expect("finish memtable");
    assert_eq!(batch.num_rows(), 5, "5 appended records must yield 5 rows");
    assert_eq!(
        batch.num_columns(),
        9,
        "the canonical schema declares 9 columns"
    );
}

#[test]
fn pipeline_of_15000_logs_acks_all_and_triggers_flush() {
    let captured = Capture {
        buffer: Arc::new(Mutex::new(String::new())),
    };
    let writer = tracing_subscriber::fmt::writer::BoxMakeWriter::new({
        let buffer = captured.buffer.clone();
        move || -> Box<dyn Write + Send + 'static> {
            Box::new(Capture {
                buffer: buffer.clone(),
            })
        }
    });
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(writer)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("install global tracing subscriber");

    let dir = tempdir().expect("create temp dir");
    let config = Arc::new(EngineConfig {
        data_dir: dir.path().join("logs"),
        wal_path: dir.path().join("current.wal"),
        mpsc_buffer_size: 64,
        crossbeam_buffer_size: 64,
        ..EngineConfig::default()
    });

    let (ingest_tx, ingest_rx) = mpsc::channel(config.mpsc_buffer_size);
    let (handoff_tx, handoff_rx) =
        crossbeam::channel::bounded::<Vec<LogRecord>>(config.crossbeam_buffer_size);
    let (truncate_tx, truncate_rx) = mpsc::channel::<usize>(config.mpsc_buffer_size);

    let flusher = ParquetFlusher::new(&config);
    let wal_handle = spawn_wal_worker(config.clone(), ingest_rx, truncate_rx, handoff_tx);
    let memtable_handle = spawn_memtable_worker(
        handoff_rx,
        truncate_tx,
        flusher,
        LiveBuffer::default(),
        config,
    );

    let runtime = tokio::runtime::Runtime::new().expect("build runtime");
    runtime.block_on(async {
        let limit = EngineConfig::default().flush_row_limit;
        let total = limit + 5_000;
        let per_batch = 1_000;
        for _ in 0..(total / per_batch) {
            let records: Vec<LogRecord> =
                (0..per_batch).map(|index| sample(index, "INFO")).collect();
            let (responder, ack) = oneshot::channel();
            let command = WalCommand::Append(IngestBatch::new(records, responder));
            ingest_tx.send(command).await.expect("enqueue ingest batch");

            let outcome = tokio::time::timeout(Duration::from_secs(10), ack)
                .await
                .expect("ack must arrive within timeout")
                .expect("worker must respond");
            assert!(outcome.is_ok(), "every batch must be fsynced before ack");
        }
        drop(ingest_tx);
    });

    wal_handle.join().expect("wal worker must exit cleanly");
    memtable_handle
        .join()
        .expect("memtable worker must exit cleanly");

    let output = captured.buffer.lock().expect("capture buffer");
    assert!(
        output.contains("Flushed batch of 10000 rows"),
        "a threshold flush of 10000 rows must be observed, got: {output}"
    );
    assert!(
        output.contains("Flushed batch of 5000 rows"),
        "the trailing 5000 rows must flush on shutdown, got: {output}"
    );
}
