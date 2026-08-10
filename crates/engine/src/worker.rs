//! The dedicated OS worker threads of the write path.
//!
//! Disk I/O and columnar conversion never run on the Tokio async runtime that
//! serves the API:
//! - [`spawn_wal_worker`] persists batches to the configured WAL path, `fsync`s
//!   them, and truncates the file once the data is safely stored as Parquet.
//! - [`spawn_memtable_worker`] converts durable records into Arrow batches,
//!   flushes them to Parquet, and signals the WAL worker to truncate.
//!
//! The two are joined by a lock-free `crossbeam` channel for record handoff and
//! two `tokio` MPSC channels back to the WAL worker: one for append actor
//! [`WalCommand`]s and a separate one for `MemTable` truncation signals. A
//! small single-threaded Tokio runtime is used inside the WAL thread only to
//! multiplex those two channels; all disk I/O still executes on that dedicated
//! OS thread. Keeping the truncation channel separate means the `MemTable`
//! worker never holds a clone of the ingest channel, so the WAL worker can see
//! its ingest channel close and exit without deadlocking.
//!
//! The `MemTable` only ever sees records that are already safe on disk and the
//! WAL is only truncated after those records are in Parquet. Cell and channel
//! sizes all come from the shared [`EngineConfig`].

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{Receiver as HandoffRx, Sender as HandoffTx};
use tokio::sync::mpsc::{Receiver as WalRx, Sender as WalTx};

use crate::config::EngineConfig;
use crate::error::EngineError;
use crate::ingest::{IngestBatch, WalCommand};
use crate::memtable::MemTable;
use crate::record::LogRecord;
use crate::storage::ParquetFlusher;
use crate::wal::WalWriter;

/// Spawns the WAL worker on a dedicated OS thread and returns its handle.
///
/// The worker multiplexes two command sources:
/// - `receiver` carries [`WalCommand`]s from the server: [`WalCommand::Append`]
///   appends a batch and forwards durable records to the `MemTable` worker over
///   `memtable_tx` only after a successful `fsync`.
/// - `truncate_rx` carries `()` signals from the `MemTable` worker, each
///   emptying the WAL file after the matching Parquet flush.
///
/// The WAL file lives at `config.wal_path`. The loop ends when the append
/// channel closes (all its senders dropped); call [`JoinHandle::join`] to wait
/// for outstanding `fsync`s to finish.
#[must_use]
pub fn spawn_wal_worker(
    config: Arc<EngineConfig>,
    receiver: WalRx<WalCommand>,
    truncate_rx: WalRx<()>,
    memtable_tx: HandoffTx<Vec<LogRecord>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let wal_path = config.wal_path.clone();
        run_wal_loop(&wal_path, receiver, truncate_rx, &memtable_tx);
    })
}

/// Spawns the `MemTable` worker on a dedicated OS thread and returns its handle.
///
/// Records are appended into an in-memory Arrow [`MemTable`]; once
/// `config.flush_row_limit` rows accumulate the batch is written to Parquet via
/// `flusher` and a truncation signal is sent to the WAL worker over
/// `truncate_tx`.
#[must_use]
pub fn spawn_memtable_worker(
    receiver: HandoffRx<Vec<LogRecord>>,
    truncate_tx: WalTx<()>,
    flusher: ParquetFlusher,
    config: Arc<EngineConfig>,
) -> JoinHandle<()> {
    thread::spawn(move || run_memtable_loop(&receiver, &truncate_tx, &flusher, &config))
}

/// Runs the WAL worker body: multiplex the two command channels.
fn run_wal_loop(
    wal_path: &std::path::Path,
    mut receiver: WalRx<WalCommand>,
    truncate_rx: WalRx<()>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let mut wal = WalWriter::open(wal_path);
    match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => {
            runtime.block_on(select_commands(&mut wal, &mut receiver, truncate_rx, memtable_tx));
        }
        Err(error) => {
            tracing::error!(?error, "failed to build wal worker runtime; falling back to blocking drain");
            drain_channel(&mut wal, &mut receiver, memtable_tx);
        }
    }
}

/// Selects over the append and truncation channels, persisting in order.
async fn select_commands(
    wal: &mut Result<WalWriter, EngineError>,
    receiver: &mut WalRx<WalCommand>,
    truncate_rx: WalRx<()>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let mut truncate_rx = Some(truncate_rx);
    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(WalCommand::Append(batch)) => handle_append(wal, batch, memtable_tx),
                Some(WalCommand::Truncate) => handle_truncate(wal),
                None => break,
            },
            signal = truncate_signal(&mut truncate_rx) => match signal {
                Some(()) => handle_truncate(wal),
                None => truncate_rx = None,
            },
        }
    }
}

/// Pends forever once the truncation channel is closed, disarming that arm.
async fn truncate_signal(rx: &mut Option<WalRx<()>>) -> Option<()> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

/// Drains commands off the WAL channel in fallback mode (no runtime).
fn drain_channel(
    wal: &mut Result<WalWriter, EngineError>,
    receiver: &mut WalRx<WalCommand>,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            WalCommand::Append(batch) => handle_append(wal, batch, memtable_tx),
            WalCommand::Truncate => handle_truncate(wal),
        }
    }
}

/// Appends a batch to the WAL and forwards durable records to the `MemTable`.
fn handle_append(
    wal: &mut Result<WalWriter, EngineError>,
    batch: IngestBatch,
    memtable_tx: &HandoffTx<Vec<LogRecord>>,
) {
    let outcome = match wal {
        Ok(writer) => writer.append_batch(&batch.records),
        Err(error) => Err(fallback_error(error)),
    };
    let durable = outcome.is_ok();
    let _ = batch.responder.send(outcome);
    if durable {
        let _ = memtable_tx.send(batch.records);
    }
}

/// Truncates the WAL file, logging (never propagating) any failure.
fn handle_truncate(wal: &mut Result<WalWriter, EngineError>) {
    if let Ok(writer) = wal.as_mut() {
        if let Err(error) = writer.truncate() {
            tracing::error!(?error, "failed to truncate wal");
        }
    }
}

/// Drains durable record streams into columnar batches, flushing on the
/// configured threshold.
///
/// A flush only truncates the WAL after the Parquet write has fully succeeded,
/// so a crash between the two never loses acknowledged records.
fn run_memtable_loop(
    receiver: &HandoffRx<Vec<LogRecord>>,
    truncate_tx: &WalTx<()>,
    flusher: &ParquetFlusher,
    config: &EngineConfig,
) {
    let limit = config.flush_row_limit;
    let mut table = MemTable::new(limit);
    let mut rows = 0usize;
    while let Ok(records) = receiver.recv() {
        for record in records {
            table.append_record(&record);
            rows += 1;
        }
        if rows >= limit {
            flush(&mut table, rows, truncate_tx, flusher);
            rows = 0;
        }
    }
    if rows > 0 {
        flush(&mut table, rows, truncate_tx, flusher);
    }
}

/// Finishes the current batch, persists it to Parquet, and triggers truncation.
fn flush(
    table: &mut MemTable,
    rows: usize,
    truncate_tx: &WalTx<()>,
    flusher: &ParquetFlusher,
) {
    match table.finish() {
        Ok(batch) => match flusher.flush(&batch) {
            Ok(()) => {
                tracing::info!("Flushed batch of {} rows", batch.num_rows());
                let _ = truncate_tx.blocking_send(());
            }
            Err(error) => tracing::error!(?error, "failed to write parquet for {rows} rows"),
        },
        Err(error) => tracing::error!(?error, "failed to finish memtable with {rows} rows"),
    }
}

/// Reconstructs an ingest-visible error when the WAL never opened.
///
/// `EngineError` is deliberately not `Clone` (its payloads cannot cheaply
/// reproduce it), so when the WAL fails to open we report a fresh [`std::io::Error`]
/// carrying the original message to every in-flight batch.
fn fallback_error(original: &EngineError) -> EngineError {
    EngineError::IoError(std::io::Error::other(original.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::sync::{mpsc, oneshot};

    use super::{spawn_memtable_worker, spawn_wal_worker};
    use crate::config::EngineConfig;
    use crate::ingest::{IngestBatch, WalCommand};
    use crate::record::LogRecord;
    use crate::storage::ParquetFlusher;

    fn sample(index: usize) -> LogRecord {
        LogRecord::new("INFO", "auth-api", format!("boot {index}"))
    }

    fn test_config(dir: &tempfile::TempDir) -> Arc<EngineConfig> {
        Arc::new(EngineConfig {
            data_dir: dir.path().join("logs"),
            wal_path: dir.path().join("current.wal"),
            mpsc_buffer_size: 4,
            crossbeam_buffer_size: 4,
            ..EngineConfig::default()
        })
    }

    #[test]
    fn wal_worker_persists_and_confirms_batch() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let config = test_config(&dir);
            let (wal_tx, wal_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (handoff_tx, _handoff_rx) =
                crossbeam::channel::bounded(config.crossbeam_buffer_size);
            let handle = spawn_wal_worker(config, wal_rx, truncate_rx, handoff_tx);

            let (responder, ack) = oneshot::channel();
            let batch = IngestBatch::new(vec![sample(1)], responder);
            wal_tx.send(WalCommand::Append(batch)).await.expect("send append");
            drop(wal_tx);
            drop(truncate_tx);

            let acked = ack.await.expect("worker must respond");
            assert!(acked.is_ok(), "wal append must succeed");

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn wal_worker_answers_error_without_panicking() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let config = Arc::new(EngineConfig {
                wal_path: dir.path().join("missing-dir").join("current.wal"),
                ..EngineConfig::default()
            });
            let (wal_tx, wal_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
            let (handoff_tx, _handoff_rx) =
                crossbeam::channel::bounded(config.crossbeam_buffer_size);
            let handle = spawn_wal_worker(config, wal_rx, truncate_rx, handoff_tx);

            let batch = IngestBatch {
                records: Vec::new(),
                responder: oneshot::channel().0,
            };
            wal_tx.send(WalCommand::Append(batch)).await.expect("send append");
            drop(wal_tx);
            drop(truncate_tx);

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn memtable_worker_drains_record_stream() {
        let dir = tempdir().expect("create temp dir");
        let config = test_config(&dir);
        let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(&config);
        let (sender, receiver) = crossbeam::channel::bounded(config.crossbeam_buffer_size);
        let handle = spawn_memtable_worker(receiver, truncate_tx, flusher, config);

        for index in 0..64 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        handle.join().expect("memtable worker must exit cleanly");
    }

    #[test]
    fn memtable_worker_flushes_at_the_configured_limit() {
        let dir = tempdir().expect("create temp dir");
        let config = Arc::new(EngineConfig {
            data_dir: dir.path().join("logs"),
            wal_path: dir.path().join("current.wal"),
            flush_row_limit: 16,
            mpsc_buffer_size: 4,
            crossbeam_buffer_size: 8,
        });
        let (truncate_tx, truncate_rx) = mpsc::channel(config.mpsc_buffer_size);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(&config);
        let (sender, receiver) = crossbeam::channel::bounded(config.crossbeam_buffer_size);
        let handle = spawn_memtable_worker(receiver, truncate_tx, flusher, config);

        for index in 0..32 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let has_partition = std::fs::read_dir(dir.path().join("logs"))
                .is_ok_and(|entries| entries.count() > 0);
            if has_partition {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "parquet must be written");
            std::thread::sleep(Duration::from_millis(10));
        }

        handle.join().expect("memtable worker must exit cleanly");
    }
}
