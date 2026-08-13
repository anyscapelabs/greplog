//! Runtime configuration for the write path.
//!
//! Every tuneable knob of the engine lives in [`EngineConfig`]. The workers and
//! the flusher read their values from the struct instance; nothing in the
//! production code assumes a fixed directory, a fixed WAL file name, or a fixed
//! row count. Callers override the fields they care about and keep the rest via
//! struct-update syntax:
//!
//! ```
//! use greplog_engine::config::EngineConfig;
//!
//! let config = EngineConfig {
//!     data_dir: std::path::PathBuf::from("/var/log/greplog"),
//!     ..EngineConfig::default()
//! };
//! ```

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// The shared, queryable in-memory log tier.
///
/// The `MemTable` worker appends finished columnar [`RecordBatch`](arrow::record_batch::RecordBatch)es here under
/// a write lock; the [`QueryEngine`](crate::query::QueryEngine) clones the batch
/// vector under a read lock just-in-time. Cloning only bumps the Arrow array
/// reference counts, so a query never copies row data.
pub type LiveBuffer = Arc<RwLock<Vec<arrow::record_batch::RecordBatch>>>;

/// Tuning knobs for the ingest, WAL, and storage pipeline.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Root directory for Hive-partitioned Parquet chunks.
    pub data_dir: PathBuf,
    /// Exact path of the Write-Ahead Log file.
    pub wal_path: PathBuf,
    /// Rows buffered in the `MemTable` before it flushes to Parquet.
    pub flush_row_limit: usize,
    /// Seconds a `MemTable` with pending rows may stay unflushed before a
    /// periodic flush to Parquet fires, independent of row count.
    pub flush_interval_secs: u64,
    /// Seconds between background compaction sweeps of the partition tree.
    pub compaction_run_interval_secs: u64,
    /// Leaf partitions holding more Parquet chunks than this get merged into a
    /// single highly compressed file.
    pub max_files_before_compaction: usize,
    /// Days of Parquet history to keep before automatic purge. `None` disables
    /// retention (data is kept forever).
    pub retention_days: Option<u32>,
    /// Seconds between background retention sweeps of the partition tree.
    pub retention_run_interval_secs: u64,
    /// Maximum seconds a single query may run before it is cancelled.
    pub query_timeout_secs: u64,
    /// Maximum number of rows a single query may return.
    pub max_query_rows: usize,
    /// Capacity of the `tokio` MPSC ingest channels.
    pub mpsc_buffer_size: usize,
    /// Capacity of the crossbeam record-handoff channel.
    pub crossbeam_buffer_size: usize,
    /// TCP port for the ingest API (`POST /api/log` on `0.0.0.0:5050`).
    pub ingest_port: u16,
    /// TCP port for the dashboard API (`POST /api/query`, `GET /api/tail` on `0.0.0.0:3000`).
    pub dashboard_port: u16,
}

impl Default for EngineConfig {
    /// Sensible out-of-the-box values for a developer machine.
    ///
    /// The defaults are only a fallback: production deployments are expected to
    /// build their own instance and leave the fields they want customized.
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/logs"),
            wal_path: PathBuf::from("data/wal/current.wal"),
            flush_row_limit: 10_000,
            flush_interval_secs: 10,
            compaction_run_interval_secs: 3600,
            max_files_before_compaction: 5,
            retention_days: Some(30),
            retention_run_interval_secs: 3600,
            query_timeout_secs: 30,
            max_query_rows: 10_000,
            mpsc_buffer_size: 1_024,
            crossbeam_buffer_size: 1_024,
            ingest_port: 5050,
            dashboard_port: 3000,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::EngineConfig;

    #[test]
    fn defaults_point_at_the_conventional_layout() {
        let config = EngineConfig::default();
        assert_eq!(config.data_dir, PathBuf::from("data/logs"));
        assert_eq!(config.wal_path, PathBuf::from("data/wal/current.wal"));
        assert_eq!(config.compaction_run_interval_secs, 3600);
        assert_eq!(config.max_files_before_compaction, 5);
        assert_eq!(config.retention_days, Some(30));
        assert_eq!(config.retention_run_interval_secs, 3600);
        assert!(config.flush_row_limit > 0, "flush threshold must allow progress");
        assert!(
            config.flush_interval_secs > 0,
            "periodic flush cadence must allow progress"
        );
        assert!(
            config.compaction_run_interval_secs > 0,
            "compaction cadence must allow progress"
        );
        assert!(
            config.max_files_before_compaction > 0,
            "compaction trigger must allow merging"
        );
        assert!(
            config.retention_run_interval_secs > 0,
            "retention cadence must allow progress"
        );
        assert!(
            config.query_timeout_secs > 0,
            "query timeout must allow progress"
        );
        assert!(
            config.max_query_rows > 0,
            "query row cap must allow progress"
        );
        assert!(config.mpsc_buffer_size > 0, "ingest channel capacity must allow progress");
        assert!(config.crossbeam_buffer_size > 0, "handoff capacity must allow progress");
    }

    #[test]
    fn cloning_preserves_an_isolated_copy() {
        let original = EngineConfig {
            data_dir: PathBuf::from("/tmp/logs"),
            ..EngineConfig::default()
        };
        let clone = original.clone();
        assert_eq!(clone.data_dir, original.data_dir);
        assert_eq!(clone.wal_path, original.wal_path);
        assert_eq!(clone.flush_row_limit, original.flush_row_limit);
        assert_eq!(clone.flush_interval_secs, original.flush_interval_secs);
        assert_eq!(
            clone.compaction_run_interval_secs,
            original.compaction_run_interval_secs
        );
        assert_eq!(
            clone.max_files_before_compaction,
            original.max_files_before_compaction
        );
        assert_eq!(clone.retention_days, original.retention_days);
        assert_eq!(
            clone.retention_run_interval_secs,
            original.retention_run_interval_secs
        );
        assert_eq!(clone.query_timeout_secs, original.query_timeout_secs);
        assert_eq!(clone.max_query_rows, original.max_query_rows);
        assert_eq!(clone.mpsc_buffer_size, original.mpsc_buffer_size);
        assert_eq!(clone.crossbeam_buffer_size, original.crossbeam_buffer_size);
        assert_eq!(clone.ingest_port, original.ingest_port);
        assert_eq!(clone.dashboard_port, original.dashboard_port);
    }

    #[test]
    fn struct_update_overrides_only_the_chosen_field() {
        let config = EngineConfig {
            flush_row_limit: 512,
            ..EngineConfig::default()
        };
        assert_eq!(config.flush_row_limit, 512);
        assert_eq!(config.data_dir, PathBuf::from("data/logs"));
        assert_eq!(config.mpsc_buffer_size, EngineConfig::default().mpsc_buffer_size);
    }
}