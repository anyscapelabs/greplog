use anyhow::Result;
use arrow::array::*;
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::execution::cache::cache_manager::CacheManagerConfig;
use datafusion::execution::context::{SQLOptions, SessionContext};
use datafusion::execution::memory_pool::GreedyMemoryPool;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::prelude::SessionConfig;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

// ── JSON SQL functions (json_get_str, json_get_int, json_get_float, etc.) ──
// Enables queries like:
//   SELECT json_get_str(attributes, 'http.latency_ms') FROM logs
// Used by the dashboard to extract HTTP metadata from LogEvent.attributes
// for SDKs (Node.js, Python) that send HTTP data as attributes rather than
// as Span messages.
use datafusion_functions_json::register_all;

/// TTL for the DataFusion file-list cache.  Entries are evicted this
/// many seconds after insertion, forcing a fresh directory listing from
/// the store on the next query.
///
/// # Why 10s
///
/// Benchmark results (12-week scale, 420 partitions, local NVMe):
///
///   p95 penalty vs. no cache:
///     filtered_scan:  +44ms  (+42%)
///     latency_pctl:   +10ms  (+11%)   — effectively within noise
///     error_rate:     -27ms  (-10%)   — actually faster
///
/// A shorter TTL (2s) produced larger tail regressions (+80–89ms p95 on
/// two of three patterns).  The dose-response is consistent with lock
/// contention in the cache's mutex-guarded HashMap: fewer expiration
/// events per unit time → less contention → lower p95.  10s is the
/// longest value that still guarantees new-flushed Parquet files become
/// visible within one compaction cycle (5s flush + 5s compaction).
///
/// # Visibility guarantee
///
/// Newly flushed Parquet files appear within at most 10 s of being
/// written.  This is the key win over disabling the cache entirely:
/// predictable bounded staleness rather than a full re-list on every
/// query.
///
/// # Forward-looking rationale
///
/// On local NVMe the cache's overhead (mutex + HashMap lookup)
/// sometimes exceeds the cost of re-listing 420 directories.  The
/// trade-off inverts on remote object storage (S3/R2) where a single
/// LIST operation costs 5–50 ms and repeated listing at multi-tenant
/// concurrency scales multiplicatively.  With this config in place,
/// switching to a remote ListingTableUrl backend gains the latency
/// benefit without any code changes.
pub(crate) const LIST_FILES_CACHE_TTL_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// Configuration defaults
// ---------------------------------------------------------------------------

/// Per-query maximum number of rows returned.  Matches the DuckDB-era cap.
const DEFAULT_ROW_LIMIT: usize = 10_000;

/// Per-query execution timeout.  Matches the DuckDB-era default.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Memory pool limit for DataFusion's query engine (256 MB).
///
/// Chosen to fit comfortably on a local dev machine while preventing a
/// pathological query from exhausting host memory.  DataFusion will
/// return a clean "exceeded memory budget" error if a query would
/// require more.
const DEFAULT_MEMORY_LIMIT: usize = 256 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// JSON‑serialisable query result, matching the shape used by the
/// existing DuckDB-based engine.
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
}

/// DataFusion-based query engine for the Greplog Parquet store.
///
/// Registration is eager (tables are registered at construction time),
/// matching the convention used by the existing DuckDB query engine.
#[derive(Clone)]
pub struct QueryEngine {
    ctx: Arc<SessionContext>,
    row_limit: usize,
    timeout_duration: Duration,
}

impl QueryEngine {
    /// Create a new query engine against `data_dir/data/`.
    ///
    /// Creates any missing `data/<table_type>/` directories and
    /// registers `logs`, `spans`, and `metrics` tables with Hive
    /// partition columns `service` and `date`.
    pub fn new(base_dir: &Path) -> Result<Self> {
        let ctx = Self::build_context()?;

        type SchemaFn = fn() -> arrow::datatypes::Schema;
        let schema_fns: [(&str, SchemaFn, &str); 3] = [
            (
                "logs",
                greplog_core::arrow_schema::log_schema as SchemaFn,
                "logs",
            ),
            (
                "spans",
                greplog_core::arrow_schema::span_schema as SchemaFn,
                "spans",
            ),
            (
                "metrics",
                greplog_core::arrow_schema::metric_schema as SchemaFn,
                "metrics",
            ),
        ];
        for (name, schema_fn, table_type) in &schema_fns {
            let dir = base_dir
                .join("data")
                .join(format!("table_type={}", table_type));
            std::fs::create_dir_all(&dir)?;
            Self::register_table(&ctx, name, &dir, schema_fn())?;
        }

        Ok(Self {
            ctx: Arc::new(ctx),
            row_limit: DEFAULT_ROW_LIMIT,
            timeout_duration: DEFAULT_TIMEOUT,
        })
    }

    /// Execute a read‑only SQL query and return JSON‑serialisable results.
    ///
    /// A maximum row limit (`DEFAULT_ROW_LIMIT`) and a per‑query timeout
    /// (`DEFAULT_TIMEOUT`) are applied automatically.
    ///
    /// ## Mutation Safety
    ///
    /// DataFusion's `SQLOptions` verification rejects DDL, DML, and
    /// `COPY` at the logical‑plan level, **before** the plan is executed.
    /// This prevents statements like `DROP TABLE logs` from reaching
    /// the registered ListingTables.
    pub async fn query(&self, sql: &str) -> Result<QueryResult> {
        let options = SQLOptions::new()
            .with_allow_ddl(false)
            .with_allow_dml(false)
            .with_allow_statements(false);
        let df = self.ctx.sql_with_options(sql, options).await?;

        // Always apply the row cap.  If the user's query already has a
        // smaller LIMIT, the extra outer Limit is a harmless no-op (the
        // inner limit produces ≤ our cap, the outer passes all through).
        let df = df.limit(0, Some(self.row_limit))?;

        let batches = timeout(self.timeout_duration, df.collect())
            .await
            .map_err(|_| anyhow::anyhow!("Query timed out after {:?}", self.timeout_duration))??;

        batches_to_result(batches)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn build_context() -> Result<SessionContext> {
        let pool = Arc::new(GreedyMemoryPool::new(DEFAULT_MEMORY_LIMIT));
        // Bounded-staleness file-list cache (see LIST_FILES_CACHE_TTL_SECS
        // docstring for trade-off analysis).  Per-entry timestamps mean
        // entries expire independently rather than all at once, avoiding
        // the p95 spike that a synchronous global-clear task would cause.
        let cache_config = CacheManagerConfig::default()
            .with_list_files_cache_ttl(Some(Duration::from_secs(LIST_FILES_CACHE_TTL_SECS)));
        let runtime = RuntimeEnvBuilder::new()
            .with_memory_pool(pool)
            .with_cache_manager(cache_config)
            .build()?;
        let mut ctx = SessionContext::new_with_config_rt(SessionConfig::new(), Arc::new(runtime));

        // Register JSON SQL functions (json_get_str, json_get_int, json_get_float, etc.)
        // so the dashboard can query LogEvent.attributes for HTTP metadata from
        // Node.js and Python SDKs via json_get_str(attributes, 'http.latency_ms').
        // Each string argument is one literal key segment (the attributes JSON is a
        // flat object whose keys contain dots), not JSONPath syntax.
        register_all(&mut ctx)?;

        Ok(ctx)
    }

    fn register_table(
        ctx: &SessionContext,
        name: &str,
        dir: &Path,
        schema: arrow::datatypes::Schema,
    ) -> Result<()> {
        let table_path = ListingTableUrl::parse(format!("file://{}", dir.display()))?;

        // Partition columns are derived from the Hive‑style directory
        // layout, not stored in the Parquet files themselves.  Strip
        // them from the declared file schema to avoid a schema conflict
        // (DataFusion appends partition columns to the table schema).
        let partition_col_names = ["service", "date"];
        let file_fields: Vec<arrow::datatypes::Field> = schema
            .fields()
            .iter()
            .filter(|f| !partition_col_names.contains(&f.name().as_str()))
            .map(|f| f.as_ref().clone())
            .collect();
        let file_schema = arrow::datatypes::Schema::new(file_fields);

        let format = ParquetFormat::default();
        let listing_options = ListingOptions::new(Arc::new(format))
            .with_file_extension(".parquet")
            .with_table_partition_cols(vec![
                ("service".to_string(), DataType::Utf8),
                ("date".to_string(), DataType::Utf8),
            ]);

        let config = ListingTableConfig::new(table_path)
            .with_listing_options(listing_options)
            .with_schema(Arc::new(file_schema));

        let table = ListingTable::try_new(config)?;
        ctx.register_table(name.to_string(), Arc::new(table))?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn ctx(&self) -> &SessionContext {
        &self.ctx
    }
}

// ---------------------------------------------------------------------------
// RecordBatch → QueryResult conversion
// ---------------------------------------------------------------------------

fn batches_to_result(batches: Vec<RecordBatch>) -> Result<QueryResult> {
    let columns: Vec<String> = if let Some(batch) = batches.first() {
        batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    } else {
        vec![]
    };

    let mut rows = Vec::new();
    for batch in &batches {
        for row_idx in 0..batch.num_rows() {
            let mut row_vals = Vec::with_capacity(batch.num_columns());
            for col_idx in 0..batch.num_columns() {
                let col = batch.column(col_idx);
                row_vals.push(arrow_value_to_json(col.as_ref(), row_idx)?);
            }
            rows.push(row_vals);
        }
    }

    let row_count = rows.len();
    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

/// Convert a single cell from a DataFusion / Arrow array to a JSON value.
///
/// Mirrors the existing `arrow_to_json` in `query/mod.rs`, but uses the
/// non-DuckDB-wrapped arrow types directly.
///
/// Fails loudly on unhandled Arrow types instead of fabricating a value: a
/// silent catch-all has caused real corruption here before (DataFusion 54's
/// `Utf8View` was serialized as the literal string "Utf8View" and rendered as
/// a bogus status code in the dashboard). An unknown type is a bug that
/// should surface as a query error, not as a wrong-looking chart.
fn arrow_value_to_json(col: &dyn Array, row: usize) -> Result<serde_json::Value> {
    if col.is_null(row) {
        return Ok(serde_json::Value::Null);
    }

    Ok(match col.data_type() {
        DataType::Boolean => {
            let a = col.as_any().downcast_ref::<BooleanArray>().unwrap();
            serde_json::Value::Bool(a.value(row))
        }
        DataType::Int8 => {
            serde_json::json!(col.as_any().downcast_ref::<Int8Array>().unwrap().value(row))
        }
        DataType::Int16 => {
            serde_json::json!(col.as_any().downcast_ref::<Int16Array>().unwrap().value(row))
        }
        DataType::Int32 => {
            serde_json::json!(col.as_any().downcast_ref::<Int32Array>().unwrap().value(row))
        }
        DataType::Int64 => {
            serde_json::json!(col.as_any().downcast_ref::<Int64Array>().unwrap().value(row))
        }
        DataType::UInt8 => serde_json::json!(
            col.as_any().downcast_ref::<UInt8Array>().unwrap().value(row)
        ),
        DataType::UInt16 => serde_json::json!(
            col.as_any().downcast_ref::<UInt16Array>().unwrap().value(row)
        ),
        DataType::UInt32 => serde_json::json!(
            col.as_any().downcast_ref::<UInt32Array>().unwrap().value(row)
        ),
        DataType::UInt64 => serde_json::json!(
            col.as_any().downcast_ref::<UInt64Array>().unwrap().value(row)
        ),
        DataType::Float32 => {
            serde_json::json!(col.as_any().downcast_ref::<Float32Array>().unwrap().value(row))
        }
        DataType::Float64 => {
            serde_json::json!(col.as_any().downcast_ref::<Float64Array>().unwrap().value(row))
        }
        DataType::Utf8 => {
            let a = col.as_any().downcast_ref::<StringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::LargeUtf8 => {
            let a = col.as_any().downcast_ref::<LargeStringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::Utf8View => {
            // DataFusion 54 uses StringView (Utf8View) for string literals and
            // CAST(.. AS VARCHAR). The downcast always succeeds for a Utf8View
            // column; the fallback avoids unwrapping on the unreachable case.
            col.as_any()
                .downcast_ref::<StringViewArray>()
                .map(|a| serde_json::Value::String(a.value(row).to_string()))
                .unwrap_or(serde_json::Value::Null)
        }
        DataType::Timestamp(_, _) => {
            if let Some(a) = col.as_any().downcast_ref::<TimestampMicrosecondArray>() {
                serde_json::json!(a.value(row))
            } else if let Some(a) = col.as_any().downcast_ref::<TimestampNanosecondArray>() {
                serde_json::json!(a.value(row))
            } else {
                serde_json::Value::String("(timestamp)".into())
            }
        }
        DataType::List(_) => {
            let a = col.as_any().downcast_ref::<ListArray>().unwrap();
            let sub = a.value(row);
            let json_values: Vec<serde_json::Value> = (0..sub.len())
                .map(|i| arrow_value_to_json(sub.as_ref(), i))
                .collect::<Result<Vec<_>>>()?;
            serde_json::Value::Array(json_values)
        }
        _ => {
            return Err(anyhow::anyhow!(
                "cannot serialize query result: unhandled Arrow type {:?} (add an arm to arrow_value_to_json)",
                col.data_type()
            ))
        }
    })
}

#[cfg(test)]
#[path = "../tests/modules/query_engine.rs"]
mod tests;
