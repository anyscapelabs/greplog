//! The DataFusion-backed SQL query engine.
//!
//! [`QueryEngine`] registers the Hive-partitioned Parquet tree on disk as a
//! queryable `logs` table and runs arbitrary SQL against it. The partition
//! folders (`year`, `month`, `day`, `service`) are exposed as real columns so
//! the optimizer can prune files that cannot satisfy a filter.

use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::DataType;
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::ListingOptions;
use datafusion::prelude::SessionContext;

use crate::config::EngineConfig;
use crate::error::EngineError;

/// Executes SQL over the persisted Parquet partition tree.
pub struct QueryEngine {
    ctx: SessionContext,
}

impl QueryEngine {
    /// Builds a query engine and registers `config.data_dir` as the `logs` table.
    ///
    /// Every `year=YYYY/`, `month=MM/`, `day=DD/`, and `service=<name>/` folder
    /// level in the tree becomes a queryable column of the matching type, so
    /// filter predicates can prune whole directories.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DataFusion`] if the directory cannot be registered
    /// or its files read, or [`EngineError::ParseError`] if `data_dir` is not
    /// valid UTF-8.
    pub async fn new(config: &EngineConfig) -> Result<Self, EngineError> {
        let ctx = SessionContext::new();
        register_logs_table(&ctx, config).await?;
        Ok(Self { ctx })
    }

    /// Runs `sql` against the registered tables and returns every produced batch.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DataFusion`] when the SQL is invalid or the query
    /// fails to execute.
    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<RecordBatch>, EngineError> {
        let batches = self.ctx.sql(sql).await?.collect().await?;
        Ok(batches)
    }
}

/// Registers `config.data_dir` as a partitioned listing table named `logs`.
///
/// The `ParquetFormat` reads each chunk; the partition columns map the
/// `key=value` folder names onto columns so directory pruning is possible.
async fn register_logs_table(
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

    ctx.register_listing_table("logs", path, options, None, None).await?;
    Ok(())
}