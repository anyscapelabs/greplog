//! Structured log searches compiled into partition-pruned physical queries.
//!
//! Dashboard requests arrive as JSON, never as SQL. This module owns that
//! boundary: it validates the request, derives Hive partition predicates from
//! the time range so the listing table prunes directories, runs one pruned
//! query per tier (Parquet + live buffer), and merges the results.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray, PrimitiveBuilder, StringBuilder};
use arrow::datatypes::{
    DataType, Field, Int64Type, Schema, SchemaRef, TimeUnit, TimestampMicrosecondType,
};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;

use crate::error::EngineError;
use crate::memtable::normalize_level;
use crate::schema::greplog_schema;

const BUCKET_ORIGIN: &str = "TIMESTAMP '1970-01-01T00:00:00Z'";
const ROW_COLUMNS: &str = "timestamp_us, trace_id, level, service, message, raw_body";

/// Partition enumeration pads one extra day on the left so a record flushed
/// just after midnight under yesterday's folder is never pruned away.
const PARTITION_PAD_DAYS: i64 = 1;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LogSearch {
    pub time_range_secs: u64,
    #[serde(default)]
    pub facets: HashMap<String, String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(flatten)]
    pub mode: SearchMode,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchMode {
    Rows {
        #[serde(default = "default_row_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    Aggregate {
        #[serde(default)]
        group_by: Vec<GroupDimension>,
        #[serde(default = "default_bucket_secs")]
        bucket_secs: u64,
        #[serde(default)]
        metrics: Vec<Metric>,
    },
}

fn default_row_limit() -> usize {
    500
}

fn default_bucket_secs() -> u64 {
    60
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum GroupDimension {
    Bucket,
    Level,
    Service,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Count,
    Errors,
    Warns,
    LastSeen,
}

impl LogSearch {
    pub fn validate(&self, max_rows: usize) -> Result<(), EngineError> {
        if self.time_range_secs == 0 {
            return Err(search_error("time_range_secs must be greater than zero"));
        }

        for facet in self.facets.keys() {
            if !matches!(facet.as_str(), "level" | "service") {
                return Err(search_error(format!(
                    "unknown facet column '{facet}', expected 'level' or 'service'"
                )));
            }
            if self.facets[facet].trim().is_empty() {
                return Err(search_error(format!("facet '{facet}' has an empty value")));
            }
        }

        match &self.mode {
            SearchMode::Rows { limit, offset } => {
                if *limit == 0 || *limit > max_rows {
                    return Err(search_error(format!(
                        "limit must be between 1 and {max_rows}"
                    )));
                }
                if offset.saturating_add(*limit) > max_rows {
                    return Err(search_error(format!(
                        "offset + limit must not exceed {max_rows}"
                    )));
                }
            }
            SearchMode::Aggregate {
                group_by,
                bucket_secs,
                metrics,
            } => {
                if has_duplicates(group_by) {
                    return Err(search_error("group_by dimensions must be unique"));
                }
                if *bucket_secs == 0 && group_by.contains(&GroupDimension::Bucket) {
                    return Err(search_error(
                        "bucket_secs must be greater than zero when grouping by bucket",
                    ));
                }
                if (*bucket_secs) > self.time_range_secs {
                    return Err(search_error("bucket_secs cannot exceed time_range_secs"));
                }
                if metrics.is_empty() {
                    return Err(search_error("aggregate requires at least one metric"));
                }
                if has_duplicates(metrics) {
                    return Err(search_error("metrics must be unique"));
                }
            }
        }
        Ok(())
    }
}

fn has_duplicates<T: Ord + Copy>(items: &[T]) -> bool {
    let mut sorted = items.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.len() != items.len()
}

fn search_error(message: impl Into<String>) -> EngineError {
    EngineError::QueryRejected(message.into())
}

/// The `(year, month, day)` tuples a window covers, padded by one day on the
/// left so records flushed under an older day folder stay reachable.
fn partition_days(now: DateTime<Utc>, time_range_secs: u64) -> Vec<(i32, u32, u32)> {
    let start = now
        - chrono::Duration::seconds(i64::try_from(time_range_secs).unwrap_or(i64::MAX))
        - chrono::Duration::days(PARTITION_PAD_DAYS);
    let end = now.date_naive();
    let mut days = Vec::new();
    let mut current = start.date_naive();
    while current <= end {
        days.push((current.year(), current.month(), current.day()));
        current = current
            .succ_opt()
            .expect("chrono dates never overflow here");
    }
    days
}

/// Single-clause Hive filter.
///
/// `DataFusion` 39 mis-prunes when two partition columns are conjuncted (e.g.
/// `year IN (2026) AND month IN (8)` matches zero files), while any single
/// partition clause prunes correctly. One column is also enough for
/// correctness — the timestamp predicate cleans up the over-inclusion — so the
/// clause granularity follows the window span: days for a month, months for
/// two years, years beyond that.
fn partition_predicate(days: &[(i32, u32, u32)]) -> Option<String> {
    if days.is_empty() {
        return None;
    }

    let mut years: Vec<i32> = Vec::new();
    let mut months: Vec<u32> = Vec::new();
    let mut day_numbers: Vec<u32> = Vec::new();
    for (year, month, day) in days {
        if !years.contains(year) {
            years.push(*year);
        }
        if !months.contains(month) {
            months.push(*month);
        }
        if !day_numbers.contains(day) {
            day_numbers.push(*day);
        }
    }

    let render = |name: &str, values: Vec<String>| format!("{name} IN ({})", values.join(", "));
    if days.len() <= 32 {
        let mut sorted = day_numbers;
        sorted.sort_unstable();
        Some(render(
            "day",
            sorted
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        ))
    } else if days.len() <= 730 {
        Some(render(
            "month",
            months
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        ))
    } else {
        Some(render(
            "year",
            years.iter().map(std::string::ToString::to_string).collect(),
        ))
    }
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn timestamp_literal(micros: i64) -> String {
    let moment = chrono::DateTime::from_timestamp(
        micros.div_euclid(1_000_000),
        u32::try_from(micros.rem_euclid(1_000_000))
            .unwrap_or(0)
            .saturating_mul(1_000),
    )
    .unwrap_or_else(Utc::now);
    format!("TIMESTAMP '{}'", moment.format("%Y-%m-%dT%H:%M:%S%.6fZ"))
}

fn split_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for character in query.chars() {
        match character {
            '"' => in_quotes = !in_quotes,
            space if space.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    terms.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }
    terms
}

fn strip_quotes(value: &str) -> &str {
    let stripped = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        });
    stripped.unwrap_or(value)
}

fn term_condition(term: &str) -> Option<String> {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(position) = trimmed.find([':', '=']) {
        let field = trimmed[..position].trim();
        let is_field_name =
            !field.is_empty() && field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let value = strip_quotes(trimmed[position + 1..].trim());
        if is_field_name && !value.is_empty() {
            return Some(match field.to_ascii_lowercase().as_str() {
                "message" | "raw_body" => {
                    format!("{field} ILIKE {}", sql_literal(&format!("%{value}%")))
                }
                "level" => format!("level = {}", sql_literal(normalize_level(value))),
                "service" => format!("service = {}", sql_literal(value)),
                _ => return None,
            });
        }
    }

    let free_text = strip_quotes(trimmed);
    if free_text.is_empty() {
        return None;
    }
    let needle = sql_literal(&format!("%{free_text}%"));
    Some(format!(
        "(message ILIKE {needle} OR service ILIKE {needle} OR raw_body ILIKE {needle} \
         OR LOWER(level) = {})",
        sql_literal(&free_text.to_ascii_lowercase())
    ))
}

fn where_clause(search: &LogSearch, cutoff_us: i64) -> String {
    let mut parts = vec![format!("timestamp_us >= {}", timestamp_literal(cutoff_us))];

    if let Some(level) = search.facets.get("level") {
        parts.push(format!("level = {}", sql_literal(normalize_level(level))));
    }
    if let Some(service) = search.facets.get("service") {
        parts.push(format!("service = {}", sql_literal(service)));
    }

    for term in search.search.iter().flat_map(|query| split_terms(query)) {
        if let Some(condition) = term_condition(&term) {
            parts.push(condition);
        }
    }

    parts.join(" AND ")
}

fn group_expressions(group_by: &[GroupDimension], bucket_secs: u64) -> Vec<String> {
    group_by
        .iter()
        .map(|dimension| match dimension {
            GroupDimension::Bucket => {
                format!("date_bin(INTERVAL '{bucket_secs} SECONDS', timestamp_us, {BUCKET_ORIGIN}) AS bucket")
            }
            GroupDimension::Level => "level".to_string(),
            GroupDimension::Service => "service".to_string(),
        })
        .collect()
}

fn metric_expressions(metrics: &[Metric]) -> Vec<String> {
    metrics
        .iter()
        .map(|metric| match metric {
            Metric::Count => "COUNT(*) AS count".to_string(),
            Metric::Errors => {
                "SUM(CASE WHEN level = 'ERROR' THEN 1 ELSE 0 END) AS errors".to_string()
            }
            Metric::Warns => "SUM(CASE WHEN level = 'WARN' THEN 1 ELSE 0 END) AS warns".to_string(),
            Metric::LastSeen => "MAX(timestamp_us) AS last_seen".to_string(),
        })
        .collect()
}

impl super::query::QueryEngine {
    pub async fn search(&self, search: &LogSearch) -> Result<Vec<RecordBatch>, EngineError> {
        search.validate(self.max_query_rows)?;

        let now = Utc::now();
        let cutoff_us = now.timestamp_micros()
            - i64::try_from(search.time_range_secs).unwrap_or(i64::MAX) * 1_000_000;
        let predicate = where_clause(search, cutoff_us);
        let partition_filter = partition_predicate(&partition_days(now, search.time_range_secs));
        let parquet_predicate = match &partition_filter {
            Some(filter) => format!("{predicate} AND {filter}"),
            None => predicate.clone(),
        };

        match &search.mode {
            SearchMode::Rows { limit, offset } => {
                // Fetch limit+offset per tier so global offset is correct after merge.
                let fetch_limit = offset.saturating_add(*limit);
                let parquet_sql = format!(
                    "SELECT {ROW_COLUMNS} FROM parquet_logs WHERE {parquet_predicate} \
                     ORDER BY timestamp_us DESC LIMIT {fetch_limit}"
                );
                let live_sql = format!(
                    "SELECT {ROW_COLUMNS} FROM live_logs WHERE {predicate} \
                     ORDER BY timestamp_us DESC LIMIT {fetch_limit}"
                );
                let (parquet_df, live_df) =
                    tokio::join!(self.ctx.sql(&parquet_sql), self.ctx.sql(&live_sql));
                merge_rows(
                    [parquet_df?.collect().await?, live_df?.collect().await?].concat(),
                    *limit,
                    *offset,
                )
            }
            SearchMode::Aggregate {
                group_by,
                bucket_secs,
                metrics,
            } => {
                let mut select = group_expressions(group_by, *bucket_secs);
                select.extend(metric_expressions(metrics));
                // An empty group_by is a global aggregate: no GROUP BY clause,
                // one output row per tier to merge.
                let group_clause = if group_by.is_empty() {
                    String::new()
                } else {
                    let ordinals: Vec<String> = (1..=group_by.len())
                        .map(|position| position.to_string())
                        .collect();
                    format!(" GROUP BY {}", ordinals.join(", "))
                };

                let parquet_sql = format!(
                    "SELECT {} FROM parquet_logs WHERE {parquet_predicate}{group_clause}",
                    select.join(", "),
                );
                let live_sql = format!(
                    "SELECT {} FROM live_logs WHERE {predicate}{group_clause}",
                    select.join(", "),
                );

                let (parquet_df, live_df) =
                    tokio::join!(self.ctx.sql(&parquet_sql), self.ctx.sql(&live_sql));
                merge_aggregate(
                    [parquet_df?.collect().await?, live_df?.collect().await?].concat(),
                    group_by,
                    metrics,
                )
            }
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
enum KeyPart {
    Text(String),
    Micros(i64),
}
impl KeyPart {
    fn sort_rank(&self) -> (u8, i64, String) {
        match self {
            KeyPart::Micros(micros) => (0, *micros, String::new()),
            KeyPart::Text(text) => (1, 0, text.clone()),
        }
    }
}

struct AggregateRow {
    count: i64,
    errors: i64,
    warns: i64,
    last_seen: i64,
}

/// Merges both tiers into one newest-first page: `[offset, offset + limit)`.
///
/// Each tier already returned its top `offset + limit` rows, so sorting only
/// that prefix is enough to serve the requested window.
fn merge_rows(
    batches: Vec<RecordBatch>,
    limit: usize,
    offset: usize,
) -> Result<Vec<RecordBatch>, EngineError> {
    let schema = greplog_schema();
    let normalized: Vec<RecordBatch> = batches
        .into_iter()
        .filter(|batch| batch.num_rows() > 0)
        .map(|batch| normalize_to_schema(&batch, &schema))
        .collect::<Result<_, _>>()?;
    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let combined = arrow::compute::concat_batches(&schema, &normalized)?;
    if combined.num_rows() == 0 {
        return Ok(Vec::new());
    }

    if offset >= combined.num_rows() {
        return Ok(Vec::new());
    }

    let rows_through_page_end = offset.saturating_add(limit).min(combined.num_rows());
    let timestamp_index = schema.index_of("timestamp_us")?;
    let indices = arrow::compute::lexsort_to_indices(
        &[arrow::compute::SortColumn {
            values: combined.column(timestamp_index).clone(),
            options: Some(arrow::compute::SortOptions {
                descending: true,
                nulls_first: false,
            }),
        }],
        Some(rows_through_page_end),
    )?;
    let sorted = arrow::compute::take_record_batch(&combined, &indices)?;
    if offset == 0 {
        return Ok(vec![sorted]);
    }
    let remaining = sorted.num_rows().saturating_sub(offset);
    Ok(vec![sorted.slice(offset, remaining.min(limit))])
}

fn normalize_to_schema(
    batch: &RecordBatch,
    schema: &SchemaRef,
) -> Result<RecordBatch, EngineError> {
    let columns: Vec<ArrayRef> = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let column = batch.column(index);
            if column.data_type() == field.data_type() {
                return Ok(Arc::clone(column));
            }
            arrow::compute::cast(column, field.data_type()).map_err(EngineError::from)
        })
        .collect::<Result<_, _>>()?;
    RecordBatch::try_new(Arc::clone(schema), columns).map_err(EngineError::from)
}

fn merge_aggregate(
    batches: Vec<RecordBatch>,
    group_by: &[GroupDimension],
    metrics: &[Metric],
) -> Result<Vec<RecordBatch>, EngineError> {
    let mut rows: HashMap<Vec<KeyPart>, AggregateRow> = HashMap::new();

    for batch in batches {
        if batch.num_rows() == 0 {
            continue;
        }
        let columns: Vec<ArrayRef> = (0..batch.num_columns())
            .map(|index| canonical_column(batch.column(index)))
            .collect::<Result<_, _>>()?;

        for row in 0..batch.num_rows() {
            let key = group_key(&columns[..group_by.len()], row)?;
            let entry = rows.entry(key).or_insert(AggregateRow {
                count: 0,
                errors: 0,
                warns: 0,
                last_seen: i64::MIN,
            });
            for (offset, metric) in metrics.iter().enumerate() {
                let value = columns[group_by.len() + offset]
                    .as_primitive::<Int64Type>()
                    .value(row);
                match metric {
                    Metric::Count => entry.count += value,
                    Metric::Errors => entry.errors += value,
                    Metric::Warns => entry.warns += value,
                    Metric::LastSeen => entry.last_seen = entry.last_seen.max(value),
                }
            }
        }
    }

    let mut merged: Vec<(Vec<KeyPart>, AggregateRow)> = rows.into_iter().collect();
    merged.sort_by(|(left_keys, _), (right_keys, _)| {
        let left: Vec<_> = left_keys.iter().map(KeyPart::sort_rank).collect();
        let right: Vec<_> = right_keys.iter().map(KeyPart::sort_rank).collect();
        left.cmp(&right)
    });

    build_aggregate_batch(&merged, group_by, metrics)
}

fn canonical_column(column: &ArrayRef) -> Result<ArrayRef, EngineError> {
    match column.data_type() {
        DataType::Int64 => Ok(Arc::clone(column)),
        DataType::Timestamp(_, _) => {
            let micros: Vec<i64> = (0..column.len())
                .map(|row| timestamp_value_us(column, row))
                .collect();
            Ok(Arc::new(arrow::array::Int64Array::from(micros)))
        }
        _ => arrow::compute::cast(column, &DataType::Utf8).map_err(EngineError::from),
    }
}

fn timestamp_value_us(column: &ArrayRef, row: usize) -> i64 {
    match column.data_type() {
        DataType::Timestamp(TimeUnit::Second, _) => {
            column
                .as_primitive::<arrow::datatypes::TimestampSecondType>()
                .value(row)
                * 1_000_000
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            column
                .as_primitive::<arrow::datatypes::TimestampMillisecondType>()
                .value(row)
                * 1_000
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            column
                .as_primitive::<arrow::datatypes::TimestampNanosecondType>()
                .value(row)
                / 1_000
        }
        _ => column
            .as_primitive::<arrow::datatypes::TimestampMicrosecondType>()
            .value(row),
    }
}

fn group_key(columns: &[ArrayRef], row: usize) -> Result<Vec<KeyPart>, EngineError> {
    columns
        .iter()
        .map(|column| match column.data_type() {
            DataType::Int64 => Ok(KeyPart::Micros(
                column.as_primitive::<Int64Type>().value(row),
            )),
            _ => Ok(KeyPart::Text(
                column.as_string::<i32>().value(row).to_string(),
            )),
        })
        .collect()
}

fn build_aggregate_batch(
    rows: &[(Vec<KeyPart>, AggregateRow)],
    group_by: &[GroupDimension],
    metrics: &[Metric],
) -> Result<Vec<RecordBatch>, EngineError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut fields: Vec<Field> = Vec::with_capacity(group_by.len() + metrics.len());
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(fields.capacity());

    for (position, dimension) in group_by.iter().enumerate() {
        match dimension {
            GroupDimension::Bucket => {
                let mut builder = PrimitiveBuilder::<TimestampMicrosecondType>::new();
                for (keys, _) in rows {
                    builder.append_value(match &keys[position] {
                        KeyPart::Micros(micros) => *micros,
                        KeyPart::Text(_) => 0,
                    });
                }
                fields.push(Field::new(
                    "bucket",
                    DataType::Timestamp(TimeUnit::Microsecond, None),
                    false,
                ));
                columns.push(Arc::new(builder.finish()));
            }
            GroupDimension::Level | GroupDimension::Service => {
                let name = match dimension {
                    GroupDimension::Level => "level",
                    GroupDimension::Service => "service",
                    GroupDimension::Bucket => unreachable!(),
                };
                let mut builder = StringBuilder::new();
                for (keys, _) in rows {
                    builder.append_value(match &keys[position] {
                        KeyPart::Text(text) => text.clone(),
                        KeyPart::Micros(_) => String::new(),
                    });
                }
                fields.push(Field::new(name, DataType::Utf8, false));
                columns.push(Arc::new(builder.finish()));
            }
        }
    }

    for metric in metrics {
        let name = match metric {
            Metric::Count => "count",
            Metric::Errors => "errors",
            Metric::Warns => "warns",
            Metric::LastSeen => "last_seen",
        };
        let mut builder = PrimitiveBuilder::<Int64Type>::new();
        for (_, row) in rows {
            builder.append_value(match metric {
                Metric::Count => row.count,
                Metric::Errors => row.errors,
                Metric::Warns => row.warns,
                Metric::LastSeen => row.last_seen,
            });
        }
        fields.push(Field::new(name, DataType::Int64, false));
        columns.push(Arc::new(builder.finish()));
    }

    let batch =
        RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).map_err(EngineError::from)?;
    Ok(vec![batch])
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::TimeZone;

    use super::*;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn partition_days_cover_the_window_plus_one_pad_day() {
        assert_eq!(
            partition_days(fixed_now(), 3600),
            vec![(2026, 3, 14), (2026, 3, 15)]
        );
        assert_eq!(
            partition_days(fixed_now(), 30 * 86_400),
            vec![
                (2026, 2, 12),
                (2026, 2, 13),
                (2026, 2, 14),
                (2026, 2, 15),
                (2026, 2, 16),
                (2026, 2, 17),
                (2026, 2, 18),
                (2026, 2, 19),
                (2026, 2, 20),
                (2026, 2, 21),
                (2026, 2, 22),
                (2026, 2, 23),
                (2026, 2, 24),
                (2026, 2, 25),
                (2026, 2, 26),
                (2026, 2, 27),
                (2026, 2, 28),
                (2026, 3, 1),
                (2026, 3, 2),
                (2026, 3, 3),
                (2026, 3, 4),
                (2026, 3, 5),
                (2026, 3, 6),
                (2026, 3, 7),
                (2026, 3, 8),
                (2026, 3, 9),
                (2026, 3, 10),
                (2026, 3, 11),
                (2026, 3, 12),
                (2026, 3, 13),
                (2026, 3, 14),
                (2026, 3, 15),
            ]
        );
    }

    #[test]
    fn partition_predicate_uses_a_single_fine_grained_clause() {
        let predicate = partition_predicate(&[(2026, 3, 14), (2026, 3, 15)]).unwrap();
        assert_eq!(predicate, "day IN (14, 15)");

        // A window spanning more than 62 distinct days degrades to months.
        let mut wide = Vec::new();
        for day_offset in 0..100 {
            let date = fixed_now() - chrono::Duration::days(day_offset);
            wide.push((date.year(), date.month(), date.day()));
        }
        let degraded = partition_predicate(&wide).unwrap();
        assert!(degraded.starts_with("month IN ("), "got: {degraded}");
    }

    #[test]
    fn split_terms_honours_quoted_phrases() {
        assert_eq!(split_terms("payment failed"), vec!["payment", "failed"]);
        assert_eq!(
            split_terms("\"payment failed\" retry"),
            vec!["payment failed", "retry"]
        );
        assert_eq!(split_terms(""), Vec::<String>::new());
    }

    #[test]
    fn term_condition_translates_fields_and_free_text() {
        assert_eq!(
            term_condition("level:error").as_deref(),
            Some("level = 'ERROR'")
        );
        assert_eq!(
            term_condition("service='auth-api'").as_deref(),
            Some("service = 'auth-api'")
        );
        assert_eq!(
            term_condition("message:\"it's broken\"").as_deref(),
            Some("message ILIKE '%it''s broken%'")
        );
        let free = term_condition("timeout").unwrap();
        assert!(free.contains("message ILIKE '%timeout%'"));
        assert!(free.contains("LOWER(level) = 'timeout'"));
        assert_eq!(term_condition("unknown_field=x"), None);
    }

    #[test]
    fn sql_literal_escapes_single_quotes() {
        assert_eq!(sql_literal("it's"), "'it''s'");
    }

    #[test]
    fn validation_rejects_bad_requests() {
        let base = LogSearch {
            time_range_secs: 3600,
            facets: HashMap::new(),
            search: None,
            mode: SearchMode::Rows {
                limit: 500,
                offset: 0,
            },
        };
        assert!(base.validate(10_000).is_ok());

        let zero_window = LogSearch {
            time_range_secs: 0,
            ..base.clone()
        };
        assert!(zero_window.validate(10_000).is_err());

        let mut bad_facet = base.clone();
        bad_facet.facets.insert("trace_id".into(), "abc".into());
        assert!(bad_facet.validate(10_000).is_err());

        let oversized = LogSearch {
            mode: SearchMode::Rows {
                limit: 10_001,
                offset: 0,
            },
            ..base.clone()
        };
        assert!(oversized.validate(10_000).is_err());

        let duplicate_metrics = LogSearch {
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Level],
                bucket_secs: 60,
                metrics: vec![Metric::Count, Metric::Count],
            },
            ..base.clone()
        };
        assert!(duplicate_metrics.validate(10_000).is_err());

        let no_metrics = LogSearch {
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Service],
                bucket_secs: 60,
                metrics: vec![],
            },
            ..base.clone()
        };
        assert!(no_metrics.validate(10_000).is_err());

        let zero_bucket = LogSearch {
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Bucket],
                bucket_secs: 0,
                metrics: vec![Metric::Count],
            },
            ..base
        };
        assert!(zero_bucket.validate(10_000).is_err());
    }
}
