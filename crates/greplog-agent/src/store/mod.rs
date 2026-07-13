use anyhow::Result;
use bytes::Bytes;
use duckdb::{Connection, params};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Single-threaded DuckDB writer.
pub struct Writer {
    conn: Connection,
}

impl Writer {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Create tables
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS logs (
                seq          BIGINT,
                service_name VARCHAR,
                message      VARCHAR,
                level        VARCHAR,
                timestamp_ns BIGINT,
                logger_name  VARCHAR,
                file         VARCHAR,
                line         INTEGER,
                correlation_id VARCHAR,
                exception_type VARCHAR,
                exception_message VARCHAR,
                attributes   VARCHAR,
                stack_trace  VARCHAR,
                ingested_at  TIMESTAMP DEFAULT now()
            );

            CREATE TABLE IF NOT EXISTS spans (
                seq           BIGINT,
                service_name  VARCHAR,
                name          VARCHAR,
                start_time_ns BIGINT,
                end_time_ns   BIGINT,
                correlation_id VARCHAR,
                parent_correlation_id VARCHAR,
                route         VARCHAR,
                method        VARCHAR,
                status_code   INTEGER,
                error         VARCHAR,
                kind          INTEGER,
                attributes    VARCHAR,
                ingested_at   TIMESTAMP DEFAULT now()
            );

            CREATE TABLE IF NOT EXISTS metrics (
                seq           BIGINT,
                service_name  VARCHAR,
                name          VARCHAR,
                value         DOUBLE,
                timestamp_ns  BIGINT,
                type          INTEGER,
                labels        VARCHAR,
                bucket_values VARCHAR,
                ingested_at   TIMESTAMP DEFAULT now()
            );
            ",
        )?;

        // Enable WAL for concurrent reads
        conn.execute_batch("PRAGMA wal_autocheckpoint=1000;")?;

        info!("DuckDB store initialized at {:?}", db_path);
        Ok(Self { conn })
    }

    /// Spawn the writer loop on a dedicated thread (not async — DuckDB is sync).
    pub fn spawn(
        mut self,
        mut rx: mpsc::Receiver<Bytes>,
        flush_interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || {
            let mut seq: i64 = 0;
            let mut ticker = tokio::runtime::Handle::current();

            loop {
                // Process all available batches, then flush
                let mut flushed = false;

                while let Ok(buf) = rx.try_recv() {
                    flushed = true;
                    if let Err(e) = self.insert_batch(&buf, seq) {
                        error!("Failed to insert batch: {}", e);
                    }
                    seq += 1;
                }

                if flushed {
                    info!("Flushed batch to DuckDB");
                }

                // Block until next message or timeout
                let dur = Duration::from_secs(flush_interval_secs);
                let deadline = ticker.block_on(async {
                    tokio::time::sleep(dur);
                });

                // Try receiving with timeout via select
                match ticker.block_on(async {
                    tokio::select! {
                        msg = rx.recv() => msg,
                        _ = tokio::time::sleep(dur) => None,
                    }
                }) {
                    Some(buf) => {
                        if let Err(e) = self.insert_batch(&buf, seq) {
                            error!("Failed to insert batch: {}", e);
                        }
                        seq += 1;
                    }
                    None => {
                        // Timeout — flush and compact
                        if let Err(e) = self.conn.execute_batch("CHECKPOINT;") {
                            error!("Checkpoint failed: {}", e);
                        }
                    }
                }
            }
        })
    }

    fn insert_batch(&self, buf: &[u8], seq: i64) -> Result<()> {
        use greplog_core::gen;

        let batch = gen::IngestBatch::decode(buf)?;

        for log in &batch.logs {
            self.conn.execute(
                "INSERT INTO logs (seq, service_name, message, level, timestamp_ns, logger_name, file, line, correlation_id, exception_type, exception_message, attributes, stack_trace)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    seq,
                    batch.service_name,
                    log.message,
                    log.level,
                    log.timestamp_ns,
                    log.logger_name,
                    log.file,
                    log.line,
                    log.correlation_id,
                    log.exception_type,
                    log.exception_message,
                    serde_json::to_string(&log.attributes)?,
                    serde_json::to_string(&log.stack_trace)?,
                ],
            )?;
        }

        for span in &batch.spans {
            self.conn.execute(
                "INSERT INTO spans (seq, service_name, name, start_time_ns, end_time_ns, correlation_id, parent_correlation_id, route, method, status_code, error, kind, attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    seq,
                    batch.service_name,
                    span.name,
                    span.start_time_ns,
                    span.end_time_ns,
                    span.correlation_id,
                    span.parent_correlation_id,
                    span.route,
                    span.method,
                    span.status_code,
                    span.error,
                    span.kind,
                    serde_json::to_string(&span.attributes)?,
                ],
            )?;
        }

        for metric in &batch.metrics {
            self.conn.execute(
                "INSERT INTO metrics (seq, service_name, name, value, timestamp_ns, type, labels, bucket_values)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    seq,
                    batch.service_name,
                    metric.name,
                    metric.value,
                    metric.timestamp_ns,
                    metric.r#type,
                    serde_json::to_string(&metric.labels)?,
                    serde_json::to_string(&metric.bucket_values)?,
                ],
            )?;
        }

        Ok(())
    }
}
