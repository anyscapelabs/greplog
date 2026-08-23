//! The Arrow schema for stored log data; mirrors [`crate::record::LogRecord`].
//! Dictionary-encoded low-cardinality columns, nullable optional ones.
//! `MemTable` bounds the level dictionary by normalizing to a fixed set.

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

/// The pruning axis every time-bounded query filters on.
pub const TIMESTAMP_COLUMN: &str = "timestamp_us";

pub const SERVICE_COLUMN: &str = "service";

#[must_use]
pub fn greplog_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp_us", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("trace_id", DataType::Utf8, true),
        Field::new(
            "level",
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            false,
        ),
        Field::new(
            "service",
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            false,
        ),
        Field::new("message", DataType::Utf8, false),
        Field::new("raw_body", DataType::Utf8, true),
    ]))
}