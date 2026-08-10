//! The dedicated OS worker threads of the write path.
//!
//! Disk I/O and columnar conversion never run on the Tokio async runtime that
//! serves the API:
//! - [`spawn_wal_worker`] persists batches to `current.wal`, `fsync`s, and
//!   truncates it once data is safely stored as Parquet.
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
//! WAL is only truncated after those records are in Parquet.

use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{Receiver as HandoffRx, Sender as HandoffTx};
use tokio::sync::mpsc::{Receiver as WalRx, Sender as WalTx};

use crate::error::EngineError;
use crate::ingest::{IngestBatch, WalCommand};
use crate::memtable::{MemTable, FLUSH_THRESHOLD};
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
///   emptying `current.wal` after the matching Parquet flush.
///
/// The loop ends when the append channel closes (all its senders dropped);
/// call [`JoinHandle::join`] to wait for outstanding `fsync`s to finish.
#[must_use]
pub fn spawn_wal_worker(
    wal_path: PathBuf,
    receiver: WalRx<WalCommand>,
    truncate_rx: WalRx<()>,
    memtable_tx: HandoffTx<Vec<LogRecord>>,
) -> JoinHandle<()> {
    thread::spawn(move || run_wal_loop(&wal_path, receiver, truncate_rx, &memtable_tx))
}

/// Spawns the `MemTable` worker on a dedicated OS thread and returns its handle.
///
/// Records are appended into an in-memory Arrow [`MemTable`]; once
/// [`FLUSH_THRESHOLD`] rows accumulate the batch is written to Parquet via
/// `flusher` and a truncation signal is sent to the WAL worker over
/// `truncate_tx`.
#[must_use]
pub fn spawn_memtable_worker(
    receiver: HandoffRx<Vec<LogRecord>>,
    truncate_tx: WalTx<()>,
    flusher: ParquetFlusher,
) -> JoinHandle<()> {
    thread::spawn(move || run_memtable_loop(&receiver, &truncate_tx, &flusher))
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

/// Truncates `current.wal`, logging (never propagating) any failure.
fn handle_truncate(wal: &mut Result<WalWriter, EngineError>) {
    if let Ok(writer) = wal.as_mut() {
        if let Err(error) = writer.truncate() {
            tracing::error!(?error, "failed to truncate wal");
        }
    }
}

/// Drains durable record streams into columnar batches, flushing on threshold.
///
/// A flush only truncates the WAL after the Parquet write has fully succeeded,
/// so a crash between the two never loses acknowledged records.
fn run_memtable_loop(
    receiver: &HandoffRx<Vec<LogRecord>>,
    truncate_tx: &WalTx<()>,
    flusher: &ParquetFlusher,
) {
    let mut table = MemTable::new();
    let mut rows = 0usize;
    while let Ok(records) = receiver.recv() {
        for record in records {
            table.append_record(&record);
            rows += 1;
        }
        if rows >= FLUSH_THRESHOLD {
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
    use tempfile::tempdir;
    use tokio::sync::{mpsc, oneshot};

    use super::{spawn_memtable_worker, spawn_wal_worker};
    use crate::ingest::{IngestBatch, WalCommand};
    use crate::record::LogRecord;
    use crate::storage::ParquetFlusher;

    fn sample(index: usize) -> LogRecord {
        LogRecord::new("INFO", "auth-api", format!("boot {index}"))
    }

    #[test]
    fn wal_worker_persists_and_confirms_batch() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let path = dir.path().join("current.wal");
            let (wal_tx, wal_rx) = mpsc::channel(4);
            let (truncate_tx, truncate_rx) = mpsc::channel(4);
            let (handoff_tx, _handoff_rx) = crossbeam::channel::unbounded();
            let handle = spawn_wal_worker(path, wal_rx, truncate_rx, handoff_tx);

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
            let path = dir.path().join("current.wal");
            let (wal_tx, wal_rx) = mpsc::channel(4);
            let (truncate_tx, truncate_rx) = mpsc::channel(4);
            let (handoff_tx, _handoff_rx) = crossbeam::channel::unbounded();
            let handle = spawn_wal_worker(path, wal_rx, truncate_rx, handoff_tx);

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
        let (truncate_tx, truncate_rx) = mpsc::channel(4);
        drop(truncate_rx);
        let flusher = ParquetFlusher::new(dir.path().join("logs"));
        let (sender, receiver) = crossbeam::channel::unbounded();
        let handle = spawn_memtable_worker(receiver, truncate_tx, flusher);

        for index in 0..64 {
            sender.send(vec![sample(index)]).expect("send record stream");
        }
        drop(sender);

        handle.join().expect("memtable worker must exit cleanly");
    }
}
