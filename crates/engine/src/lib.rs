//! Greplog engine: the typed, zero-data-loss logging core.

#![warn(clippy::all, clippy::pedantic)]
// Pedantic style calls: error docs per function are noise here; the types
// and the central error enum carry that contract.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod compactor;
pub mod config;
pub mod error;
pub mod ingest;
pub mod memtable;
pub mod query;
pub mod record;
pub mod retention;
pub mod schema;
pub mod search;
pub mod storage;
pub mod wal;
pub mod worker;

pub use compactor::Compactor;
pub use config::EngineConfig;
pub use error::EngineError;
pub use ingest::{IngestBatch, WalCommand};
pub use memtable::MemTable;
pub use query::QueryEngine;
pub use record::LogRecord;
pub use retention::Retention;
pub use schema::greplog_schema;
pub use search::{GroupDimension, LogSearch, Metric, SearchMode};
pub use storage::ParquetFlusher;
pub use wal::WalWriter;
pub use worker::{spawn_memtable_worker, spawn_wal_worker};
