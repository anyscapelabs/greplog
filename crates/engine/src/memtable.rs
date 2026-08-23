//! The in-memory columnar buffer: converts row-oriented [`LogRecord`]s into an
//! Apache Arrow [`RecordBatch`].
//!
//! Durability is not this type's job: the WAL has already `fsync`ed every
//! record that reaches [`MemTable::append_record`], and its segment is only
//! reclaimed after the row lands in Parquet.

use std::sync::Arc;

use arrow::array::{
    ArrayBuilder, ArrayRef, StringBuilder, StringDictionaryBuilder, TimestampMicrosecondBuilder,
};
use arrow::datatypes::Int16Type;
use arrow::record_batch::RecordBatch;

use crate::error::EngineError;
use crate::record::LogRecord;
use crate::schema::greplog_schema;

/// Bounded set of severity levels stored in the `level` dictionary; anything
/// else degrades to `UNKNOWN`, so a malformed client can never overflow the
/// dictionary key space.
const LEVELS: &[&str] = &["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL", "CRITICAL", "UNKNOWN"];

#[must_use]
pub fn normalize_level(level: &str) -> &'static str {
    let upper = level.trim().to_ascii_uppercase();
    LEVELS
        .iter()
        .find(|known| **known == upper)
        .copied()
        .unwrap_or("UNKNOWN")
}

pub struct MemTable {
    timestamps: TimestampMicrosecondBuilder,
    trace_ids: StringBuilder,
    levels: StringDictionaryBuilder<Int16Type>,
    services: StringDictionaryBuilder<Int16Type>,
    messages: StringBuilder,
    raw_bodies: StringBuilder,
}

impl MemTable {
    /// Creates an empty table; `capacity` is a pre-allocation hint, not a bound.
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
        }
    }

    /// Pushes one log record into the columnar buffers.
    ///
    /// Every call appends to all six builders, which is what keeps their
    /// lengths equal and [`MemTable::finish`] infallible in practice.
    pub fn append_record(&mut self, record: &LogRecord) {
        self.timestamps.append_value(record.timestamp_us);
        append_optional(&mut self.trace_ids, record.trace_id.as_deref());
        self.levels.append_value(normalize_level(&record.level));
        self.services.append_value(record.service.as_str());
        self.messages.append_value(record.message.as_str());
        append_optional(&mut self.raw_bodies, record.raw_body.as_deref());
    }

    pub fn finish(&mut self) -> Result<RecordBatch, EngineError> {
        build_batch(
            std::mem::take(&mut self.timestamps),
            std::mem::take(&mut self.trace_ids),
            std::mem::take(&mut self.levels),
            std::mem::take(&mut self.services),
            std::mem::take(&mut self.messages),
            std::mem::take(&mut self.raw_bodies),
        )
    }

    #[must_use]
    pub fn num_rows(&self) -> usize {
        self.timestamps.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }
}

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
    fn finish_resets_the_table_on_success() {
        let mut table = MemTable::new(8);
        for index in 0..5 {
            table.append_record(&sample(index, "INFO"));
        }

        let batch = table.finish().expect("finish builds a batch");
        assert_eq!(batch.num_rows(), 5);
        assert!(table.is_empty(), "a successful finish must reset the table");
    }

    #[test]
    fn every_column_stays_the_same_length_as_the_row_count() {
        // The infallibility of `finish` rests on all six builders receiving
        // exactly one append per record. Pin that invariant directly rather
        // than paying for a shadow copy of every row to guard against it.
        let mut table = MemTable::new(4);
        for index in 0..7 {
            let mut record = sample(index, "WARN");
            if index.is_multiple_of(2) {
                record.trace_id = None;
                record.raw_body = None;
            }
            table.append_record(&record);
        }
        assert_eq!(table.num_rows(), 7);

        let batch = table.finish().expect("finish");
        assert_eq!(batch.num_columns(), 6);
        for column in batch.columns() {
            assert_eq!(column.len(), 7, "every column must match the row count");
        }
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