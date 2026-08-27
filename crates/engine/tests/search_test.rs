//! Integration tests for [`QueryEngine::search`]: two-tier execution,
//! partition pruning correctness, facet/search translation, and aggregate
//! merging.

use std::collections::HashMap;

use arrow::array::{Array, AsArray, Int64Array, TimestampMicrosecondArray};

use greplog_engine::config::{EngineConfig, LiveBuffer};
use greplog_engine::memtable::MemTable;
use greplog_engine::query::QueryEngine;
use greplog_engine::record::LogRecord;
use greplog_engine::search::{GroupDimension, LogSearch, Metric, SearchMode};
use greplog_engine::storage::ParquetFlusher;
use tempfile::tempdir;

fn sample(age_secs: i64, index: usize, level: &str, service: &str) -> LogRecord {
    LogRecord {
        timestamp_us: chrono::Utc::now().timestamp_micros() - age_secs * 1_000_000,
        trace_id: Some(format!("trace-{index}")),
        span_id: None,
        parent_span_id: None,
        duration_ms: None,
        level: level.to_string(),
        service: service.to_string(),
        message: format!("order {index} failed for customer"),
        raw_body: Some(format!(r#"{{"order_id":{index}}}"#)),
    }
}

/// Seeds `count` disk rows spread between 4 hours and `spread_days` days ago.
fn seed_disk(config: &EngineConfig, count: usize, spread_days: i64) {
    let flusher = ParquetFlusher::new(config);
    let mut table = MemTable::new(count);
    for index in 0..count {
        let age = 4 * 3600 + (index as i64 + 1) * spread_days * 86_400 / count as i64;
        let level = if index % 4 == 0 {
            "ERROR"
        } else if index % 4 == 1 {
            "WARN"
        } else {
            "INFO"
        };
        table.append_record(&sample(age, index, level, "auth-api"));
    }
    flusher
        .flush(&table.finish().expect("finish memtable"))
        .expect("flush");
}

async fn engine_with(config: &EngineConfig, live_rows: &[LogRecord]) -> QueryEngine {
    if !live_rows.is_empty() {
        let mut table = MemTable::new(live_rows.len());
        for record in live_rows {
            table.append_record(record);
        }
        let buffer: LiveBuffer = LiveBuffer::default();
        buffer
            .write()
            .expect("lock")
            .push(table.finish().expect("finish"));
        return QueryEngine::new(config, buffer).await.expect("engine");
    }
    QueryEngine::new(config, LiveBuffer::default())
        .await
        .expect("engine")
}

fn rows_search(time_range_secs: u64, limit: usize) -> LogSearch {
    rows_search_offset(time_range_secs, limit, 0)
}

fn rows_search_offset(time_range_secs: u64, limit: usize, offset: usize) -> LogSearch {
    LogSearch {
        time_range_secs,
        facets: HashMap::new(),
        search: None,
        mode: SearchMode::Rows { limit, offset },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rows_mode_merges_both_tiers_newest_first() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk(&config, 200, 3);

    // Two live rows: one very recent, one older than everything on disk.
    let live = vec![
        sample(10, 10_000, "INFO", "payment-worker"),
        sample(40 * 86_400, 20_000, "ERROR", "payment-worker"),
    ];
    let engine = engine_with(&config, &live).await;

    let batches = engine
        .search(&rows_search(90 * 86_400, 500))
        .await
        .expect("search");
    let timestamps = collected_timestamps(&batches);
    assert_eq!(timestamps.len(), 202, "disk rows plus both live rows");
    assert!(
        timestamps.windows(2).all(|pair| pair[0] >= pair[1]),
        "newest first"
    );

    let services = collected_strings(&batches, "service");
    assert!(services.contains(&"payment-worker".to_string()));
    assert!(services.contains(&"auth-api".to_string()));

    let limited = engine
        .search(&rows_search(90 * 86_400, 50))
        .await
        .expect("limited search");
    assert_eq!(
        collected_timestamps(&limited).len(),
        50,
        "limit applies after merge"
    );

    // A window that excludes everything on disk still finds recent live rows.
    let recent_only = engine
        .search(&rows_search(3_600, 500))
        .await
        .expect("recent search");
    let timestamps = collected_timestamps(&recent_only);
    assert_eq!(
        timestamps.len(),
        1,
        "only the fresh live row falls inside 1 hour"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aggregate_mode_sums_across_tiers_and_groups() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk(&config, 200, 3);
    let live = vec![sample(30, 30_000, "ERROR", "auth-api")];
    let engine = engine_with(&config, &live).await;

    // Total count over a window covering both tiers.
    let total = engine
        .search(&LogSearch {
            time_range_secs: 90 * 86_400,
            facets: HashMap::new(),
            search: None,
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Service],
                bucket_secs: 60,
                metrics: vec![Metric::Count],
            },
        })
        .await
        .expect("aggregate search");
    assert_eq!(
        metric_ints(&total, "count"),
        vec![201],
        "disk 200 plus live 1, merged into one key"
    );

    // Level facet narrows to ERROR rows: 50 on disk (every fourth) + 1 live.
    let mut facets = HashMap::new();
    facets.insert("level".to_string(), "error".to_string());
    let errors = engine
        .search(&LogSearch {
            time_range_secs: 90 * 86_400,
            facets,
            search: None,
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Level],
                bucket_secs: 60,
                metrics: vec![Metric::Count],
            },
        })
        .await
        .expect("faceted search");
    assert_eq!(metric_ints(&errors, "count"), vec![51]);
    assert_eq!(
        collected_strings(&errors, "level"),
        vec!["ERROR"],
        "facet value is normalized against stored levels"
    );

    // last_seen takes the max across tiers.
    let last_seen = engine
        .search(&LogSearch {
            time_range_secs: 90 * 86_400,
            facets: HashMap::new(),
            search: None,
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Level],
                bucket_secs: 60,
                metrics: vec![Metric::Count, Metric::LastSeen],
            },
        })
        .await
        .expect("last_seen search");
    let seen = metric_ints(&last_seen, "last_seen");
    let all = collected_timestamps(
        &engine
            .search(&rows_search(90 * 86_400, 500))
            .await
            .expect("rows"),
    );
    assert_eq!(
        seen.iter().max(),
        all.first(),
        "the newest row's level reports the global last_seen"
    );
    for value in &seen {
        assert!(
            *value <= all.first().copied().unwrap_or(0),
            "no level exceeds the global newest"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bucket_grouping_returns_time_ordered_buckets() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };

    let flusher = ParquetFlusher::new(&config);
    let mut table = MemTable::new(120);
    for index in 0..120usize {
        let age = i64::try_from(index).unwrap() * 60; // spread over two hours
        table.append_record(&sample(age, index, "INFO", "auth-api"));
    }
    flusher
        .flush(&table.finish().expect("finish"))
        .expect("flush");

    let engine = engine_with(&config, &[]).await;
    let histogram = engine
        .search(&LogSearch {
            time_range_secs: 7 * 86_400,
            facets: HashMap::new(),
            search: None,
            mode: SearchMode::Aggregate {
                group_by: vec![GroupDimension::Bucket],
                bucket_secs: 600,
                metrics: vec![Metric::Count],
            },
        })
        .await
        .expect("histogram search");

    let batch = histogram.first().expect("one output batch");
    let buckets = batch
        .column_by_name("bucket")
        .and_then(|column| column.as_any().downcast_ref::<TimestampMicrosecondArray>())
        .expect("bucket column is timestamps");
    let bucket_values = buckets.values();
    assert!(bucket_values.len() >= 12, "two hours binned at ten minutes");
    assert!(
        bucket_values.windows(2).all(|pair| pair[0] < pair[1]),
        "buckets ascend"
    );
    assert!(
        bucket_values.iter().all(|value| value % 600_000_000 == 0),
        "bins align to epoch like date_bin does"
    );
    let counts = metric_ints(&histogram, "count");
    assert_eq!(counts.iter().sum::<i64>(), 120);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn free_text_and_service_facets_filter_rows() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };

    let flusher = ParquetFlusher::new(&config);
    let mut table = MemTable::new(20);
    for index in 0..20usize {
        let mut record = sample(3600, index, "INFO", "auth-api");
        if index % 5 == 0 {
            record.message = format!("unicorn sighting {index}");
            record.service = "payment-worker".to_string();
        }
        table.append_record(&record);
    }
    flusher
        .flush(&table.finish().expect("finish"))
        .expect("flush");

    let engine = engine_with(&config, &[]).await;

    let hits = engine
        .search(&LogSearch {
            time_range_secs: 86_400,
            facets: HashMap::new(),
            search: Some("unicorn".to_string()),
            mode: SearchMode::Rows {
                limit: 100,
                offset: 0,
            },
        })
        .await
        .expect("free-text search");
    let messages = collected_strings(&hits, "message");
    assert_eq!(messages.len(), 4);
    assert!(messages.iter().all(|message| message.contains("unicorn")));

    let mut facets = HashMap::new();
    facets.insert("service".to_string(), "payment-worker".to_string());
    let faceted = engine
        .search(&LogSearch {
            time_range_secs: 86_400,
            facets,
            search: None,
            mode: SearchMode::Rows {
                limit: 100,
                offset: 0,
            },
        })
        .await
        .expect("facet search");
    assert_eq!(collected_strings(&faceted, "service").len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offset_pagination_returns_disjoint_newest_first_pages() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk(&config, 250, 3);
    let engine = engine_with(&config, &[]).await;

    // Concatenating page 0, 1, 2 must reproduce the unpaginated result:
    // same rows, same newest-first order, no gaps or overlaps.
    let all = collected_timestamps(
        &engine
            .search(&rows_search(90 * 86_400, 500))
            .await
            .expect("unpaged search"),
    );
    assert_eq!(all.len(), 250);

    let mut paged: Vec<i64> = Vec::new();
    for offset in [0, 100, 200] {
        let timestamps = collected_timestamps(
            &engine
                .search(&rows_search_offset(90 * 86_400, 100, offset))
                .await
                .expect("paged search"),
        );
        assert!(
            timestamps.windows(2).all(|pair| pair[0] > pair[1]),
            "page strictly newest first"
        );
        paged.extend(timestamps);
    }
    assert_eq!(
        paged.len(),
        all.len(),
        "every row appears exactly once across pages"
    );
    assert_eq!(paged, all, "pages concatenate to the unpaged ordering");

    // An offset past the end of the window is an empty page, not an error.
    let past_end = engine
        .search(&rows_search_offset(90 * 86_400, 100, 300))
        .await
        .expect("offset past end");
    assert_eq!(collected_timestamps(&past_end).len(), 0);
}

#[tokio::test]
async fn invalid_requests_are_rejected_not_executed() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        max_query_rows: 100,
        ..EngineConfig::default()
    };
    let engine = engine_with(&config, &[]).await;

    let oversized_limit = LogSearch {
        time_range_secs: 3600,
        facets: HashMap::new(),
        search: None,
        mode: SearchMode::Rows {
            limit: 101,
            offset: 0,
        },
    };
    let error = engine
        .search(&oversized_limit)
        .await
        .expect_err("must reject");
    assert!(
        matches!(error, greplog_engine::error::EngineError::QueryRejected(_)),
        "expected rejection, got: {error}"
    );

    let bad_facet = LogSearch {
        time_range_secs: 3600,
        facets: [("unknown_field".to_string(), "x".to_string())]
            .into_iter()
            .collect(),
        search: None,
        mode: SearchMode::Rows {
            limit: 10,
            offset: 0,
        },
    };
    assert!(engine.search(&bad_facet).await.is_err());
    let trace_facet = LogSearch {
        time_range_secs: 3600,
        facets: [("trace_id".to_string(), "job_abc".to_string())]
            .into_iter()
            .collect(),
        search: None,
        mode: SearchMode::Rows {
            limit: 10,
            offset: 0,
        },
    };
    assert!(engine.search(&trace_facet).await.is_ok());

    let zero_bucket = LogSearch {
        time_range_secs: 3600,
        facets: HashMap::new(),
        search: None,
        mode: SearchMode::Aggregate {
            group_by: vec![GroupDimension::Bucket],
            bucket_secs: 0,
            metrics: vec![Metric::Count],
        },
    };
    assert!(engine.search(&zero_bucket).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_aggregate_folds_both_tiers_into_one_row() {
    let dir = tempdir().expect("tmp");
    let config = EngineConfig {
        data_dir: dir.path().join("logs"),
        ..EngineConfig::default()
    };
    seed_disk(&config, 40, 3);
    let live = vec![sample(30, 30_000, "ERROR", "payment-worker")];
    let engine = engine_with(&config, &live).await;

    let totals = engine
        .search(&LogSearch {
            time_range_secs: 90 * 86_400,
            facets: HashMap::new(),
            search: None,
            mode: SearchMode::Aggregate {
                group_by: vec![],
                bucket_secs: 60,
                metrics: vec![Metric::Count, Metric::Errors],
            },
        })
        .await
        .expect("global aggregate");

    assert_eq!(metric_ints(&totals, "count"), vec![41]);
    assert_eq!(
        metric_ints(&totals, "errors"),
        vec![11],
        "10 disk ERROR rows plus 1 live"
    );
}

fn collected_timestamps(batches: &[arrow::record_batch::RecordBatch]) -> Vec<i64> {
    batches
        .iter()
        .flat_map(|batch| {
            int_values(
                batch
                    .column_by_name("timestamp_us")
                    .expect("timestamp column"),
            )
        })
        .collect()
}

/// Reads an integer-or-timestamp column as microseconds/counts.
fn int_values(column: &arrow::array::ArrayRef) -> Vec<i64> {
    match column.data_type() {
        arrow::datatypes::DataType::Timestamp(_, _) => (0..column.len())
            .map(|row| {
                column
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .expect("microsecond timestamps")
                    .value(row)
            })
            .collect(),
        _ => column
            .as_any()
            .downcast_ref::<Int64Array>()
            .expect("int column")
            .values()
            .to_vec(),
    }
}

fn collected_strings(batches: &[arrow::record_batch::RecordBatch], name: &str) -> Vec<String> {
    batches
        .iter()
        .flat_map(|batch| {
            batch.column_by_name(name).map_or_else(Vec::new, |column| {
                let cast = arrow::compute::cast(column, &arrow::datatypes::DataType::Utf8)
                    .expect("string cast");
                cast.as_string::<i32>()
                    .iter()
                    .flatten()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

fn metric_ints(batches: &[arrow::record_batch::RecordBatch], name: &str) -> Vec<i64> {
    batches
        .iter()
        .flat_map(|batch| int_values(batch.column_by_name(name).expect("metric column")))
        .collect()
}
