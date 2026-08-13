//! The unit of work flowing through the ingestion pipeline.
//!
//! An [`IngestBatch`] bundles the records with a [`oneshot::Sender`] so the
//! async network layer can await physical disk confirmation before returning
//! `HTTP 200 OK`. The WAL worker listens on a single channel of actor-style
//! [`WalCommand`]s carrying append batches; space reclamation is handled by a
//! separate, dedicated signal channel owned by the `MemTable` worker so the
//! two write-phases can never deadlock on the same channel.

use tokio::sync::oneshot;

use crate::error::EngineError;
use crate::record::LogRecord;

/// A batch of log records plus a responder that signals durability.
#[derive(Debug)]
pub struct IngestBatch {
    /// The log records to append to the Write-Ahead Log.
    pub records: Vec<LogRecord>,
    /// Fired with `Ok(())` once the batch is `fsync`ed to stable storage, or
    /// with the [`EngineError`] that prevented it.
    pub responder: oneshot::Sender<Result<(), EngineError>>,
}

impl IngestBatch {
    /// Constructs a new [`IngestBatch`].
    #[must_use]
    pub fn new(
        records: Vec<LogRecord>,
        responder: oneshot::Sender<Result<(), EngineError>>,
    ) -> Self {
        Self { records, responder }
    }
}

/// A command directed at the WAL worker actor.
///
/// The server sends [`WalCommand::Append`] batches for durable persistence.
/// Sealed-segment reclamation is *not* a command here: the `MemTable` worker
/// owns a separate signal channel so the two write phases share no channel
/// and can never deadlock.
#[derive(Debug)]
pub enum WalCommand {
    /// Append a batch of log records to the active WAL segment.
    Append(IngestBatch),
}

#[cfg(test)]
mod tests {
    use super::{IngestBatch, WalCommand};
    use crate::record::LogRecord;
    use tokio::sync::oneshot;

    #[test]
    fn batch_carries_records_and_responder() {
        let (responder, mut ack) = oneshot::channel();
        let first = LogRecord::new("INFO", "auth-api", "signed in");
        let second = LogRecord::new("ERROR", "auth-api", "sign in failed");
        let batch = IngestBatch::new(vec![first, second], responder);

        assert_eq!(batch.records.len(), 2);
        assert_eq!(batch.records[0].level, "INFO");
        assert_eq!(batch.records[1].level, "ERROR");
        assert!(matches!(
            ack.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ), "ack must stay pending until the worker fires it");
    }

    #[test]
    fn append_command_preserves_its_batch() {
        let (responder, _ack) = oneshot::channel();
        let command = WalCommand::Append(IngestBatch::new(
            vec![LogRecord::new("WARN", "auth-api", "slow")],
            responder,
        ));

        match command {
            WalCommand::Append(batch) => assert_eq!(batch.records.len(), 1),
        }
    }
}