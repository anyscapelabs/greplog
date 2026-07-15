use anyhow::{bail, Result};
use duckdb::Connection;
use duckdb::arrow::array::*;
use duckdb::arrow::datatypes::DataType;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct QueryEngine {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

/// SQL keywords that must not appear at the start of a query.
const BLOCKED_PREFIXES: &[&str] = &[
    "insert", "update", "delete", "drop", "alter", "create",
    "truncate", "replace", "merge", "grant", "revoke", "attach",
    "detach", "copy", "load", "install", "pragma", "set",
    "begin", "commit", "rollback", "checkpoint",
];

impl QueryEngine {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open_in_memory()?;

        // Attach the on-disk DB as read-only for query safety
        conn.execute_batch(&format!(
            "ATTACH '{}' AS store (READ_ONLY);",
            db_path.display()
        ))?;

        // Set default schema so queries don't need to prefix table names
        conn.execute_batch("USE store;")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Execute a read-only SQL query and return JSON-serializable results.
    /// Uses DuckDB's Arrow API to avoid the "statement was not executed yet" panic.
    pub fn query(&self, sql: &str) -> Result<QueryResult> {
        Self::validate_query(sql)?;

        let conn = self.conn.lock().map_err(|e| {
            anyhow::anyhow!("Query engine lock poisoned: {}", e)
        })?;

        let mut stmt = conn.prepare(sql)?;
        let batches: Vec<duckdb::arrow::record_batch::RecordBatch> =
            stmt.query_arrow([])?.collect();

        // Extract column names from Arrow schema
        let columns: Vec<String> = if let Some(batch) = batches.first() {
            batch.schema().fields().iter().map(|f| f.name().clone()).collect()
        } else {
            vec![]
        };

        // Convert Arrow record batches to JSON rows
        let mut rows = Vec::new();
        for batch in &batches {
            for row_idx in 0..batch.num_rows() {
                let mut row_vals = Vec::with_capacity(batch.num_columns());
                for col_idx in 0..batch.num_columns() {
                    let col = batch.column(col_idx);
                    row_vals.push(arrow_to_json(col.as_ref(), row_idx));
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

    /// Validates that a query string is a safe read-only operation.
    fn validate_query(sql: &str) -> Result<()> {
        let trimmed = sql.trim();

        // Block multi-statement queries
        if let Some(pos) = trimmed.find(';') {
            let after = trimmed[pos + 1..].trim();
            if !after.is_empty() {
                bail!("Multi-statement queries are not allowed");
            }
        }

        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        for blocked in BLOCKED_PREFIXES {
            if first_word == *blocked {
                bail!("Only SELECT, EXPLAIN, and DESCRIBE queries are allowed");
            }
        }

        if first_word != "select"
            && first_word != "explain"
            && first_word != "describe"
            && first_word != "show"
            && first_word != "with"
        {
            bail!("Only SELECT, EXPLAIN, DESCRIBE, SHOW, and WITH queries are allowed");
        }

        Ok(())
    }
}

/// Convert a single cell from an Arrow array to a serde_json::Value.
fn arrow_to_json(col: &dyn Array, row: usize) -> serde_json::Value {
    if col.is_null(row) {
        return serde_json::Value::Null;
    }

    match col.data_type() {
        DataType::Boolean => {
            let a = col.as_any().downcast_ref::<BooleanArray>().unwrap();
            serde_json::Value::Bool(a.value(row))
        }
        DataType::Int8 => serde_json::json!(col.as_any().downcast_ref::<Int8Array>().unwrap().value(row)),
        DataType::Int16 => serde_json::json!(col.as_any().downcast_ref::<Int16Array>().unwrap().value(row)),
        DataType::Int32 => serde_json::json!(col.as_any().downcast_ref::<Int32Array>().unwrap().value(row)),
        DataType::Int64 => serde_json::json!(col.as_any().downcast_ref::<Int64Array>().unwrap().value(row)),
        DataType::UInt8 => serde_json::json!(col.as_any().downcast_ref::<UInt8Array>().unwrap().value(row)),
        DataType::UInt16 => serde_json::json!(col.as_any().downcast_ref::<UInt16Array>().unwrap().value(row)),
        DataType::UInt32 => serde_json::json!(col.as_any().downcast_ref::<UInt32Array>().unwrap().value(row)),
        DataType::UInt64 => serde_json::json!(col.as_any().downcast_ref::<UInt64Array>().unwrap().value(row)),
        DataType::Float32 => serde_json::json!(col.as_any().downcast_ref::<Float32Array>().unwrap().value(row)),
        DataType::Float64 => serde_json::json!(col.as_any().downcast_ref::<Float64Array>().unwrap().value(row)),
        DataType::Utf8 => {
            let a = col.as_any().downcast_ref::<StringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::LargeUtf8 => {
            let a = col.as_any().downcast_ref::<LargeStringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::Timestamp(_, _) => {
            // DuckDB timestamps come as microseconds in Int64
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

#[cfg(test)]
mod tests {
    use super::QueryEngine;

    #[test]
    fn test_validates_select() {
        assert!(QueryEngine::validate_query("SELECT * FROM logs").is_ok());
        assert!(QueryEngine::validate_query("  select count(*) from logs  ").is_ok());
    }

    #[test]
    fn test_rejects_multi_statement() {
        assert!(QueryEngine::validate_query("SELECT 1; DROP TABLE logs").is_err());
    }

    #[test]
    fn test_rejects_mutation() {
        assert!(QueryEngine::validate_query("DROP TABLE logs").is_err());
        assert!(QueryEngine::validate_query("INSERT INTO logs VALUES (1)").is_err());
        assert!(QueryEngine::validate_query("DELETE FROM logs").is_err());
    }

    #[test]
    fn test_allows_with_cte() {
        assert!(QueryEngine::validate_query(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ).is_ok());
    }

    #[test]
    fn test_trailing_semicolon_ok() {
        assert!(QueryEngine::validate_query("SELECT 1;").is_ok());
        assert!(QueryEngine::validate_query("SELECT 1;  ").is_ok());
    }
}
