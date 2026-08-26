//! Central error domain for the Greplog engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("arrow error: {0}")]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("datafusion error: {0}")]
    DataFusion(#[from] datafusion::error::DataFusionError),

    #[error("rwlock poisoned")]
    LockError,

    #[error("query timed out")]
    QueryTimeout,

    #[error("query rejected: {0}")]
    QueryRejected(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("bincode error: {0}")]
    BincodeError(#[from] Box<bincode::ErrorKind>),

    #[error("parquet error: {0}")]
    ParquetError(#[from] parquet::errors::ParquetError),
}

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
        assert!(EngineError::ParseError("bad".to_string())
            .to_string()
            .contains("parse error"));

        let io = EngineError::IoError(std::io::Error::other("boom"));
        assert!(io.to_string().contains("io error"));

        let arrow = EngineError::ArrowError(arrow::error::ArrowError::SchemaError("nope".into()));
        assert!(arrow.to_string().contains("arrow error"));

        let datafusion =
            EngineError::DataFusion(datafusion::error::DataFusionError::Plan("nope".into()));
        assert!(datafusion.to_string().contains("datafusion error"));

        let bincode = EngineError::BincodeError(Box::new(ErrorKind::Custom("nope".to_string())));
        assert!(bincode.to_string().contains("bincode error"));

        assert!(EngineError::LockError
            .to_string()
            .contains("rwlock poisoned"));
    }

    #[test]
    fn from_conversions_cover_underlying_errors() {
        let io: EngineError = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
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
