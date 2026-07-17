use anyhow::Result;
use bytes::Bytes;
use duckdb::{params, Connection};
use greplog_core::gen::IngestBatch;
use greplog_core::redact::{redact_string, RedactionMode};
use prost::Message;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub mod wal;

pub struct Writer {
    db: Arc<Mutex<Connection>>,
}

impl Writer {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let db = Connection::open(&db_path)?;

        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS logs (
                id VARCHAR PRIMARY KEY,
                service VARCHAR NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                level VARCHAR,
                message VARCHAR,
                attributes JSON
            );
            CREATE TABLE IF NOT EXISTS spans (
                id VARCHAR PRIMARY KEY,
                correlation_id VARCHAR,
                service VARCHAR NOT NULL,
                name VARCHAR,
                start_time TIMESTAMP NOT NULL,
                end_time TIMESTAMP NOT NULL,
                attributes JSON
            );
            CREATE TABLE IF NOT EXISTS metrics (
                id VARCHAR PRIMARY KEY,
                service VARCHAR NOT NULL,
                name VARCHAR NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                value DOUBLE NOT NULL,
                labels JSON
            );
            ",
        )?;

        info!("Store writer initialized at {:?}", db_path);
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub fn spawn(
        self,
        mut batch_rx: mpsc::Receiver<Bytes>,
        flush_interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        let db = self.db.clone();
        tokio::spawn(async move {
            let mut pending_batches: Vec<IngestBatch> = Vec::new();
            let flush_interval = Duration::from_secs(flush_interval_secs);
            let mut flush_ticker = tokio::time::interval(flush_interval);

            loop {
                tokio::select! {
                    _ = flush_ticker.tick() => {
                        if !pending_batches.is_empty() {
                            let batches = std::mem::take(&mut pending_batches);
                            let db_ref = db.clone();
                            if let Err(e) = tokio::task::spawn_blocking(move || {
                                flush_batches(&db_ref, &batches)
                            }).await.unwrap_or_else(|e| Err(anyhow::anyhow!("Join error: {}", e))) {
                                error!("Flush failed: {}", e);
                            }
                        }
                    }
                    msg = batch_rx.recv() => {
                        match msg {
                            Some(bytes) => {
                                match IngestBatch::decode(bytes) {
                                    Ok(batch) => pending_batches.push(batch),
                                    Err(e) => error!("Failed to decode IngestBatch: {}", e),
                                }
                            }
                            None => {
                                info!("Writer channel closed, flushing remaining and exiting");
                                if !pending_batches.is_empty() {
                                    let db_ref = db.clone();
                                    let batches = std::mem::take(&mut pending_batches);
                                    if let Err(e) = tokio::task::spawn_blocking(move || {
                                        flush_batches(&db_ref, &batches)
                                    }).await.unwrap_or_else(|e| Err(anyhow::anyhow!("Join error: {}", e))) {
                                        error!("Final flush failed: {}", e);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        })
    }
}

fn flush_batches(db: &Arc<Mutex<Connection>>, batches: &[IngestBatch]) -> Result<()> {
    debug!("Flushing {} batches to store", batches.len());
    let mut conn = db.lock().map_err(|e| anyhow::anyhow!("DB lock poisoned: {}", e))?;
    let tx = conn.transaction()?;

    let mut log_stmt = tx.prepare_cached(
        "INSERT OR IGNORE INTO logs (id, service, timestamp, level, message, attributes)
         VALUES (?, ?, to_timestamp(? / 1000000000.0), ?, ?, ?)",
    )?;
    let mut span_stmt = tx.prepare_cached(
        "INSERT OR IGNORE INTO spans (id, correlation_id, service, name, start_time, end_time, attributes)
         VALUES (?, ?, ?, ?, to_timestamp(? / 1000000000.0), to_timestamp(? / 1000000000.0), ?)",
    )?;
    let mut metric_stmt = tx.prepare_cached(
        "INSERT OR IGNORE INTO metrics (id, service, name, timestamp, value, labels)
         VALUES (?, ?, ?, to_timestamp(? / 1000000000.0), ?, ?)",
    )?;

    for batch in batches {
        let service = &batch.service_name;

        for log in &batch.logs {
            let mut attrs = log.attributes.clone();
            redact_map(&mut attrs);
            let attrs_json = serde_json::to_string(&attrs).unwrap_or_default();
            let id = uuid::Uuid::new_v4().to_string();
            log_stmt.execute(params![
                id,
                service,
                log.timestamp_ns,
                log.level,
                log.message,
                attrs_json
            ])?;
        }

        for span in &batch.spans {
            let mut attrs = span.attributes.clone();
            redact_map(&mut attrs);
            let attrs_json = serde_json::to_string(&attrs).unwrap_or_default();
            let id = uuid::Uuid::new_v4().to_string();
            span_stmt.execute(params![
                id,
                span.correlation_id,
                service,
                span.name,
                span.start_time_ns,
                span.end_time_ns,
                attrs_json
            ])?;
        }

        for metric in &batch.metrics {
            let mut labels = metric.labels.clone();
            redact_map(&mut labels);
            let labels_json = serde_json::to_string(&labels).unwrap_or_default();
            let id = uuid::Uuid::new_v4().to_string();
            metric_stmt.execute(params![
                id,
                service,
                metric.name,
                metric.timestamp_ns,
                metric.value,
                labels_json
            ])?;
        }
    }

    drop(log_stmt);
    drop(span_stmt);
    drop(metric_stmt);
    tx.commit()?;
    Ok(())
}

/// Redact sensitive keys in a protobuf map<string, string>.
fn redact_map(map: &mut std::collections::HashMap<String, String>) {
    for (key, value) in map.iter_mut() {
        let k = key.to_lowercase();
        if k.contains("password") || k.contains("token") || k.contains("secret") {
            *value = redact_string(value, RedactionMode::Full);
        } else if k.contains("email") {
            *value = redact_string(value, RedactionMode::Partial);
        }
    }
}
