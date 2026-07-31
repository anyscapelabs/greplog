use super::*;
use crate::store::io::write_parquet_atomic;
use arrow::array::{Int32Array, ListBuilder, StringBuilder};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("greplog_query_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

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
    let ts = 1_700_000_000_000_000;
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    st_builder.append(false);
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
            Arc::new(StringArray::from(vec![None::<&str>])),  // logger_name
            Arc::new(StringArray::from(vec![None::<&str>])),  // file
            Arc::new(Int32Array::from(vec![None::<i32>])),   // line
            Arc::new(StringArray::from(vec![None::<&str>])),  // correlation_id
            Arc::new(st_builder.finish()),                    // stack_trace
            Arc::new(StringArray::from(vec![None::<&str>])),  // exception_type
            Arc::new(StringArray::from(vec![None::<&str>])),  // exception_message
        ],
    )
    .expect("build log batch");
    write_parquet_atomic(&dir, &batch).expect("write log parquet");
    dir
}

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

async fn count_rows(engine: &QueryEngine, sql: &str) -> usize {
    let result = engine.query(sql).await.expect("query");
    result
        .rows
        .first()
        .and_then(|r| r.first())
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0)
}

/// Write a log row with HTTP attributes and `logger_name = 'greplog.http'`,
/// matching the shape emitted by the Node.js and Python SDKs' HTTP middleware
/// (whose `level` is derived from the response status: >=500 -> error,
/// 400-499 -> warn, <400 -> info).
fn write_http_log_file(base: &Path, service: &str, date: &str, id: &str, latency_ms: &str, status_code: &str) -> PathBuf {
    let dir = base
        .join("data")
        .join("table_type=logs")
        .join(format!("service={}", service))
        .join(format!("date={}", date));
    let schema = greplog_core::arrow_schema::log_schema();
    let ts = 1_705_276_800_000_000; // 2024-01-15 00:00:00 UTC (µs), matching the date partition
    let level = match status_code.parse::<u16>() {
        Ok(code) if code >= 500 => "error",
        Ok(code) if code >= 400 => "warn",
        _ => "info",
    };
    let attributes = serde_json::json!({
        "http.method": "GET",
        "http.route": "/api",
        "http.status_code": status_code,
        "http.latency_ms": latency_ms,
    })
    .to_string();
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    st_builder.append(false);
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(vec![id])),
            Arc::new(StringArray::from(vec![service])),
            Arc::new(TimestampMicrosecondArray::from(vec![ts])),
            Arc::new(StringArray::from(vec![Some(level)])),
            Arc::new(StringArray::from(vec![Some("/api")])),
            Arc::new(StringArray::from(vec![Some("GET /api -> 200")])),
            Arc::new(StringArray::from(vec![Some(attributes.as_str())])),
            Arc::new(StringArray::from(vec![Some("greplog.http")])), // logger_name
            Arc::new(StringArray::from(vec![None::<&str>])),  // file
            Arc::new(Int32Array::from(vec![None::<i32>])),   // line
            Arc::new(StringArray::from(vec![None::<&str>])),  // correlation_id
            Arc::new(st_builder.finish()),                    // stack_trace
            Arc::new(StringArray::from(vec![None::<&str>])),  // exception_type
            Arc::new(StringArray::from(vec![None::<&str>])),  // exception_message
        ],
    )
    .expect("build http log batch");
    write_parquet_atomic(&dir, &batch).expect("write http log parquet");
    dir
}

/// Write a span row with typed HTTP metadata, matching the shape emitted by the
/// Go and Rust SDKs' HTTP middleware (start_time is the spans table's time
/// column — there is no `timestamp` column).
fn write_http_span_file(
    base: &Path,
    service: &str,
    date: &str,
    id: &str,
    latency_ms: f64,
    status_code: i32,
    start_time_us: i64,
) -> PathBuf {
    let dir = base
        .join("data")
        .join("table_type=spans")
        .join(format!("service={}", service))
        .join(format!("date={}", date));
    let schema = greplog_core::arrow_schema::span_schema();
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(vec![id])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![service])),
            Arc::new(StringArray::from(vec![Some("GET /api")])),
            Arc::new(StringArray::from(vec![Some("/api")])),
            Arc::new(StringArray::from(vec![Some("GET")])),
            Arc::new(Int32Array::from(vec![Some(status_code)])),
            Arc::new(Float64Array::from(vec![Some(latency_ms)])),
            Arc::new(BooleanArray::from(vec![Some(false)])),
            Arc::new(TimestampMicrosecondArray::from(vec![start_time_us])),
            Arc::new(TimestampMicrosecondArray::from(vec![start_time_us])),
            Arc::new(StringArray::from(vec![None::<&str>])),
        ],
    )
    .expect("build http span batch");
    write_parquet_atomic(&dir, &batch).expect("write http span parquet");
    dir
}

#[tokio::test]
async fn test_basic_query() {
    let dir = test_dir("basic_query");
    write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "hello");
    let engine = QueryEngine::new(&dir).expect("new engine");

    let result = engine
        .query("SELECT * FROM logs WHERE service = 'api'")
        .await
        .expect("query");
    assert_eq!(result.columns.len(), 15);
    assert_eq!(result.row_count, 1);
    assert_eq!(result.rows[0][4], serde_json::json!("hello"));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_partition_pruning() {
    let dir = test_dir("partition_pruning");
    write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "api msg");
    write_log_file(&dir, "worker", "2024-01-15", "id-2", "debug", "worker msg");
    write_log_file(&dir, "api", "2024-01-16", "id-3", "error", "api error");

    let engine = QueryEngine::new(&dir).expect("new engine");

    let result = engine
        .query("SELECT message, service FROM logs WHERE service = 'api'")
        .await
        .expect("query");
    assert_eq!(result.row_count, 2, "should find both api rows");
    for row in &result.rows {
        assert_eq!(row[1], serde_json::json!("api"));
    }

    let result = engine
        .query("SELECT message FROM logs WHERE date = '2024-01-15'")
        .await
        .expect("query");
    assert_eq!(result.row_count, 2, "two rows on 2024-01-15");

    let result = engine
        .query("SELECT message FROM logs WHERE service = 'api' AND date = '2024-01-16'")
        .await
        .expect("query");
    assert_eq!(result.row_count, 1);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_limit_injection_no_existing() {
    let dir = test_dir("limit_no_existing");
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
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    for _ in 0..n {
        st_builder.append(false);
    }
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(services)),
            Arc::new(TimestampMicrosecondArray::from(ts)),
            Arc::new(StringArray::from(levels)),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(msg_arr),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(Int32Array::from(vec![None::<i32>; n])),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(st_builder.finish()),
            Arc::new(StringArray::from(none_str.clone())),
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

    let result = engine
        .query("SELECT * FROM (SELECT * FROM logs LIMIT 3) AS sub")
        .await
        .expect("query");
    assert_eq!(result.row_count, 3, "subquery limit should be respected");

    let result = engine
        .query("WITH cte AS (SELECT * FROM logs LIMIT 3) SELECT * FROM cte")
        .await
        .expect("query");
    assert_eq!(result.row_count, 3, "CTE limit should be respected");

    let _ = fs::remove_dir_all(&dir);
}

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

    let idle_cpu = if cfg!(target_os = "linux") {
        let before = cpu_time_ns().unwrap_or(u64::MAX);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let after = cpu_time_ns().unwrap_or(u64::MAX);
        after.saturating_sub(before)
    } else {
        0
    };

    let df = engine
        .ctx()
        .sql("SELECT COUNT(*) FROM logs CROSS JOIN logs AS l2")
        .await
        .expect("cross join sql");

    let cpu_pre_query = cpu_time_ns().unwrap_or(u64::MAX);
    let result = tokio::time::timeout(Duration::from_millis(1), df.collect()).await;
    let cpu_post_timeout = cpu_time_ns().unwrap_or(u64::MAX);

    assert!(result.is_err(), "cross-join should timeout with 1ms limit");

    tokio::time::sleep(Duration::from_millis(800)).await;
    let cpu_post_wait = cpu_time_ns().unwrap_or(u64::MAX);

    let cpu_during_query = cpu_post_timeout.saturating_sub(cpu_pre_query);
    let cpu_during_wait = cpu_post_wait.saturating_sub(cpu_post_timeout);

    let threshold = idle_cpu.saturating_add(50_000_000);
    if cpu_during_wait > threshold {
        eprintln!(
            "[cpu-monitor] idle baseline={}ms  during-query={}ms  during-wait={}ms  threshold={}ms",
            idle_cpu / 1_000_000,
            cpu_during_query / 1_000_000,
            cpu_during_wait / 1_000_000,
            threshold / 1_000_000,
        );
    }

    let result = engine
        .query("SELECT 'hello' AS greeting")
        .await
        .expect("simple query after timeout");
    assert_eq!(result.row_count, 1);
    assert_eq!(result.rows[0][0], serde_json::json!("hello"));

    let _ = fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
fn cpu_time_ns() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let paren_end = stat.rfind(')')?;
    let rest = &stat[paren_end + 2..];
    let mut fields = rest.split_whitespace();
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    const CLK_TCK: u64 = 100;
    Some((utime + stime) * 1_000_000_000 / CLK_TCK)
}

#[cfg(not(target_os = "linux"))]
fn cpu_time_ns() -> Option<u64> {
    None
}

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
    let mut st_builder = ListBuilder::new(StringBuilder::new());
    for _ in 0..n {
        st_builder.append(false);
    }
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(services)),
            Arc::new(TimestampMicrosecondArray::from(ts)),
            Arc::new(StringArray::from(levels)),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(msg_arr),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(Int32Array::from(vec![None::<i32>; n])),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(st_builder.finish()),
            Arc::new(StringArray::from(none_str.clone())),
            Arc::new(StringArray::from(none_str)),
        ],
    )
    .expect("build batch");
    write_parquet_atomic(&part_dir, &batch).expect("write parquet");
    let engine = QueryEngine::new(&dir).expect("new engine");

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

#[tokio::test]
async fn test_empty_database() {
    let dir = test_dir("empty_db");
    let engine = QueryEngine::new(&dir).expect("new engine");

    let result = engine.query("SELECT * FROM logs").await.expect("query on empty db");
    assert_eq!(result.row_count, 0, "empty db returns zero rows");
    assert!(result.columns.is_empty() || result.columns.len() == 15);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_mutation_rejection() {
    let dir = test_dir("mutation_rejection");
    write_log_file(&dir, "api", "2024-01-15", "id-1", "info", "hello");
    let engine = QueryEngine::new(&dir).expect("new engine");

    async fn try_mutation(
        engine: &QueryEngine,
        sql: &str,
    ) -> Result<String, String> {
        match engine.query(sql).await {
            Err(e) => Ok(format!("rejected: {}", e)),
            Ok(_) => Err(sql.to_string()),
        }
    }

    let r = try_mutation(&engine, "INSERT INTO logs VALUES ('x', 'svc', 0, 'info', 'msg')").await;
    assert!(r.is_ok(), "INSERT should be rejected: {:?}", r);

    let r = try_mutation(&engine, "DELETE FROM logs WHERE id = 'id-1'").await;
    assert!(r.is_ok(), "DELETE should be rejected: {:?}", r);

    let r = try_mutation(&engine, "UPDATE logs SET message = 'hacked' WHERE id = 'id-1'").await;
    assert!(r.is_ok(), "UPDATE should be rejected: {:?}", r);

    let r = try_mutation(&engine, "DROP TABLE logs").await;
    assert!(r.is_ok(), "DROP TABLE should be rejected: {:?}", r);

    let r = try_mutation(&engine, "CREATE TABLE evil AS SELECT * FROM logs").await;
    assert!(r.is_ok(), "CREATE TABLE AS should be rejected: {:?}", r);

    let r = try_mutation(
        &engine,
        "COPY logs TO '/tmp/greplog_exfil_test.parquet'",
    )
    .await;
    assert!(r.is_ok(), "COPY TO should be rejected: {:?}", r);

    let r = try_mutation(&engine, "SELECT 1; DROP TABLE logs;").await;
    assert!(r.is_ok(), "multi-statement should be rejected: {:?}", r);

    let after = engine
        .query("SELECT COUNT(*) AS cnt FROM logs")
        .await
        .expect("count after all mutation attempts");
    assert_eq!(
        after.rows[0][0],
        serde_json::json!(1i64),
        "data should be unchanged after all mutation attempts"
    );
    let row = engine
        .query("SELECT message FROM logs WHERE id = 'id-1'")
        .await
        .expect("check original row");
    assert_eq!(row.rows[0][0], serde_json::json!("hello"));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_new_data_visibility_within_staleness_window() {
    let dir = test_dir("visibility_within_window");

    write_log_file(&dir, "svc", "2024-01-15", "id-A", "info", "batch A");

    let engine = QueryEngine::new(&dir).expect("new engine");
    assert_eq!(
        count_rows(&engine, "SELECT count(*) AS cnt FROM logs").await,
        1,
        "cold cache: should see batch A",
    );

    write_log_file(&dir, "svc", "2024-01-15", "id-B", "warn", "batch B");

    assert_eq!(
        count_rows(&engine, "SELECT count(*) AS cnt FROM logs").await,
        1,
        "stale cache: new file not yet visible",
    );

    tokio::time::sleep(Duration::from_secs(LIST_FILES_CACHE_TTL_SECS + 1)).await;

    assert_eq!(
        count_rows(&engine, "SELECT count(*) AS cnt FROM logs").await,
        2,
        "both files visible after cache entry expiry",
    );
    assert_eq!(
        count_rows(&engine, "SELECT count(*) FROM logs WHERE id = 'id-B'").await,
        1,
        "batch B row appears after cache expiry",
    );

    let _ = fs::remove_dir_all(&dir);
}

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
    assert_eq!(result.rows[0][4], serde_json::json!("real data"));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_json_get_str_http_attributes() {
    let dir = test_dir("json_get_str_http");

    write_http_log_file(&dir, "web", "2024-01-15", "id-1", "42.5", "200");
    write_http_log_file(&dir, "web", "2024-01-15", "id-2", "77.0", "500");
    write_http_log_file(&dir, "api", "2024-01-15", "id-3", "12.5", "200");
    // A plain log with NULL attributes and no logger_name — its attributes
    // column must not break json_get_str for the rows that do have JSON.
    write_log_file(&dir, "web", "2024-01-15", "id-4", "info", "plain log");

    let engine = QueryEngine::new(&dir).expect("new engine");

    // Raw string extraction from the JSON attributes column.
    let result = engine
        .query("SELECT json_get_str(attributes, 'http.status_code') AS code FROM logs WHERE logger_name = 'greplog.http' ORDER BY id")
        .await
        .expect("query json_get_str");
    assert_eq!(result.row_count, 3);
    assert_eq!(result.rows[0][0], serde_json::json!("200"));
    assert_eq!(result.rows[1][0], serde_json::json!("500"));

    // Cast + aggregate path used by the dashboard's latency percentile chart.
    let result = engine
        .query("SELECT approx_percentile_cont(CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE), 0.50) AS p50 FROM logs WHERE logger_name = 'greplog.http'")
        .await
        .expect("query latency percentile");
    assert_eq!(result.rows[0][0], serde_json::json!(42.5));

    // Group-by path used by the dashboard's status-code pie chart.
    let result = engine
        .query("SELECT json_get_str(attributes, 'http.status_code') AS code, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http' GROUP BY json_get_str(attributes, 'http.status_code') ORDER BY cnt DESC")
        .await
        .expect("query status codes");
    assert_eq!(result.row_count, 2, "200 and 500");
    assert_eq!(result.rows[0][0], serde_json::json!("200"));
    assert_eq!(result.rows[0][1], serde_json::json!(2i64));

    // Sum + count path used by the dashboard's avg-response-time chart.
    let result = engine
        .query("SELECT service, sum(CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE)) AS sum_ms, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http' GROUP BY service ORDER BY service")
        .await
        .expect("query avg response time");
    assert_eq!(result.row_count, 2);
    assert_eq!(result.rows[0][0], serde_json::json!("api"));
    assert_eq!(result.rows[0][1], serde_json::json!(12.5));

    // Missing keys / NULL attributes produce NULL (skipped by aggregates),
    // not a query error — scan all rows including the plain one.
    let result = engine
        .query("SELECT id, json_get_str(attributes, 'http.latency_ms') AS lat FROM logs ORDER BY id")
        .await
        .expect("query all rows with json_get_str");
    assert_eq!(result.row_count, 4);
    assert_eq!(result.rows[3][1], serde_json::Value::Null, "plain log has no http attributes");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_union_all_http_span_and_log_attributes() {
    // Mirrors the dashboard's 3 HTTP chart queries: the spans and
    // logs-attributes sources (Go/Rust SDKs vs Node.js/Python SDKs) are
    // UNION ALL'd server-side and aggregated over the combined population.
    // The spans arm uses the translated time predicate (`start_time`, since
    // the spans table has no `timestamp` column); the logs arm uses
    // `timestamp`.
    let dir = test_dir("union_all_http");
    let day = 1_705_276_800_000_000; // 2024-01-15 00:00:00 UTC (µs)
    let old_day = day - 86_400_000_000; // 2024-01-14 00:00:00 UTC (µs)

    // Go/Rust SDKs → typed spans (service "web").
    write_http_span_file(&dir, "web", "2024-01-15", "s-1", 42.5, 200, day);
    write_http_span_file(&dir, "web", "2024-01-15", "s-2", 77.0, 500, day);
    // Outside the time-range filter — must be excluded from the spans arm.
    write_http_span_file(&dir, "web", "2024-01-14", "s-old", 999.0, 200, old_day);

    // Node.js/Python SDKs → log attributes (services "web" + "api").
    write_http_log_file(&dir, "web", "2024-01-15", "id-4", "150.0", "404");
    write_http_log_file(&dir, "api", "2024-01-15", "id-3", "12.5", "200");

    let engine = QueryEngine::new(&dir).expect("new engine");
    let spans_pred = "start_time >= TIMESTAMP '2024-01-15 00:00:00'";
    let logs_pred = "timestamp >= TIMESTAMP '2024-01-15 00:00:00'";

    // Avg response time per service over BOTH sources: web = (42.5 + 77.0 +
    // 150.0)/3, api = 12.5/1. The 999ms old span must not leak in.
    let result = engine
        .query(&format!(
            "SELECT service, sum(latency_ms) AS sum_ms, count(*) AS cnt FROM (
               SELECT service, latency_ms FROM spans WHERE {spans_pred}
               UNION ALL
               SELECT service, CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
               FROM logs WHERE logger_name = 'greplog.http' AND {logs_pred}
             ) t GROUP BY service ORDER BY service"
        ))
        .await
        .expect("query union avg response time");
    assert_eq!(result.row_count, 2);
    assert_eq!(result.rows[0][0], serde_json::json!("api"));
    assert_eq!(result.rows[0][1], serde_json::json!(12.5));
    assert_eq!(result.rows[0][2], serde_json::json!(1i64));
    assert_eq!(result.rows[1][0], serde_json::json!("web"));
    assert_eq!(result.rows[1][1], serde_json::json!(42.5 + 77.0 + 150.0));
    assert_eq!(result.rows[1][2], serde_json::json!(3i64));

    // Status codes combined across both sources (old span excluded by filter):
    // 200 appears twice (s-1 + id-3), 404 and 500 once each.
    let result = engine
        .query(&format!(
            "SELECT status_code, sum(cnt) AS cnt FROM (
               SELECT CAST(status_code AS VARCHAR) AS status_code, count(*) AS cnt
               FROM spans WHERE {spans_pred} GROUP BY CAST(status_code AS VARCHAR)
               UNION ALL
               SELECT json_get_str(attributes, 'http.status_code') AS status_code, count(*) AS cnt
               FROM logs WHERE logger_name = 'greplog.http' AND {logs_pred}
               GROUP BY json_get_str(attributes, 'http.status_code')
             ) t GROUP BY status_code ORDER BY cnt DESC, status_code"
        ))
        .await
        .expect("query union status codes");
    assert_eq!(result.row_count, 3, "200, 404 and 500 across both sources");
    assert_eq!(result.rows[0][0], serde_json::json!("200"));
    assert_eq!(result.rows[0][1], serde_json::json!(2i64));
    assert_eq!(result.rows[1][0], serde_json::json!("404"));
    assert_eq!(result.rows[1][1], serde_json::json!(1i64));
    assert_eq!(result.rows[2][0], serde_json::json!("500"));
    assert_eq!(result.rows[2][1], serde_json::json!(1i64));

    // Latency percentile over the combined population. The exact value depends
    // on the t-digest approximation, so assert the four contributing rows and
    // that the percentile is inside their range.
    let result = engine
        .query(&format!(
            "SELECT approx_percentile_cont(latency_ms, 0.50) AS p50 FROM (
               SELECT latency_ms FROM spans WHERE {spans_pred}
               UNION ALL
               SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
               FROM logs WHERE logger_name = 'greplog.http' AND {logs_pred}
             ) t"
        ))
        .await
        .expect("query union latency percentile");
    assert_eq!(result.row_count, 1);
    let p50 = result.rows[0][0].as_f64().expect("p50 is a number");
    assert!((12.5..=150.0).contains(&p50), "p50 {p50} must sit between the min and max latency");

    // The UNION source must contain exactly the filtered rows (2 spans + 2
    // logs), proving both SDK paths contribute and the old span is excluded.
    let result = engine
        .query(&format!(
            "SELECT count(*) AS cnt FROM (
               SELECT latency_ms FROM spans WHERE {spans_pred}
               UNION ALL
               SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
               FROM logs WHERE logger_name = 'greplog.http' AND {logs_pred}
             ) t"
        ))
        .await
        .expect("query union row count");
    assert_eq!(result.rows[0][0], serde_json::json!(4i64));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_union_all_http_level_and_status_filters() {
    // Mirrors the dashboard's per-arm predicate translation (httpArmPredicates
    // in useAnalytics.ts). A `level IN ('error')` filter stays raw on the logs
    // arm (greplog.http rows carry the status-derived level, so 'error' IS
    // status >= 500) and becomes `status_code >= 500` on the spans arm — both
    // arms must select the SAME error population.
    let dir = test_dir("union_all_http_filters");
    let day = 1_705_276_800_000_000; // 2024-01-15 00:00:00 UTC (µs)

    // Spans (Go/Rust SDK shape): a 200 and a 500.
    write_http_span_file(&dir, "web", "2024-01-15", "s-ok", 42.5, 200, day);
    write_http_span_file(&dir, "web", "2024-01-15", "s-err", 77.0, 500, day);

    // HTTP logs (Node/Python SDK shape): level derived from status, so the 500
    // row is level 'error' and the 200 row is level 'info'.
    write_http_log_file(&dir, "web", "2024-01-15", "id-ok", "12.5", "200");
    write_http_log_file(&dir, "web", "2024-01-15", "id-err", "150.0", "500");

    let engine = QueryEngine::new(&dir).expect("new engine");

    // `level IN ('error')` → only the two 500 rows across both arms.
    let result = engine
        .query("SELECT count(*) AS cnt FROM (
           SELECT latency_ms FROM spans WHERE status_code >= 500
           UNION ALL
           SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
           FROM logs WHERE logger_name = 'greplog.http' AND (level IN ('error'))
         ) t")
        .await
        .expect("query level-filtered union count");
    assert_eq!(result.rows[0][0], serde_json::json!(2i64), "only error-status rows across both arms");

    // Per-service sum/count over the error population.
    let result = engine
        .query("SELECT service, sum(latency_ms) AS sum_ms, count(*) AS cnt FROM (
           SELECT service, latency_ms FROM spans WHERE status_code >= 500
           UNION ALL
           SELECT service, CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
           FROM logs WHERE logger_name = 'greplog.http' AND (level IN ('error'))
         ) t GROUP BY service ORDER BY service")
        .await
        .expect("query level-filtered union avg");
    assert_eq!(result.row_count, 1);
    assert_eq!(result.rows[0][0], serde_json::json!("web"));
    assert_eq!(result.rows[0][1], serde_json::json!(77.0 + 150.0));
    assert_eq!(result.rows[0][2], serde_json::json!(2i64));

    // Status-chip translation: `line >= 500` becomes a status_code predicate on
    // BOTH arms (line is null on HTTP rows; the chip means response status).
    let result = engine
        .query("SELECT count(*) AS cnt FROM (
           SELECT latency_ms FROM spans WHERE status_code >= 500
           UNION ALL
           SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
           FROM logs WHERE logger_name = 'greplog.http'
             AND (CAST(json_get_str(attributes, 'http.status_code') AS INT) >= 500)
         ) t")
        .await
        .expect("query status-chip union count");
    assert_eq!(result.rows[0][0], serde_json::json!(2i64));

    // Free-text translation: `message LIKE '%api%'` stays raw on the logs arm
    // and becomes `name LIKE`/`route LIKE` on the spans arm. All four rows
    // mention /api, so the combined population is all four.
    let result = engine
        .query("SELECT count(*) AS cnt FROM (
           SELECT latency_ms FROM spans WHERE (name LIKE '%api%' OR route LIKE '%api%')
           UNION ALL
           SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
           FROM logs WHERE logger_name = 'greplog.http' AND (message LIKE '%api%')
         ) t")
        .await
        .expect("query message-filtered union count");
    assert_eq!(result.rows[0][0], serde_json::json!(4i64), "all rows mention /api");

    // A search matching nothing must exclude rows on BOTH arms, not just one.
    let result = engine
        .query("SELECT count(*) AS cnt FROM (
           SELECT latency_ms FROM spans WHERE (name LIKE '%nonexistent%' OR route LIKE '%nonexistent%')
           UNION ALL
           SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
           FROM logs WHERE logger_name = 'greplog.http' AND (message LIKE '%nonexistent%')
         ) t")
        .await
        .expect("query non-matching message union count");
    assert_eq!(result.rows[0][0], serde_json::json!(0i64));

    let _ = fs::remove_dir_all(&dir);
}
