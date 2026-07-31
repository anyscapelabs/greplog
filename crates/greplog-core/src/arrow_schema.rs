use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use std::sync::Arc;

pub fn log_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("service", DataType::Utf8, false),
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("level", DataType::Utf8, true),
        Field::new("route", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, true),
        Field::new("attributes", DataType::Utf8, true),
        Field::new("logger_name", DataType::Utf8, true),
        Field::new("file", DataType::Utf8, true),
        Field::new("line", DataType::Int32, true),
        Field::new("correlation_id", DataType::Utf8, true),
        Field::new("stack_trace", DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))), true),
        Field::new("exception_type", DataType::Utf8, true),
        Field::new("exception_message", DataType::Utf8, true),
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


