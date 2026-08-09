//! The dedicated Write-Ahead Log worker thread.
//!
//! Disk I/O is confined to a plain [`std::thread`] so the Tokio async runtime
//! is never blocked on `fsync`. The thread drains [`IngestBatch`]es from the
//! ingest MPSC channel, persists each batch, and fires each batch's responder.

use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use tokio::sync::mpsc::Receiver;

use crate::ingest::IngestBatch;
use crate::wal::WalWriter;

/// Spawns the WAL worker on a dedicated OS thread and returns its handle.
///
/// The loop ends when the channel closes (all senders dropped); call
/// [`JoinHandle::join`] to wait for outstanding `fsync`s to finish.
#[must_use]
pub fn spawn_wal_worker(wal_path: PathBuf, mut receiver: Receiver<IngestBatch>) -> JoinHandle<()> {
    thread::spawn(move || {
        let wal = WalWriter::open(&wal_path);
        drain_channel(wal, &mut receiver);
    })
}

/// Drains batches off the channel, persisting each before acknowledging.
fn drain_channel(
    mut wal: Result<WalWriter, crate::error::EngineError>,
    receiver: &mut Receiver<IngestBatch>,
) {
    while let Some(batch) = receiver.blocking_recv() {
        let outcome = match &mut wal {
            Ok(writer) => writer.append_batch(&batch.records),
            Err(error) => Err(fallback_error(error)),
        };
        let _ = batch.responder.send(outcome);
    }
}

/// Reconstructs an ingest-visible error when the WAL never opened.
///
/// `EngineError` is deliberately not `Clone` (its payloads cannot cheaply
/// reproduce it), so when the WAL fails to open we report a fresh [`std::io::Error`]
/// carrying the original message to every in-flight batch.
fn fallback_error(original: &crate::error::EngineError) -> crate::error::EngineError {
    crate::error::EngineError::IoError(std::io::Error::other(original.to_string()))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tokio::sync::{mpsc, oneshot};

    use super::spawn_wal_worker;
    use crate::ingest::IngestBatch;
    use crate::record::LogRecord;

    #[test]
    fn worker_persists_and_confirms_batch() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let path = dir.path().join("current.wal");
            let (sender, receiver) = mpsc::channel(4);
            let handle = spawn_wal_worker(path, receiver);

            let (responder, ack) = oneshot::channel();
            let batch = IngestBatch::new(vec![LogRecord::new("INFO", "svc", "hello")], responder);
            sender.send(batch).await.expect("send batch");
            drop(sender);

            let acked = ack.await.expect("worker must respond");
            assert!(acked.is_ok(), "wal append must succeed");

            handle.join().expect("worker thread must exit cleanly");
        });
    }

    #[test]
    fn worker_answers_error_without_panicking() {
        let runtime = tokio::runtime::Runtime::new().expect("build runtime");
        runtime.block_on(async {
            let dir = tempdir().expect("create temp dir");
            let path = dir.path().join("current.wal");
            let (sender, receiver) = mpsc::channel(4);
            let handle = spawn_wal_worker(path, receiver);

            let batch = IngestBatch {
                records: Vec::new(),
                responder: oneshot::channel().0,
            };
            sender.send(batch).await.expect("send batch");
            drop(sender);

            handle.join().expect("worker thread must exit cleanly");
        });
    }
}