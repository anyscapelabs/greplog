//! Greplog engine: the typed, zero-data-loss logging core.
//!
//! Foundation domains:
//! - [`error`]: the crate-wide [`EngineError`] type.
//! - [`record`]: the [`LogRecord`] wire model.
//! - [`schema`]: the canonical Arrow [`Schema`](arrow::datatypes::Schema).
//!
//! Write-path domains:
//! - [`ingest`]: the [`IngestBatch`] unit of work.
//! - [`wal`]: the physical [`WalWriter`] with `fsync` durability.
//! - [`worker`]: the dedicated [`spawn_wal_worker`] OS thread.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod error;
pub mod ingest;
pub mod record;
pub mod schema;
pub mod wal;
pub mod worker;

pub use error::EngineError;
pub use ingest::IngestBatch;
pub use record::LogRecord;
pub use schema::greplog_schema;
pub use wal::WalWriter;
pub use worker::spawn_wal_worker;