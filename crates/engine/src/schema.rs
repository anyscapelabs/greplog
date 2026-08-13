//! The Apache Arrow schema for stored log data.
//!
//! Columnar layout mirrors [`crate::record::LogRecord`] one-to-one. Low-cardinality
//! columns (`level`, `service`) are dictionary-encoded and optional payload columns
//! (`trace_id`, `raw_body`) are nullable to waste zero space when absent.
//!
//! `level` uses an `Int16` key so that even a pathological client cannot exceed
//! the key space; `MemTable` additionally normalizes every level into a bounded
//! enumerated set so the dictionary stays tiny in practice.

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

/// Returns the canonical Arrow [`Schema`] for Greplog log data.
///
/// Field order is meaningful:
/// 1. `timestamp_us` — the partition-pruning axis, microsecond precision.
/// 2. `trace_id` — nullable correlation id (worker job / HTTP request).
/// 3. `level` — dictionary-encoded (Int16) severity level.
/// 4. `service` — dictionary-encoded (Int16) source application.
/// 5. `message` — the raw log summary.
/// 6. `raw_body` — nullable stringified JSON payload.
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