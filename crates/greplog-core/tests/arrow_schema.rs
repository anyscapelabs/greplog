use arrow::array::{
    BooleanArray, Float64Array, Int32Array, StringArray, TimestampMicrosecondArray,
};
use arrow::record_batch::RecordBatch;
use greplog_core::arrow_schema::{log_schema, metric_schema, span_schema};
use std::sync::Arc;

#[test]
fn test_log_schema() {
    let schema = log_schema();

    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(vec!["log-1", "log-2"])),
            Arc::new(StringArray::from(vec!["api", "worker"])),
            Arc::new(TimestampMicrosecondArray::from(vec![
                1_000_000_000_000,
                1_000_000_000_001,
            ])),
            Arc::new(StringArray::from(vec![Some("info"), Some("error")])),
            Arc::new(StringArray::from(vec![Some("/users"), None])),
            Arc::new(StringArray::from(vec![
                Some("request completed"),
                Some("connection refused"),
            ])),
            Arc::new(StringArray::from(vec![Some(r#"{"method": "GET"}"#), None])),
        ],
    )
    .expect("log RecordBatch should validate against schema");

    assert_eq!(batch.num_rows(), 2);
}

#[test]
fn test_span_schema() {
    let schema = span_schema();

    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(vec!["span-1", "span-2"])),
            Arc::new(StringArray::from(vec![Some("corr-1"), None])),
            Arc::new(StringArray::from(Vec::<Option<&str>>::from([None, None]))),
            Arc::new(StringArray::from(vec!["api", "db"])),
            Arc::new(StringArray::from(vec![Some("GET /users"), Some("SELECT")])),
            Arc::new(StringArray::from(vec![Some("/users"), None])),
            Arc::new(StringArray::from(vec![Some("GET"), None])),
            Arc::new(Int32Array::from(vec![Some(200), Some(500)])),
            Arc::new(Float64Array::from(vec![Some(42.5), Some(1000.0)])),
            Arc::new(BooleanArray::from(vec![Some(false), Some(true)])),
            Arc::new(TimestampMicrosecondArray::from(vec![
                1_000_000_000_000,
                1_000_000_000_500,
            ])),
            Arc::new(TimestampMicrosecondArray::from(vec![
                1_000_000_000_042,
                1_000_000_001_500,
            ])),
            Arc::new(StringArray::from(vec![
                Some(r#"{"db.statement": "SELECT 1"}"#),
                None,
            ])),
        ],
    )
    .expect("span RecordBatch should validate against schema");

    assert_eq!(batch.num_rows(), 2);
}

#[test]
fn test_metric_schema() {
    let schema = metric_schema();

    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(vec!["metric-1", "metric-2"])),
            Arc::new(StringArray::from(vec!["api", "worker"])),
            Arc::new(StringArray::from(vec!["http.requests", "queue.size"])),
            Arc::new(TimestampMicrosecondArray::from(vec![
                1_000_000_000_000,
                1_000_000_001_000,
            ])),
            Arc::new(Float64Array::from(vec![42.0, 100.5])),
            Arc::new(StringArray::from(vec![
                Some(r#"{"method": "GET", "status": "200"}"#),
                None,
            ])),
        ],
    )
    .expect("metric RecordBatch should validate against schema");

    assert_eq!(batch.num_rows(), 2);
}
