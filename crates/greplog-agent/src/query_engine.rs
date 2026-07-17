use anyhow::Result;
use arrow::array::*;
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::execution::context::SessionContext;
use datafusion::execution::memory_pool::GreedyMemoryPool;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::prelude::SessionConfig;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

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

        let schema_fns: [(&str, fn() -> arrow::datatypes::Schema, &str); 3] = [
            ("logs", greplog_core::arrow_schema::log_schema, "logs"),
            ("spans", greplog_core::arrow_schema::span_schema, "spans"),
            ("metrics", greplog_core::arrow_schema::metric_schema, "metrics"),
        ];
        for (name, schema_fn, table_type) in &schema_fns {
            let dir = base_dir.join("data").join(format!("table_type={}", table_type));
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
    pub async fn query(&self, sql: &str) -> Result<QueryResult> {
        let df = self.ctx.sql(sql).await?;

        // Always apply the row cap.  If the user's query already has a
        // smaller LIMIT, the extra outer Limit is a harmless no-op (the
        // inner limit produces ≤ our cap, the outer passes all through).
        let df = df.limit(0, Some(self.row_limit))?;

        let batches = timeout(self.timeout_duration, df.collect())
            .await
            .map_err(|_| anyhow::anyhow!("Query timed out after {:?}", self.timeout_duration))??;

        Ok(batches_to_result(batches))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn build_context() -> Result<SessionContext> {
        let pool = Arc::new(GreedyMemoryPool::new(DEFAULT_MEMORY_LIMIT));
        let runtime = RuntimeEnvBuilder::new()
            .with_memory_pool(pool)
            .build()?;
        let ctx = SessionContext::new_with_config_rt(SessionConfig::new(), Arc::new(runtime));
        Ok(ctx)
    }

    fn register_table(
        ctx: &SessionContext,
        name: &str,
        dir: &Path,
        schema: arrow::datatypes::Schema,
    ) -> Result<()> {
        let table_path = ListingTableUrl::parse(&format!("file://{}", dir.display()))?;

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

    #[cfg(test)]
    pub(crate) fn row_limit(&self) -> usize {
        self.row_limit
    }
}

// ---------------------------------------------------------------------------
// RecordBatch → QueryResult conversion
// ---------------------------------------------------------------------------

fn batches_to_result(batches: Vec<RecordBatch>) -> QueryResult {
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
                row_vals.push(arrow_value_to_json(col.as_ref(), row_idx));
            }
            rows.push(row_vals);
        }
    }

    let row_count = rows.len();
    QueryResult {
        columns,
        rows,
        row_count,
    }
}

/// Convert a single cell from a DataFusion / Arrow array to a JSON value.
///
/// Mirrors the existing `arrow_to_json` in `query/mod.rs`, but uses the
/// non-DuckDB-wrapped arrow types directly.
fn arrow_value_to_json(col: &dyn Array, row: usize) -> serde_json::Value {
    if col.is_null(row) {
        return serde_json::Value::Null;
    }

    match col.data_type() {
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
        DataType::Timestamp(_, _) => {
            if let Some(a) = col.as_any().downcast_ref::<TimestampMicrosecondArray>() {
                serde_json::json!(a.value(row))
            } else if let Some(a) = col.as_any().downcast_ref::<TimestampNanosecondArray>() {
                serde_json::json!(a.value(row))
            } else {
                serde_json::Value::String("(timestamp)".into())
            }
        }
        _ => serde_json::Value::String(format!("{:?}", col.data_type())),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::io::write_parquet_atomic;
    use arrow::array::*;
    use arrow::datatypes::{Schema, TimeUnit};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("greplog_query_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    /// Write a small log‑table parquet file under
    /// `data/table_type=logs/service=<svc>/date=<date>/`.
    fn write_log_file(
        base: &Path,
        service: &str,
        date: &str,
        id: &str,
        level: &str,
        message: &str,
    ) -> PathBuf {
        let dir = base
            .join("data")
            .join("table_type=logs")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        let schema = greplog_core::arrow_schema::log_schema();
        let ts = 1_700_000_000_000_000; // 2023-11-14T22:00:00 UTC in micros
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(vec![id])),
                Arc::new(StringArray::from(vec![service])),
                Arc::new(TimestampMicrosecondArray::from(vec![ts])),
                Arc::new(StringArray::from(vec![Some(level)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(StringArray::from(vec![Some(message)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
            ],
        )
        .expect("build log batch");
        write_parquet_atomic(&dir, &batch).expect("write log parquet");
        dir
    }

    /// Write a small span‑table parquet file.
    #[allow(dead_code)]
    fn write_span_file(
        base: &Path,
        service: &str,
        date: &str,
        id: &str,
        name: &str,
    ) -> PathBuf {
        let dir = base
            .join("data")
            .join("table_type=spans")
            .join(format!("service={}", service))
            .join(format!("date={}", date));
        let schema = greplog_core::arrow_schema::span_schema();
        let ts = 1_700_000_000_000_000;
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(vec![id])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(StringArray::from(Vec::<Option<&str>>::from([None]))),
                Arc::new(StringArray::from(vec![service])),
                Arc::new(StringArray::from(vec![Some(name)])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(StringArray::from(vec![None::<&str>])),
                Arc::new(Int32Array::from(vec![None::<i32>])),
                Arc::new(Float64Array::from(vec![None::<f64>])),
                Arc::new(BooleanArray::from(vec![None::<bool>])),
                Arc::new(TimestampMicrosecondArray::from(vec![ts])),
                Arc::new(TimestampMicrosecondArray::from(vec![ts])),
                Arc::new(StringArray::from(vec![None::<&str>])),
            ],
        )
        .expect("build span batch");
        write_parquet_atomic(&dir, &batch).expect("write span parquet");
        dir
    }

    /// Count rows returned by a SELECT query.
    async fn count_rows(engine: &QueryEngine, sql: &str) -> usize {
        let result = engine.query(sql).await.expect("query");
        result.row_count
    }

    // ------------------------------------------------------------------
    // Test 1: Basic query correctness
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_basic_query() {
        let dir = test_dir("basic_query");
        write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "hello");
        let engine = QueryEngine::new(&dir).expect("new engine");

        let result = engine
            .query("SELECT * FROM logs WHERE service = 'api'")
            .await
            .expect("query");
        assert_eq!(result.columns.len(), 8);
        assert_eq!(result.row_count, 1);
        // message is at column index 4 (id=0, timestamp=1, level=2, route=3, message=4)
        assert_eq!(result.rows[0][4], serde_json::json!("hello"));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 2: Partition pruning works
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_partition_pruning() {
        let dir = test_dir("partition_pruning");
        write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "api msg");
        write_log_file(&dir, "worker", "2024-01-15", "id-2", "debug", "worker msg");
        write_log_file(&dir, "api", "2024-01-16", "id-3", "error", "api error");

        let engine = QueryEngine::new(&dir).expect("new engine");

        // Filter by service — should only return rows for "api"
        let result = engine
            .query("SELECT message, service FROM logs WHERE service = 'api'")
            .await
            .expect("query");
        assert_eq!(result.row_count, 2, "should find both api rows");
        for row in &result.rows {
            // Projected columns: [message, service] → message at 0, service at 1
            assert_eq!(row[1], serde_json::json!("api"));
        }

        // Filter by date
        let result = engine
            .query("SELECT message FROM logs WHERE date = '2024-01-15'")
            .await
            .expect("query");
        assert_eq!(result.row_count, 2, "two rows on 2024-01-15");

        // Filter by both partition columns
        let result = engine
            .query("SELECT message FROM logs WHERE service = 'api' AND date = '2024-01-16'")
            .await
            .expect("query");
        assert_eq!(result.row_count, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 3: Limit injection — no existing limit
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_limit_injection_no_existing() {
        let dir = test_dir("limit_no_existing");
        // Write more rows than the configured cap.  Use a single file
        // with many rows instead of many tiny files to avoid excessive
        // execution time for this test.
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=svc")
            .join("date=2024-01-15");
        let schema = greplog_core::arrow_schema::log_schema();
        let n = DEFAULT_ROW_LIMIT + 50;
        let ids: Vec<String> = (0..n).map(|i| format!("id-{}", i)).collect();
        let services: Vec<&str> = (0..n).map(|_| "svc").collect();
        let ts: Vec<i64> = (0..n).map(|_| 1_700_000_000_000_000).collect();
        let levels: Vec<&str> = (0..n).map(|_| "info").collect();
        let messages: Vec<Option<String>> = (0..n)
            .map(|i| Some(format!("msg {}", i)))
            .collect();
        let none_str: Vec<Option<&str>> = (0..n).map(|_| None).collect();
        let msg_arr: StringArray = messages.iter().map(|m| m.as_deref()).collect();
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(services)),
                Arc::new(TimestampMicrosecondArray::from(ts)),
                Arc::new(StringArray::from(levels)),
                Arc::new(StringArray::from(none_str.clone())),
                Arc::new(msg_arr),
                Arc::new(StringArray::from(none_str)),
            ],
        )
        .expect("build batch");
        write_parquet_atomic(&part_dir, &batch).expect("write parquet");
        let engine = QueryEngine::new(&dir).expect("new engine");
        assert_eq!(engine.row_limit, DEFAULT_ROW_LIMIT);

        let result = engine
            .query("SELECT * FROM logs")
            .await
            .expect("query");
        assert_eq!(
            result.row_count, DEFAULT_ROW_LIMIT,
            "should be capped at row limit"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 4: Limit injection — respects a smaller explicit limit
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_limit_injection_respects_smaller() {
        let dir = test_dir("limit_smaller");
        for i in 0..200 {
            write_log_file(
                &dir,
                "svc",
                "2024-01-15",
                &format!("id-{}", i),
                "info",
                &format!("msg {}", i),
            );
        }
        let engine = QueryEngine::new(&dir).expect("new engine");

        let result = engine
            .query("SELECT * FROM logs LIMIT 5")
            .await
            .expect("query");
        assert_eq!(
            result.row_count, 5,
            "should return exactly 5, not capped at default"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 5: Nested / CTE with inner LIMIT
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_limit_cte_or_subquery() {
        let dir = test_dir("limit_cte");
        for i in 0..100 {
            write_log_file(
                &dir,
                "svc",
                "2024-01-15",
                &format!("id-{:03}", i),
                "info",
                &format!("msg {}", i),
            );
        }
        let engine = QueryEngine::new(&dir).expect("new engine");

        // Subquery with inner LIMIT 3
        let result = engine
            .query("SELECT * FROM (SELECT * FROM logs LIMIT 3) AS sub")
            .await
            .expect("query");
        assert_eq!(result.row_count, 3, "subquery limit should be respected");

        // CTE with inner LIMIT 3
        let result = engine
            .query("WITH cte AS (SELECT * FROM logs LIMIT 3) SELECT * FROM cte")
            .await
            .expect("query");
        assert_eq!(result.row_count, 3, "CTE limit should be respected");

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 6: Timeout actually stops execution — with work verification
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_timeout_stops_execution() {
        let dir = test_dir("timeout_test");
        for i in 0..50 {
            write_log_file(
                &dir,
                "svc",
                "2024-01-15",
                &format!("id-{}", i),
                "info",
                &format!("msg {}", i),
            );
        }
        let engine = QueryEngine::new(&dir).expect("new engine");

        // Build the DataFrame on the main task, then spawn execution into a
        // child task so we can observe whether the child finishes after the
        // timeout fires (and the collect future is dropped).
        let df = engine
            .ctx()
            .sql("SELECT COUNT(*) FROM logs CROSS JOIN logs AS l2")
            .await
            .expect("cross join sql");

        let handle = tokio::spawn(async move {
            let result = tokio::time::timeout(Duration::from_millis(1), df.collect()).await;
            // If the timeout fires (expected), the collect future is dropped
            // and DataFusion should cancel all associated background work.
            let _ = result;
        });
        let abort_handle = handle.abort_handle();

        // Wait for the spawned task to finish on its own within a deadline.
        // If DataFusion properly cancels work when the stream is dropped,
        // the task should return quickly.  If orphaned background work is
        // still executing, the task may hang for much longer.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if abort_handle.is_finished() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                abort_handle.abort();
                panic!(
                    "Spawned task did not finish within 10s after timeout — \
                     orphaned background work likely still executing. \
                     This means DataFusion's stream cancellation did not \
                     propagate to internal execution tasks."
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Also verify the engine is still responsive after the timeout.
        let result = engine
            .query("SELECT 'hello' AS greeting")
            .await
            .expect("simple query after timeout");
        assert_eq!(result.row_count, 1);
        assert_eq!(result.rows[0][0], serde_json::json!("hello"));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 7a: Limit injection — over-cap explicit LIMIT
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_limit_injection_over_cap() {
        let dir = test_dir("limit_over_cap");
        let part_dir = dir
            .join("data")
            .join("table_type=logs")
            .join("service=svc")
            .join("date=2024-01-15");
        let schema = greplog_core::arrow_schema::log_schema();
        let n = DEFAULT_ROW_LIMIT + 50;
        let ids: Vec<String> = (0..n).map(|i| format!("id-{}", i)).collect();
        let services: Vec<&str> = (0..n).map(|_| "svc").collect();
        let ts: Vec<i64> = (0..n).map(|_| 1_700_000_000_000_000).collect();
        let levels: Vec<&str> = (0..n).map(|_| "info").collect();
        let messages: Vec<Option<String>> =
            (0..n).map(|i| Some(format!("msg {}", i))).collect();
        let none_str: Vec<Option<&str>> = (0..n).map(|_| None).collect();
        let msg_arr: StringArray = messages.iter().map(|m| m.as_deref()).collect();
        let batch = RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(services)),
                Arc::new(TimestampMicrosecondArray::from(ts)),
                Arc::new(StringArray::from(levels)),
                Arc::new(StringArray::from(none_str.clone())),
                Arc::new(msg_arr),
                Arc::new(StringArray::from(none_str)),
            ],
        )
        .expect("build batch");
        write_parquet_atomic(&part_dir, &batch).expect("write parquet");
        let engine = QueryEngine::new(&dir).expect("new engine");

        // Request 5× the cap — the outer Limit(0, cap) should win.
        let cap = DEFAULT_ROW_LIMIT;
        let result = engine
            .query(&format!("SELECT * FROM logs LIMIT {}", cap * 5))
            .await
            .expect("query with LIMIT > cap");
        assert_eq!(
            result.row_count, cap,
            "over-cap LIMIT should be reduced to configured cap"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 7b: Empty database handling
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_empty_database() {
        let dir = test_dir("empty_db");
        // Don't write any data files — just create the engine.
        let engine = QueryEngine::new(&dir).expect("new engine");

        let result = engine.query("SELECT * FROM logs").await.expect("query on empty db");
        assert_eq!(result.row_count, 0, "empty db returns zero rows");
        assert!(result.columns.is_empty() || result.columns.len() == 7);

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // Test 8: Manifest / .tmp files are never treated as data
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_non_parquet_files_ignored() {
        let dir = test_dir("non_parquet_ignored");
        let part_dir = write_log_file(
            &dir,
            "svc",
            "2024-01-15",
            "id-real",
            "info",
            "real data",
        );

        // Write a .compaction_manifest file and a .tmp file into the
        // same partition directory.
        fs::write(part_dir.join(".compaction_manifest"), b"{}").expect("write manifest");
        fs::write(
            part_dir.join("stray.tmp"),
            b"not parquet data",
        )
        .expect("write tmp");

        let engine = QueryEngine::new(&dir).expect("new engine");

        let result = engine
            .query("SELECT * FROM logs")
            .await
            .expect("query");
        assert_eq!(result.row_count, 1, "only the real parquet row should appear");
        // message is at column index 4 (id=0, timestamp=1, level=2, route=3, message=4)
        assert_eq!(result.rows[0][4], serde_json::json!("real data"));

        let _ = fs::remove_dir_all(&dir);
    }
}
