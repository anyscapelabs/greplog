//! The unit of work flowing through the ingestion pipeline.

use tokio::sync::oneshot;

use crate::error::EngineError;
use crate::record::LogRecord;

/// A batch of records plus a responder fired once the batch is fsynced (or
/// with the error that prevented it).
#[derive(Debug)]
pub struct IngestBatch {
    /// The log records to append to the Write-Ahead Log.
    pub records: Vec<LogRecord>,
    pub responder: oneshot::Sender<Result<(), EngineError>>,
}

impl IngestBatch {
    #[must_use]
    pub fn new(
        records: Vec<LogRecord>,
        responder: oneshot::Sender<Result<(), EngineError>>,
    ) -> Self {
        Self { records, responder }
    }
}

/// A command directed at the WAL worker actor. Sealed-segment reclamation is
/// not a command here: the MemTable worker owns a separate signal channel, so
/// the two write phases share no channel and can never deadlock.
#[derive(Debug)]
pub enum WalCommand {
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