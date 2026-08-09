//! Greplog engine: the typed, zero-data-loss logging core.
//!
//! Round 1 supplies the three foundational domains:
//! - [`error`]: the crate-wide [`EngineError`] type.
//! - [`record`]: the [`LogRecord`] wire model.
//! - [`schema`]: the canonical Arrow [`Schema`](arrow::datatypes::Schema).

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod error;
pub mod record;
pub mod schema;

pub use error::EngineError;
pub use record::LogRecord;
pub use schema::greplog_schema;