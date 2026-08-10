//! The DataFusion-backed SQL query engine.
//!
//! [`QueryEngine`] exposes a unified `logs` view over two tiers:
//! - `parquet_logs`, the Hive-partitioned Parquet tree on disk, and
//! - `live_logs`, a [`TableProvider`] that snapshots the shared [`LiveBuffer`]
//!   on every scan.
//!
//! A SQL view `UNION ALL`s the two, bridging the schema gap: the Parquet table
//! carries injected `year`/`month`/`day`/`service` partition columns, which the
//! live branch recomputes from `timestamp_us` on the fly.

use std::any::Any;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::{DataType, SchemaRef};
use async_trait::async_trait;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::ListingOptions;
use datafusion::datasource::{MemTable, TableProvider};
use datafusion::error::DataFusionError;
use datafusion::execution::session_state::SessionState;
use datafusion::logical_expr::{Expr, TableType};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::prelude::SessionContext;

use crate::config::{EngineConfig, LiveBuffer};
use crate::error::EngineError;
use crate::schema::greplog_schema;

/// SQL fused once at init into the `logs` view over both tiers.
///
/// Column names follow [`greplog_schema`]; the live branch casts its
/// dictionary-encoded `service` column to `Utf8` to match the partition column
/// and derives the date partition columns from `timestamp_us`.
const UNIFIED_VIEW_SQL: &str = "\
CREATE VIEW logs AS \
SELECT timestamp_us, trace_id, level, service, message, raw_body, year, month, day \
FROM parquet_logs \
UNION ALL \
SELECT timestamp_us, trace_id, level, arrow_cast(service, 'Utf8'), message, raw_body, \
       arrow_cast(EXTRACT(year FROM timestamp_us), 'UInt16') AS year, \
       arrow_cast(EXTRACT(month FROM timestamp_us), 'UInt8') AS month, \
       arrow_cast(EXTRACT(day FROM timestamp_us), 'UInt8') AS day \
FROM live_logs";

/// Executes SQL over the unified live + Parquet `logs` view.
pub struct QueryEngine {
    ctx: SessionContext,
}

impl QueryEngine {
    /// Builds a query engine over `config.data_dir` and the shared `live_buffer`.
    ///
    /// The Parquet tree registers as `parquet_logs`, the shared buffer registers
    /// as `live_logs`, and the unified `logs` view is created on top.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DataFusion`] if registration or view creation
    /// fails, or [`EngineError::ParseError`] if `data_dir` is not valid UTF-8.
    pub async fn new(config: &EngineConfig, live_buffer: LiveBuffer) -> Result<Self, EngineError> {
        let ctx = SessionContext::new();
        register_parquet_table(&ctx, config).await?;
        register_live_table(&ctx, live_buffer, greplog_schema())?;
        ctx.sql(UNIFIED_VIEW_SQL).await?;
        Ok(Self { ctx })
    }

    /// Runs `sql` against the unified view.
    ///
    /// The `live_logs` provider snapshots the shared [`LiveBuffer`] during each
    /// scan under a brief read lock — cloning only the Arrow array reference
    /// counts, never row data — so the query always sees the latest rows and is
    /// never blocked on Parquet disk I/O.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DataFusion`] when the SQL is invalid or fails to
    /// execute, or [`EngineError::LockError`] if the buffer lock is poisoned.
    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<RecordBatch>, EngineError> {
        let batches = self.ctx.sql(sql).await?.collect().await?;
        Ok(batches)
    }
}

/// Registers `config.data_dir` as a partitioned listing table named `parquet_logs`.
///
/// The `ParquetFormat` reads each chunk; the partition columns map the
/// `key=value` folder names onto columns so directory pruning is possible.
async fn register_parquet_table(
    ctx: &SessionContext,
    config: &EngineConfig,
) -> Result<(), EngineError> {
    let partition_cols = vec![
        (String::from("year"), DataType::UInt16),
        (String::from("month"), DataType::UInt8),
        (String::from("day"), DataType::UInt8),
        (String::from("service"), DataType::Utf8),
    ];

    let options = ListingOptions::new(Arc::new(ParquetFormat::default()))
        .with_table_partition_cols(partition_cols);

    let path = config
        .data_dir
        .to_str()
        .ok_or_else(|| crate::error::parse_error("data_dir is not valid UTF-8"))?;

    ctx.register_listing_table("parquet_logs", path, options, None, None).await?;
    Ok(())
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
/// back fresh rows itself. Just-in-time, each scan clones the buffered batch
/// vector under a brief read lock — only bumping Arrow reference counts — and
/// serves it through an in-memory [`MemTable`]. The view therefore always sees
/// the latest rows, and the lock is dropped before any data is consumed.
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