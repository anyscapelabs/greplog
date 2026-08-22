//! The DataFusion-backed SQL query engine over live + Parquet tiers.

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::RecordBatch;
use arrow::datatypes::{DataType, SchemaRef};
use async_trait::async_trait;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl};
use datafusion::datasource::{MemTable, TableProvider};
use datafusion::error::DataFusionError;
use datafusion::execution::session_state::SessionState;
use datafusion::logical_expr::{Expr, TableType};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::prelude::SessionContext;
use datafusion::sql::sqlparser::ast::Statement;
use datafusion::sql::sqlparser::dialect::GenericDialect;
use datafusion::sql::sqlparser::parser::Parser as SqlParser;
use tokio::runtime::Runtime;

use crate::config::{EngineConfig, LiveBuffer};
use crate::error::EngineError;
use crate::schema::greplog_schema;

/// The unified `logs` view over both tiers. The live branch casts its
/// dictionary-encoded `service` to `Utf8` and derives the partition columns
/// from `timestamp_us` to match the Parquet branch.
const UNIFIED_VIEW_SQL: &str = "\
CREATE VIEW logs AS \
SELECT timestamp_us, trace_id, level, service, message, raw_body, year, month, day \
FROM parquet_logs \
UNION ALL \
SELECT timestamp_us, trace_id, level, CAST(service AS VARCHAR), message, raw_body, \
       CAST(EXTRACT(year FROM timestamp_us) AS INT) AS year, \
       CAST(EXTRACT(month FROM timestamp_us) AS INT) AS month, \
       CAST(EXTRACT(day FROM timestamp_us) AS INT) AS day \
FROM live_logs";

/// Executes SQL over the unified live + Parquet `logs` view.
pub struct QueryEngine {
    pub(crate) ctx: SessionContext,
    query_timeout_secs: u64,
    pub(crate) max_query_rows: usize,
    /// Dedicated runtime for query execution, reused across calls.
    ///
    /// `DataFusion` 39 offers no per-query cancellation: aborting `collect()`
    /// leaves its internally-spawned tasks running. Running each query on its
    /// own runtime, torn down via `shutdown_timeout(0)` on deadline, is the
    /// only way to stop a runaway query. A fresh runtime is rebuilt lazily
    /// after a timeout; under concurrency the idle one is retired on a
    /// dedicated thread instead of being dropped in the async context (see
    /// [`QueryEngine::execute_sql`]).
    query_rt: tokio::sync::Mutex<Option<Runtime>>,
}

impl QueryEngine {
    /// Builds a query engine over `config.data_dir` and the shared `live_buffer`.
    pub async fn new(config: &EngineConfig, live_buffer: LiveBuffer) -> Result<Self, EngineError> {
        let ctx = SessionContext::new();
        register_parquet_table(&ctx, config)?;
        register_live_table(&ctx, live_buffer, greplog_schema())?;
        ctx.sql(UNIFIED_VIEW_SQL).await?;
        Ok(Self {
            ctx,
            query_timeout_secs: config.query_timeout_secs,
            max_query_rows: config.max_query_rows,
            query_rt: tokio::sync::Mutex::new(None),
        })
    }

    /// Runs `sql` against the unified view.
    ///
    /// Only a single read-only `SELECT` is accepted; results are capped at
    /// `max_query_rows` and cancelled after `query_timeout_secs`. The
    /// `live_logs` provider snapshots the shared buffer per scan under a brief
    /// read lock (Arrow reference counts only), so queries always see the
    /// latest rows without blocking on disk I/O.
    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<RecordBatch>, EngineError> {
        validate_read_only_query(sql)?;

        let df = self.ctx.sql(sql).await?;
        let df = df.limit(0, Some(self.max_query_rows))?;

        let timeout = Duration::from_secs(self.query_timeout_secs.max(1));
        let query_rt = {
            let mut slot = self.query_rt.lock().await;
            match slot.take() {
                Some(rt) => rt,
                None => build_query_runtime()?,
            }
        };
        // Guarded so a cancellation of this future never drops the runtime
        // inside this async context, which Tokio rejects with a panic.
        let query_rt = RuntimeDropGuard::new(query_rt);

        let handle = query_rt.as_runtime().spawn(df.collect());
        let result = tokio::time::timeout(timeout, handle).await;
        let batches = match result {
            Ok(Ok(Ok(batches))) => Ok(batches),
            Ok(Ok(Err(e))) => Err(EngineError::DataFusion(e)),
            Ok(Err(_)) => Err(EngineError::DataFusion(DataFusionError::Execution(
                "query task aborted".to_string(),
            ))),
            Err(_) => {
                // Force-cancel every task still running on the runtime,
                // including DataFusion's internally-spawned ones. Dropping a
                // runtime blocks, so hand teardown to a dedicated thread and
                // leave the slot empty for the next call to rebuild.
                std::thread::spawn(move || {
                    query_rt.take().shutdown_timeout(Duration::from_millis(0));
                });
                return Err(EngineError::QueryTimeout);
            }
        }?;

        // Park the runtime for reuse — unless a concurrent query already put
        // one there. Overwriting would drop it here inside an async context;
        // retire the idle one on a dedicated thread instead.
        let idle = query_rt.take();
        let mut slot = self.query_rt.lock().await;
        if slot.is_none() {
            *slot = Some(idle);
        } else {
            std::thread::spawn(move || drop(idle));
        }

        Ok(batches)
    }
}

impl Drop for QueryEngine {
    fn drop(&mut self) {
        // A `Runtime` must be torn down on a thread where blocking is allowed;
        // dropping one inside an async context panics. Offload it.
        if let Some(rt) = self.query_rt.get_mut().take() {
            std::thread::spawn(move || drop(rt));
        }
    }
}

/// Builds the dedicated runtime used to execute a single query.
fn build_query_runtime() -> Result<Runtime, EngineError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| EngineError::IoError(std::io::Error::other(e.to_string())))
}

/// Owns a [`Runtime`] and guarantees it is never dropped on a thread where
/// blocking is disallowed.
///
/// Tokio panics — "Cannot drop a runtime in a context where blocking is not
/// allowed" — if a [`Runtime`] is dropped from within an asynchronous context.
/// A query can be torn down by the HTTP layer at any moment, which drops the
/// in-flight future and with it any [`Runtime`] held in a local variable; this
/// parks that drop on a fresh, non-async OS thread instead.
struct RuntimeDropGuard {
    inner: Option<Runtime>,
}

impl RuntimeDropGuard {
    fn new(runtime: Runtime) -> Self {
        Self { inner: Some(runtime) }
    }

    fn as_runtime(&self) -> &Runtime {
        self.inner.as_ref().expect("runtime already taken")
    }

    fn take(mut self) -> Runtime {
        self.inner.take().expect("runtime already taken")
    }
}

impl Drop for RuntimeDropGuard {
    fn drop(&mut self) {
        if let Some(runtime) = self.inner.take() {
            // Offload the blocking teardown to a fresh OS thread so the
            // runtime is never dropped inside an asynchronous context.
            std::thread::spawn(move || drop(runtime));
        }
    }
}

/// Rejects any statement that is not a plain, read-only `SELECT` query.
/// `WITH` queries parse as a [`Statement::Query`] and pass.
fn validate_read_only_query(sql: &str) -> Result<(), EngineError> {
    let statements = SqlParser::parse_sql(&GenericDialect {}, sql)
        .map_err(|e| EngineError::QueryRejected(format!("invalid SQL: {e}")))?;

    match statements.as_slice() {
        [Statement::Query(_)] => Ok(()),
        [other] => Err(EngineError::QueryRejected(format!(
            "only read-only SELECT queries are allowed, got: {other}"
        ))),
        _ => Err(EngineError::QueryRejected(
            "only a single SELECT query is allowed per request".to_string(),
        )),
    }
}

/// Registers `config.data_dir` as a partitioned listing table named
/// `parquet_logs`. The directory is re-listed on every scan, so data that
/// arrives after startup is visible without a restart. An explicit file schema
/// (data fields only; partition columns are merged in by [`ListingTable`])
/// keeps the table well-typed even before any Parquet file exists.
fn register_parquet_table(
    ctx: &SessionContext,
    config: &EngineConfig,
) -> Result<(), EngineError> {
    std::fs::create_dir_all(&config.data_dir)?;
    let partition_cols = vec![
        (String::from("year"), DataType::Int32),
        (String::from("month"), DataType::Int32),
        (String::from("day"), DataType::Int32),
        (String::from("service"), DataType::Utf8),
    ];

    let options = ListingOptions::new(Arc::new(ParquetFormat::default()))
        .with_table_partition_cols(partition_cols)
        // Without this, in-progress chunk writes (`*.parquet.part`) are read
        // as parquet and fail to parse as intermittent scan errors.
        .with_file_extension(".parquet");

    let path = config
        .data_dir
        .to_str()
        .ok_or_else(|| crate::error::parse_error("data_dir is not valid UTF-8"))?;

    let url = ListingTableUrl::parse(path)?;
    let config = ListingTableConfig::new(url)
        .with_listing_options(options)
        .with_schema(parquet_file_schema());
    let table = ListingTable::try_new(config)?;
    ctx.register_table("parquet_logs", Arc::new(table))?;
    Ok(())
}

/// Schema of the data fields written to a Parquet chunk: [`greplog_schema`]
/// without `service`, which lives only in the folder name.
fn parquet_file_schema() -> SchemaRef {
    Arc::new(arrow::datatypes::Schema::new(
        greplog_schema()
            .fields()
            .iter()
            .filter(|field| field.name() != "service")
            .map(|field| field.as_ref().clone())
            .collect::<Vec<_>>(),
    ))
}

/// Registers an always-live provider named `live_logs` over the shared buffer.
fn register_live_table(
    ctx: &SessionContext,
    live_buffer: LiveBuffer,
    schema: SchemaRef,
) -> Result<(), EngineError> {
    let live = LiveBufferTable::new(live_buffer, schema);
    ctx.register_table("live_logs", Arc::new(live))?;
    Ok(())
}

/// A [`TableProvider`] that snapshots the shared [`LiveBuffer`] on every scan.
///
/// `DataFusion` bakes a view's leaf providers into the stored logical plan, so
/// the provider registered as `live_logs` must stay a single object and hand
/// back fresh rows itself. Each scan clones the buffered batch vector under a
/// brief read lock (only Arrow reference counts) and serves it through an
/// in-memory [`MemTable`].
struct LiveBufferTable {
    buffer: LiveBuffer,
    schema: SchemaRef,
}

impl LiveBufferTable {
    fn new(buffer: LiveBuffer, schema: SchemaRef) -> Self {
        Self { buffer, schema }
    }
}

#[async_trait]
impl TableProvider for LiveBufferTable {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &SessionState,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        let batches = {
            let buffer = self
                .buffer
                .read()
                .map_err(|_| DataFusionError::Execution("live buffer read lock poisoned".into()))?;
            buffer.clone()
        };
        let live = MemTable::try_new(Arc::clone(&self.schema), vec![batches])?;
        live.scan(state, projection, filters, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeDropGuard;
    use crate::error::EngineError;
    use std::time::Duration;

    fn build() -> Result<tokio::runtime::Runtime, EngineError> {
        super::build_query_runtime()
    }

    #[test]
    fn cancelled_future_must_not_drop_runtime_in_async_context() {
        let outer = tokio::runtime::Runtime::new().expect("build outer runtime");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            outer.block_on(async {
                let inner = build().expect("build query runtime");
                let guard = RuntimeDropGuard::new(inner);
                let handle = guard.as_runtime().spawn(async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                });
                tokio::select! {
                    _ = handle => {}
                    () = tokio::time::sleep(Duration::from_millis(20)) => {}
                }
                // Guard is dropped here, still inside the async context. Prior
                // to the guard this panicked with "Cannot drop a runtime in a
                // context where blocking is not allowed"; now the teardown is
                // parked on a fresh OS thread.
                drop(guard);
            });
        }));
        assert!(result.is_ok(), "dropping the runtime in an async context must not panic");
    }
}