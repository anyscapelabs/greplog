use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

pub fn log_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("service", DataType::Utf8, false),
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("level", DataType::Utf8, true),
        Field::new("route", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, true),
        Field::new("attributes", DataType::Utf8, true),
    ])
}

pub fn span_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("correlation_id", DataType::Utf8, true),
        Field::new("parent_correlation_id", DataType::Utf8, true),
        Field::new("service", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, true),
        Field::new("route", DataType::Utf8, true),
        Field::new("method", DataType::Utf8, true),
        Field::new("status_code", DataType::Int32, true),
        Field::new("latency_ms", DataType::Float64, true),
        Field::new("is_error", DataType::Boolean, true),
        Field::new("start_time", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("end_time", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("attributes", DataType::Utf8, true),
    ])
}

pub fn metric_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("service", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("value", DataType::Float64, false),
        Field::new("labels", DataType::Utf8, true),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{
        BooleanArray, Float64Array, Int32Array, StringArray,
        TimestampMicrosecondArray,
    };
    use arrow::record_batch::RecordBatch;
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
                Arc::new(StringArray::from(vec![
                    Some(r#"{"method": "GET"}"#),
                    None,
                ])),
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
}
