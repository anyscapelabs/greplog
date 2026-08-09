//! The unit of work flowing through the ingestion pipeline.
//!
//! An [`IngestBatch`] bundles the records with a [`oneshot::Sender`] so the
//! async network layer can await physical disk confirmation before returning
//! `HTTP 200 OK`.

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

#[cfg(test)]
mod tests {
    use super::IngestBatch;
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
}