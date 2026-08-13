//! The in-memory columnar buffer: converts row-oriented [`LogRecord`]s into an
//! Apache Arrow [`RecordBatch`] using the official array builders.
//!
//! The builder choice mirrors [`crate::schema::greplog_schema`] exactly so that
//! `RecordBatch::try_new` accepts the result without casts:
//! - `level` is dictionary-encoded with an `Int16` key and every value is
//!   normalized to a small enumerated set, so the key space can never exceed
//!   what the dictionary type holds.
//! - `service` is dictionary-encoded with an `Int16` key.
//! - `trace_id` and `raw_body` are nullable `Utf8` columns built with
//!   `append_option`.
//!
//! The table shadows every appended [`LogRecord`] so that a failed columnar
//! build is fully recoverable: [`MemTable::drain_to_batch`] hands the source
//! records back to the caller to requeue instead of silently dropping them.

use std::sync::Arc;

use arrow::array::{
    ArrayBuilder, ArrayRef, StringDictionaryBuilder, StringBuilder, TimestampMicrosecondBuilder,
};
use arrow::datatypes::Int16Type;
use arrow::record_batch::RecordBatch;

use crate::error::EngineError;
use crate::record::LogRecord;
use crate::schema::greplog_schema;

/// The bounded set of severity levels stored in the `level` dictionary.
///
/// Anything outside this set is bucketed to [`NORMALIZED_PREFIX`] + level, so a
/// malformed client can never grow the `level` key space past what the
/// dictionary-encoded column can hold.
const LEVELS: &[&str] = &["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL", "CRITICAL", "UNKNOWN"];

/// Normalizes an arbitrary level string to a value in [`LEVELS`].
///
/// Case-insensitive; unrecognized values are bucketed to `UNKNOWN` rather than
/// being stored verbatim, which keeps the `level` dictionary bounded.
#[must_use]
pub fn normalize_level(level: &str) -> &'static str {
    let upper = level.trim().to_ascii_uppercase();
    if let Some(canonical) = LEVELS.iter().find(|known| **known == upper) {
        canonical
    } else {
        "UNKNOWN"
    }
}

/// An append-only columnar buffer for [`LogRecord`]s.
pub struct MemTable {
    timestamps: TimestampMicrosecondBuilder,
    trace_ids: StringBuilder,
    levels: StringDictionaryBuilder<Int16Type>,
    services: StringDictionaryBuilder<Int16Type>,
    messages: StringBuilder,
    raw_bodies: StringBuilder,
    /// Shadow of every appended record, enabling lossless recovery when the
    /// columnar build fails.
    pending_records: Vec<LogRecord>,
}

impl MemTable {
    /// Creates an empty table with the given per-builder capacity hint.
    ///
    /// The hint is a pre-allocation, not a bound: the builders grow as needed,
    /// so it should track the row count the caller flushes at.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let varchar_capacity = capacity.saturating_mul(8);
        Self {
            timestamps: TimestampMicrosecondBuilder::with_capacity(capacity),
            trace_ids: StringBuilder::with_capacity(capacity, varchar_capacity),
            levels: StringDictionaryBuilder::with_capacity(capacity, varchar_capacity, varchar_capacity),
            services: StringDictionaryBuilder::with_capacity(capacity, varchar_capacity, varchar_capacity),
            messages: StringBuilder::with_capacity(capacity, varchar_capacity),
            raw_bodies: StringBuilder::with_capacity(capacity, varchar_capacity),
            pending_records: Vec::with_capacity(capacity),
        }
    }

    /// Pushes one log record into the columnar buffers.
    ///
    /// The level is normalized to the bounded [`LEVELS`] set, so a malformed or
    /// pathological client can never overflow the dictionary key space.
    pub fn append_record(&mut self, record: &LogRecord) {
        self.timestamps.append_value(record.timestamp_us);
        append_optional(&mut self.trace_ids, record.trace_id.as_deref());
        self.levels.append_value(normalize_level(&record.level));
        self.services.append_value(record.service.as_str());
        self.messages.append_value(record.message.as_str());
        append_optional(&mut self.raw_bodies, record.raw_body.as_deref());
        self.pending_records.push(record.clone());
    }

    /// Finishes the current batch and resets the table for the next one.
    ///
    /// The returned [`RecordBatch`] follows [`greplog_schema`]. The table is
    /// emptied and can be immediately reused for the following batch.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ArrowError`] if the finished arrays do not match
    /// the canonical schema.
    pub fn finish(&mut self) -> Result<RecordBatch, EngineError> {
        self.drain_to_batch().map_err(|(error, _records)| error)
    }

    /// Finishes the batch, returning the source records on failure.
    ///
    /// On success the table is reset and the batch returned. On a columnar
    /// build failure the table is also reset, but every record that fed the
    /// failed build is returned so the caller can requeue them — nothing is
    /// silently dropped.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ArrowError`] alongside the affected records if
    /// the finished arrays do not match the canonical schema.
    pub fn drain_to_batch(&mut self) -> Result<RecordBatch, (EngineError, Vec<LogRecord>)> {
        let pending = std::mem::take(&mut self.pending_records);
        build_batch(
            std::mem::take(&mut self.timestamps),
            std::mem::take(&mut self.trace_ids),
            std::mem::take(&mut self.levels),
            std::mem::take(&mut self.services),
            std::mem::take(&mut self.messages),
            std::mem::take(&mut self.raw_bodies),
        )
        .map_err(|error| (error, pending))
    }

    /// Returns the number of rows currently staged in the builders.
    #[must_use]
    pub fn num_rows(&self) -> usize {
        self.timestamps.len()
    }

    /// Returns true if no rows are staged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }
}

/// Consumes the builders and assembles the finished [`RecordBatch`].
fn build_batch(
    mut timestamps: TimestampMicrosecondBuilder,
    mut trace_ids: StringBuilder,
    mut levels: StringDictionaryBuilder<Int16Type>,
    mut services: StringDictionaryBuilder<Int16Type>,
    mut messages: StringBuilder,
    mut raw_bodies: StringBuilder,
) -> Result<RecordBatch, EngineError> {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(timestamps.finish()),
        Arc::new(trace_ids.finish()),
        Arc::new(levels.finish()),
        Arc::new(services.finish()),
        Arc::new(messages.finish()),
        Arc::new(raw_bodies.finish()),
    ];
    RecordBatch::try_new(greplog_schema(), columns).map_err(EngineError::from)
}

/// Appends an optional string, preserving `None` as a null slot.
fn append_optional(builder: &mut StringBuilder, value: Option<&str>) {
    match value {
        Some(text) => builder.append_value(text),
        None => builder.append_null(),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_level, MemTable};
    use crate::record::LogRecord;

    fn sample(index: usize, level: &str) -> LogRecord {
        LogRecord {
            timestamp_us: 1_700_000_000_000_000 + i64::try_from(index).expect("fit i64"),
            trace_id: Some(format!("trace-{index}")),
            level: level.to_string(),
            service: "payment-worker".to_string(),
            message: format!("order {index} processed"),
            raw_body: Some(r#"{"order_id": 9876}"#.to_string()),
        }
    }

    #[test]
    fn normalize_level_buckets_anything() {
        assert_eq!(normalize_level("INFO"), "INFO");
        assert_eq!(normalize_level("info"), "INFO", "case-insensitive");
        assert_eq!(normalize_level("  warn "), "WARN", "whitespace-trimmed");
        assert_eq!(normalize_level("Critical"), "CRITICAL");
        assert_eq!(
            normalize_level("catastrophic"),
            "UNKNOWN",
            "unbounded levels degrade to UNKNOWN instead of growing the dictionary"
        );
    }

    #[test]
    fn wholesale_unbounded_levels_build_without_loss() {
        // > 127 distinct levels used to overflow the Int8 dictionary key space
        // and poison the whole batch. The normalized set is far smaller, so the
        // batch builds and every row survives.
        let mut table = MemTable::new(256);
        for index in 0..300 {
            table.append_record(&sample(index, &format!("level_{index}")));
        }

        let batch = table.finish().expect("finish must never fail on many levels");
        assert_eq!(batch.num_rows(), 300, "every row must survive regardless of level");
    }

    #[test]
    fn drain_to_batch_resets_table_on_success() {
        let mut table = MemTable::new(8);
        for index in 0..5 {
            table.append_record(&sample(index, "INFO"));
        }

        let batch = table.drain_to_batch().expect("drain builds a batch");
        assert_eq!(batch.num_rows(), 5);
        assert!(table.is_empty(), "successful drain must reset the table");
    }

    #[test]
    fn finish_produces_expected_rows_and_columns() {
        let mut table = MemTable::new(8);
        for index in 0..5 {
            table.append_record(&sample(index, "INFO"));
        }

        let batch = table.finish().expect("finish");
        assert_eq!(batch.num_rows(), 5);
        assert_eq!(batch.num_columns(), 6);
    }

    #[test]
    fn option_fields_become_null_when_absent() {
        let mut table = MemTable::new(8);
        let mut record = sample(1, "WARN");
        record.trace_id = None;
        record.raw_body = None;
        table.append_record(&record);

        let batch = table.finish().expect("finish");
        assert_eq!(batch.column(1).null_count(), 1, "trace_id must be null");
        assert_eq!(batch.column(5).null_count(), 1, "raw_body must be null");
    }

    #[test]
    fn populated_optional_fields_have_no_nulls() {
        let mut table = MemTable::new(8);
        table.append_record(&sample(1, "INFO"));

        let batch = table.finish().expect("finish");
        assert_eq!(batch.column(1).null_count(), 0, "trace_id must be present");
        assert_eq!(batch.column(5).null_count(), 0, "raw_body must be present");
    }

    #[test]
    fn finish_resets_table_for_reuse() {
        let mut table = MemTable::new(8);
        for index in 0..3 {
            table.append_record(&sample(index, "INFO"));
        }
        let first = table.finish().expect("first batch");
        assert_eq!(first.num_rows(), 3);

        for index in 0..2 {
            table.append_record(&sample(index, "ERROR"));
        }
        let second = table.finish().expect("second batch");
        assert_eq!(second.num_rows(), 2);
    }

    #[test]
    fn empty_table_finishes_to_zero_rows() {
        let mut table = MemTable::new(8);
        let batch = table.finish().expect("finish");
        assert_eq!(batch.num_rows(), 0);
    }

    #[test]
    fn tables_grow_beyond_their_capacity_hint() {
        let mut table = MemTable::new(4);
        for index in 0..16 {
            table.append_record(&sample(index, "INFO"));
        }
        let batch = table.finish().expect("finish");
        assert_eq!(batch.num_rows(), 16, "capacity is a hint, not a bound");
    }
}