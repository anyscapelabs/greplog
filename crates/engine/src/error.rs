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

    /// A binary serialization error from `bincode` while writing or reading
    /// the Write-Ahead Log.
    #[error("bincode error: {0}")]
    BincodeError(#[from] Box<bincode::ErrorKind>),

    /// An upstream error from the Apache Parquet runtime while flushing or
    /// reading columnar data.
    #[error("parquet error: {0}")]
    ParquetError(#[from] parquet::errors::ParquetError),
}

/// Convenience constructor for [`EngineError::ParseError`].
///
/// Keeps call sites concise (`return Err(EngineError::parse("..."))`) without
/// formatting machinery leaking into the error domain.
#[must_use]
pub fn parse_error(message: impl Into<String>) -> EngineError {
    EngineError::ParseError(message.into())
}

#[cfg(test)]
mod tests {
    use super::{parse_error, EngineError};
    use bincode::ErrorKind;

    #[test]
    fn variant_messages_are_descriptive() {
        assert!(EngineError::ParseError("bad".to_string()).to_string().contains("parse error"));

        let io = EngineError::IoError(std::io::Error::other("boom"));
        assert!(io.to_string().contains("io error"));

        let arrow = EngineError::ArrowError(arrow::error::ArrowError::SchemaError("nope".into()));
        assert!(arrow.to_string().contains("arrow error"));

        let bincode = EngineError::BincodeError(Box::new(ErrorKind::Custom("nope".to_string())));
        assert!(bincode.to_string().contains("bincode error"));
    }

    #[test]
    fn from_conversions_cover_underlying_errors() {
        let io: EngineError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
        assert!(matches!(io, EngineError::IoError(_)));

        let bincode: EngineError = Box::new(ErrorKind::SizeLimit).into();
        assert!(matches!(bincode, EngineError::BincodeError(_)));
    }

    #[test]
    fn parse_error_helper_wraps_message() {
        assert!(matches!(
            parse_error("unexpected token"),
            EngineError::ParseError(message) if message == "unexpected token"
        ));
    }
}