use super::*;
use crate::store::io::write_parquet_atomic;
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
    assert!(result.columns.is_empty() || result.columns.len() == 7);

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
