//! The physical Write-Ahead Log writer.
//!
//! Records are serialized with `bincode` and written through a [`BufWriter`].
//! Durability is guaranteed by flushing the buffered writer to the OS and then
//! calling `sync_data` to push bytes to the physical platter/SSD before the
//! ingest acceptor is allowed to acknowledge receipt.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::EngineError;
use crate::record::LogRecord;

/// Owns the on-disk WAL file and appends batches of records to it.
pub struct WalWriter {
    writer: BufWriter<File>,
}

impl WalWriter {
    /// Opens (creating if necessary) the WAL file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the file cannot be created.
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    /// Appends `records` to the WAL and makes them durable.
    ///
    /// The write order is deliberate: serialize each record into the buffer,
    /// flush the [`BufWriter`] into the OS page cache, then `sync_data` to
    /// force the kernel to write to physical storage. Only after all three
    /// steps succeed is it safe to report success to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::BincodeError`] if any record fails to serialize,
    /// or [`EngineError::IoError`] if the flush or `fsync` fails.
    pub fn append_batch(&mut self, records: &[LogRecord]) -> Result<(), EngineError> {
        for record in records {
            bincode::serialize_into(&mut self.writer, record)?;
        }
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }

    /// Appends `records` to the buffered writer without syncing.
    ///
    /// Used by the group-commit path to coalesce multiple batches before a
    /// single `fsync`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::BincodeError`] if serialization fails.
    pub fn append_batch_no_sync(&mut self, records: &[LogRecord]) -> Result<(), EngineError> {
        for record in records {
            bincode::serialize_into(&mut self.writer, record)?;
        }
        Ok(())
    }

    /// Flushes the buffer and syncs the underlying file to durable storage.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the flush or `sync_data` fails.
    pub fn sync_data(&mut self) -> Result<(), EngineError> {
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }

    /// Returns a cheap estimate of the bytes written so far, for diagnostics
    /// and WAL-truncation decisions.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.writer
            .get_ref()
            .metadata()
            .map_or(0, |m| m.len())
    }

    /// Empties the WAL file back to zero bytes once the data is safely on disk
    /// as Parquet.
    ///
    /// Any bytes still sitting in the [`BufWriter`] are flushed to the OS first;
    /// the file is then truncated, its cursor reset to the start, and the
    /// metadata `fsync`ed so the empty length survives a crash. The writer stays
    /// usable for subsequent appends.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the flush, truncation, seek, or
    /// `fsync` fails.
    pub fn truncate(&mut self) -> Result<(), EngineError> {
        self.writer.flush()?;
        self.writer.get_ref().set_len(0)?;
        self.writer.get_ref().seek(SeekFrom::Start(0))?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }

    /// Replays the WAL file at `path` and returns all recovered records.
    ///
    /// If the file does not exist, an empty vector is returned. Deserialization
    /// loops until an `UnexpectedEof` is encountered, which signals the end of
    /// the valid log.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::IoError`] if the file cannot be opened (except
    /// `NotFound`), or [`EngineError::BincodeError`] for corrupt entries other
    /// than `UnexpectedEof`.
    pub fn replay_wal(path: &Path) -> Result<Vec<LogRecord>, EngineError> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(EngineError::IoError(e)),
        };
        let mut reader = BufReader::new(file);
        let mut out = Vec::new();
        loop {
            match bincode::deserialize_from::<_, LogRecord>(&mut reader) {
                Ok(record) => out.push(record),
                Err(e) => {
                    if let bincode::ErrorKind::Io(io_err) = e.as_ref() {
                        if io_err.kind() == std::io::ErrorKind::UnexpectedEof {
                            break;
                        }
                    }
                    // bincode may wrap UnexpectedEof without Io variant in some cases;
                    // also check error string for robustness
                    if e.to_string().contains("unexpected end of file")
                        || e.to_string().contains("UnexpectedEof")
                    {
                        break;
                    }
                    return Err(EngineError::BincodeError(e));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use tempfile::tempdir;

    use super::WalWriter;
    use crate::record::LogRecord;

    fn record(trace: &str, level: &str) -> LogRecord {
        LogRecord {
            timestamp_us: 1_700_000_000_000_123,
            trace_id: Some(trace.to_string()),
            level: level.to_string(),
            service: "payment-worker".to_string(),
            message: "payment failed".to_string(),
            raw_body: Some(r#"{"order_id": 42}"#.to_string()),
        }
    }

    #[test]
    fn append_writes_nonempty_durable_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        let mut wal = WalWriter::open(&path).expect("open wal");

        wal.append_batch(&[record("a", "ERROR"), record("b", "WARN"), record("c", "INFO")])
            .expect("append batch");

        let size = std::fs::metadata(&path).expect("stat wal").len();
        assert!(size > 0, "wal file must contain bytes");
    }

    #[test]
    fn appended_batches_read_back_in_order() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        {
            let mut wal = WalWriter::open(&path).expect("open wal");
            wal.append_batch(&[record("a", "INFO")]).expect("first append");
            wal.append_batch(&[record("b", "WARN"), record("c", "ERROR")]).expect("second append");
        }

        let file = std::fs::File::open(&path).expect("open wal for read");
        let mut reader = BufReader::new(file);
        let mut seen = Vec::new();
        while let Ok(parsed) = bincode::deserialize_from::<_, LogRecord>(&mut reader) {
            seen.push(parsed.trace_id.expect("trace id must be present"));
        }
        assert_eq!(seen, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn open_recreates_missing_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("brand-new.wal");
        WalWriter::open(&path).expect("open must create the file");
        assert!(path.exists());
    }

    #[test]
    fn truncate_empties_file_and_stays_usable() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        let mut wal = WalWriter::open(&path).expect("open wal");

        wal.append_batch(&[record("a", "INFO"), record("b", "WARN")]).expect("append");
        let pre_truncate = std::fs::metadata(&path).expect("stat wal").len();
        assert!(pre_truncate > 0);

        wal.truncate().expect("truncate wal");
        let post_truncate = std::fs::metadata(&path).expect("stat wal").len();
        assert_eq!(post_truncate, 0, "wal must be empty after truncate");

        wal.append_batch(&[record("c", "ERROR")]).expect("append after truncate");
        let after_reuse = std::fs::metadata(&path).expect("stat wal").len();
        assert!(after_reuse > 0, "wal must accept appends after truncation");
    }
}
