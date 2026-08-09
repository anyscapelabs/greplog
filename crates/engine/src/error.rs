//! Central error domain for the Greplog engine.
//!
//! Every fallible operation in this crate returns an [`EngineError`]. Production
//! code must never panic; errors are always propagated with the `?` operator and
//! converted into a [`EngineError`] via `#[from]` conversions.

use thiserror::Error;

/// All errors that can be produced by the Greplog engine.
#[derive(Debug, Error)]
pub enum EngineError {
    /// An upstream error from the Apache Arrow runtime.
    #[error("arrow error: {0}")]
    ArrowError(#[from] arrow::error::ArrowError),

    /// A parse failure, e.g. malformed JSON, an invalid timestamp, or an
    /// unrecognized log level.
    #[error("parse error: {0}")]
    ParseError(String),

    /// An I/O error while reading or writing WAL, Parquet, or log files.
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Convenience constructor for [`EngineError::ParseError`].
///
/// Keeps call sites concise (`return Err(EngineError::parse("..."))`) without
/// formatting machinery leaking into the error domain.
#[must_use]
pub fn parse_error(message: impl Into<String>) -> EngineError {
    EngineError::ParseError(message.into())
}