//! Greplog engine: the typed, zero-data-loss logging core.
//!
//! Foundation domains:
//! - [`error`]: the crate-wide [`EngineError`] type.
//! - [`record`]: the [`LogRecord`] wire model.
//! - [`schema`]: the canonical Arrow [`Schema`](arrow::datatypes::Schema).
//!
//! Write-path domains:
//! - [`config`]: the runtime [`EngineConfig`] tuning knobs.
//! - [`ingest`]: the [`IngestBatch`] unit of work + [`WalCommand`] actor commands.
//! - [`wal`]: the physical [`WalWriter`] with `fsync` durability + truncation.
//! - [`memtable`]: the Arrow columnar [`MemTable`] buffer.
//! - [`storage`]: the Hive-partitioned Parquet [`ParquetFlusher`].
//! - [`worker`]: the WAL + `MemTable` OS-thread workers.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod config;
pub mod error;
pub mod ingest;
pub mod memtable;
pub mod record;
pub mod schema;
pub mod storage;
pub mod wal;
pub mod worker;

pub use config::EngineConfig;
pub use error::EngineError;
pub use ingest::{IngestBatch, WalCommand};
pub use memtable::MemTable;
pub use record::LogRecord;
pub use schema::greplog_schema;
pub use storage::ParquetFlusher;
pub use wal::WalWriter;
pub use worker::{spawn_memtable_worker, spawn_wal_worker};