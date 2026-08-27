//! The physical Write-Ahead Log writer.
//!
//! Segments are never wholesale-truncated. The active segment lives at
//! `current.wal`; once its records are durably stored as Parquet the worker
//! rotates it to a `sealed-<n>.wal` segment. Sealed segments are only deleted
//! once every record they contain has been confirmed, so a (mis)ordered
//! truncation can never destroy acknowledged bytes.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::EngineError;
use crate::record::LogRecord;

const SEALED_PREFIX: &str = "sealed-";

#[must_use]
pub fn sealed_path(wal_path: &Path, seq: u64) -> PathBuf {
    let name = wal_path.file_name().map_or_else(
        || "current.wal".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let directory = wal_path.parent().unwrap_or_else(|| Path::new("."));
    directory.join(format!("{SEALED_PREFIX}{seq:06}-{name}"))
}

pub struct WalWriter {
    writer: BufWriter<File>,
}

impl WalWriter {
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

    /// Appends `records` to the WAL and makes them durable with `sync_data`.
    ///
    /// Only after the sync succeeds is it safe to report success to the caller.
    pub fn append_batch(&mut self, records: &[LogRecord]) -> Result<(), EngineError> {
        for record in records {
            bincode::serialize_into(&mut self.writer, record)?;
        }
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }

    pub fn append_batch_no_sync(&mut self, records: &[LogRecord]) -> Result<(), EngineError> {
        for record in records {
            bincode::serialize_into(&mut self.writer, record)?;
        }
        Ok(())
    }

    pub fn sync_data(&mut self) -> Result<(), EngineError> {
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }

    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.writer.get_ref().metadata().map_or(0, |m| m.len())
    }

    /// Replays the WAL file at `path`, returning all recovered records.
    ///
    /// A truncated trailing record (crash mid-append) is tolerated; any other
    /// corruption is an error.
    pub fn replay_wal(path: &Path) -> Result<Vec<LogRecord>, EngineError> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(EngineError::IoError(e)),
        };
        let mut reader = BufReader::new(file);
        let mut out = Vec::new();
        let mut legacy_mode: Option<bool> = None;
        loop {
            match try_read_record(&mut reader, legacy_mode) {
                Ok((Some(record), is_legacy)) => {
                    if legacy_mode.is_none() {
                        legacy_mode = Some(is_legacy);
                    }
                    out.push(record);
                }
                Ok((None, _)) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

pub fn sealed_segments(wal_path: &Path) -> Result<Vec<PathBuf>, EngineError> {
    let Some(directory) = wal_path.parent() else {
        return Err(EngineError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "wal path has no parent directory",
        )));
    };

    let mut segments: Vec<(u64, PathBuf)> = Vec::new();
    for entry in match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(EngineError::IoError(e)),
    } {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(seq) = parse_sealed_seq(&name) else {
            continue;
        };
        segments.push((seq, entry.path()));
    }
    segments.sort_by_key(|(seq, _)| *seq);
    Ok(segments.into_iter().map(|(_, path)| path).collect())
}

pub fn record_count(path: &Path) -> Result<usize, EngineError> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(EngineError::IoError(e)),
    };
    let mut reader = BufReader::new(file);
    let mut count = 0usize;
    let mut legacy_mode: Option<bool> = None;
    loop {
        match try_read_record(&mut reader, legacy_mode) {
            Ok((Some(_), is_legacy)) => {
                if legacy_mode.is_none() {
                    legacy_mode = Some(is_legacy);
                }
                count += 1;
            }
            Ok((None, _)) => break,
            Err(e) => return Err(e),
        }
    }
    Ok(count)
}

pub fn replay_all(wal_path: &Path) -> Result<Vec<LogRecord>, EngineError> {
    let mut out = Vec::new();
    for sealed in sealed_segments(wal_path)? {
        out.extend(WalWriter::replay_wal(&sealed)?);
    }
    out.extend(WalWriter::replay_wal(wal_path)?);
    Ok(out)
}

fn is_end_of_log(error: &bincode::ErrorKind) -> bool {
    matches!(
        error,
        bincode::ErrorKind::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof
    )
}

#[derive(Deserialize)]
struct LegacyLogRecord {
    timestamp_us: i64,
    trace_id: Option<String>,
    level: String,
    service: String,
    message: String,
    raw_body: Option<String>,
}

impl From<LegacyLogRecord> for LogRecord {
    fn from(v: LegacyLogRecord) -> Self {
        Self {
            timestamp_us: v.timestamp_us,
            trace_id: v.trace_id,
            span_id: None,
            parent_span_id: None,
            duration_ms: None,
            level: v.level,
            service: v.service,
            message: v.message,
            raw_body: v.raw_body,
        }
    }
}

fn try_read_record(
    reader: &mut BufReader<File>,
    legacy_mode: Option<bool>,
) -> Result<(Option<LogRecord>, bool), EngineError> {
    use std::io::{Seek, SeekFrom};
    let pos = reader.stream_position().map_err(EngineError::IoError)?;
    let try_new_first = legacy_mode.unwrap_or(false) == false;
    if try_new_first {
        match bincode::deserialize_from::<_, LogRecord>(&mut *reader) {
            Ok(record) => return Ok((Some(record), false)),
            Err(e) if is_end_of_log(&e) => return Ok((None, false)),
            Err(e) if legacy_mode.is_none() && !is_encoding_error(&e) => {
                reader.seek(SeekFrom::Start(pos)).map_err(EngineError::IoError)?;
                match bincode::deserialize_from::<_, LegacyLogRecord>(&mut *reader) {
                    Ok(legacy) => return Ok((Some(legacy.into()), true)),
                    Err(e) if is_end_of_log(&e) => return Ok((None, true)),
                    Err(_) => {
                        reader.seek(SeekFrom::Start(pos)).map_err(EngineError::IoError)?;
                        match bincode::deserialize_from::<_, LogRecord>(&mut *reader) {
                            Ok(r) => return Ok((Some(r), false)),
                            Err(e) if is_end_of_log(&e) => return Ok((None, false)),
                            Err(e) => return Err(EngineError::BincodeError(e)),
                        }
                    }
                }
            }
            Err(e) => return Err(EngineError::BincodeError(e)),
        }
    } else {
        match bincode::deserialize_from::<_, LegacyLogRecord>(&mut *reader) {
            Ok(legacy) => return Ok((Some(legacy.into()), true)),
            Err(e) if is_end_of_log(&e) => return Ok((None, true)),
            Err(e) => return Err(EngineError::BincodeError(e)),
        }
    }
}

fn is_encoding_error(error: &bincode::ErrorKind) -> bool {
    let msg = error.to_string();
    msg.contains("invalid") || msg.contains("Utf8") || msg.contains("utf8")
}

fn parse_sealed_seq(name: &str) -> Option<u64> {
    let rest = name.strip_prefix(SEALED_PREFIX)?;
    let seq = rest.split_once('-')?.0;
    seq.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::BufReader;

    use tempfile::tempdir;

    use super::{record_count, WalWriter};
    use crate::record::LogRecord;

    fn record(trace: &str, level: &str) -> LogRecord {
        LogRecord {
            timestamp_us: 1_700_000_000_000_123,
            trace_id: Some(trace.to_string()),
            span_id: None,
            parent_span_id: None,
            duration_ms: None,
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

        wal.append_batch(&[
            record("a", "ERROR"),
            record("b", "WARN"),
            record("c", "INFO"),
        ])
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
            wal.append_batch(&[record("a", "INFO")])
                .expect("first append");
            wal.append_batch(&[record("b", "WARN"), record("c", "ERROR")])
                .expect("second append");
        }

        let file = std::fs::File::open(&path).expect("open wal for read");
        let mut reader = BufReader::new(file);
        let mut seen = Vec::new();
        while let Ok(parsed) = bincode::deserialize_from::<_, LogRecord>(&mut reader) {
            seen.push(parsed.trace_id.expect("trace id must be present"));
        }
        assert_eq!(
            seen,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn open_recreates_missing_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("brand-new.wal");
        WalWriter::open(&path).expect("open must create the file");
        assert!(path.exists());
    }

    #[test]
    fn record_count_matches_appended_records() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        {
            let mut wal = WalWriter::open(&path).expect("open wal");
            wal.append_batch(&[record("a", "INFO"), record("b", "WARN")])
                .expect("append");
            wal.append_batch(&[record("c", "ERROR")]).expect("append");
        }
        assert_eq!(record_count(&path).expect("count"), 3);
        assert_eq!(
            WalWriter::replay_wal(&path).expect("replay").len(),
            3,
            "replay must agree with record_count"
        );
    }

    #[test]
    fn replay_tolerates_a_truncated_tail_but_not_midfile_corruption() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        {
            let mut wal = WalWriter::open(&path).expect("open wal");
            wal.append_batch(&[
                record("a", "INFO"),
                record("b", "WARN"),
                record("c", "ERROR"),
            ])
            .expect("append");
        }

        // Truncate into the middle of the last record: a clean crash tail.
        let full = fs::read(&path).expect("read wal");
        let cut = full.len() - 4;
        fs::write(&path, &full[..cut]).expect("truncate");

        let recovered = WalWriter::replay_wal(&path).expect("truncated tail must not error");
        let traces: Vec<String> = recovered
            .iter()
            .map(|record| record.trace_id.clone().unwrap_or_default())
            .collect();
        assert_eq!(traces, vec!["a", "b"], "complete records survive the cut");

        // Corrupt string content mid-file: without checksums, byte flips in
        // numeric fields are indistinguishable from valid (wrong) data, but
        // invalid UTF-8 in a payload must surface instead of truncating.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a single test record is far below usize::MAX"
        )]
        let first_record_len =
            bincode::serialized_size(&record("a", "INFO")).expect("size") as usize;
        let mut corrupted = full.clone();
        corrupted[first_record_len - 2] = 0xFF;
        fs::write(&path, &corrupted).expect("write corrupted wal");
        assert!(
            WalWriter::replay_wal(&path).is_err(),
            "mid-file corruption must surface, never truncate silently"
        );
    }

    #[test]
    fn replay_all_concatenates_sealed_segments_then_active() {
        use super::{replay_all, sealed_path, sealed_segments};

        let dir = tempdir().expect("create temp dir");
        let active = dir.path().join("current.wal");
        let first = sealed_path(&active, 1);
        let second = sealed_path(&active, 2);

        {
            let mut wal = WalWriter::open(&first).expect("open sealed 1");
            wal.append_batch(&[record("s1-a", "INFO"), record("s1-b", "WARN")])
                .expect("append");
            wal.sync_data().expect("sync");
        }
        {
            let mut wal = WalWriter::open(&second).expect("open sealed 2");
            wal.append_batch(&[record("s2-a", "ERROR")])
                .expect("append");
            wal.sync_data().expect("sync");
        }
        {
            let mut wal = WalWriter::open(&active).expect("open active");
            wal.append_batch(&[record("cur-a", "INFO"), record("cur-b", "DEBUG")])
                .expect("append");
            wal.sync_data().expect("sync");
        }

        let recovered = replay_all(&active).expect("replay all");
        let traces: Vec<String> = recovered
            .iter()
            .map(|r| r.trace_id.clone().unwrap_or_default())
            .collect();
        assert_eq!(
            traces,
            vec!["s1-a", "s1-b", "s2-a", "cur-a", "cur-b"],
            "sealed segments must replay oldest-first, then the active file"
        );

        let listed = sealed_segments(&active).expect("list sealed");
        assert_eq!(
            listed,
            vec![first, second],
            "sealed segments must be ordered by sequence"
        );
    }

    #[test]
    fn sealed_segments_ignores_unrelated_files() {
        use super::sealed_segments;

        let dir = tempdir().expect("create temp dir");
        let active = dir.path().join("current.wal");
        std::fs::write(dir.path().join("sealed-01-current.wal"), [0u8; 1]).expect("write");
        std::fs::write(dir.path().join("notes.txt"), [0u8; 1]).expect("write");
        std::fs::write(dir.path().join("sealed-3-current.wal"), [0u8; 1]).expect("write");

        let listed = sealed_segments(&active).expect("list sealed");
        assert_eq!(listed.len(), 2, "only sealed-<n> wal names are segments");
    }

    #[test]
    fn legacy_wal_replays_with_new_fields_as_none() {
        use std::io::Write;
        use super::WalWriter;
        use serde::Serialize;
        #[derive(Serialize)]
        struct OldRecord {
            timestamp_us: i64,
            trace_id: Option<String>,
            level: String,
            service: String,
            message: String,
            raw_body: Option<String>,
        }
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("current.wal");
        {
            let file = std::fs::File::create(&path).expect("create wal");
            let mut writer = std::io::BufWriter::new(file);
            let old = OldRecord {
                timestamp_us: 1_700_000_000_000_123,
                trace_id: Some("legacy".into()),
                level: "INFO".into(),
                service: "auth-api".into(),
                message: "hello".into(),
                raw_body: None,
            };
            bincode::serialize_into(&mut writer, &old).expect("serialize legacy");
            writer.flush().expect("flush");
        }
        let recovered = WalWriter::replay_wal(&path).expect("legacy must replay");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].trace_id.as_deref(), Some("legacy"));
        // new tracing columns default to None, not error
        assert_eq!(recovered[0].span_id, None);
        assert_eq!(recovered[0].parent_span_id, None);
        assert_eq!(recovered[0].duration_ms, None);
        assert_eq!(record_count(&path).expect("count legacy"), 1);
    }
}
