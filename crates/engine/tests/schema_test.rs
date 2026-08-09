//! Integration tests for the public Greplog engine API.
//!
//! Validates that `greplog_schema()` produces the exact columnar layout the
//! storage engine and SDKs depend on.

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use greplog_engine::{greplog_schema, EngineError, LogRecord};

#[test]
fn schema_has_canonical_field_order_and_types() {
    let schema: Arc<Schema> = greplog_schema();

    let expected = [
        ("timestamp_us", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        ("trace_id", DataType::Utf8, true),
        (
            "level",
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            false,
        ),
        (
            "service",
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            false,
        ),
        ("message", DataType::Utf8, false),
        ("raw_body", DataType::Utf8, true),
    ];

    assert_eq!(
        schema.fields().len(),
        expected.len(),
        "greplog_schema must declare exactly {} fields",
        expected.len()
    );

    for (index, (name, data_type, nullable)) in expected.iter().enumerate() {
        let field: &Field = schema.field(index);
        assert_eq!(field.name(), name, "field {index} must be named {name}");
        assert_eq!(field.data_type(), data_type, "field {name} must have the intended type");
        assert_eq!(
            field.is_nullable(),
            *nullable,
            "field {name} nullability must match the schema contract"
        );
    }
}

#[test]
fn level_and_service_are_dictionary_encoded() {
    let schema = greplog_schema();

    let level = schema.field(2);
    assert!(
        matches!(level.data_type(), DataType::Dictionary(_, _)),
        "level must be dictionary-encoded to save space"
    );

    let service = schema.field(3);
    assert!(
        matches!(service.data_type(), DataType::Dictionary(_, _)),
        "service must be dictionary-encoded to save space"
    );
}

#[test]
fn schema_matches_log_record_wire_fields() {
    let schema = greplog_schema();
    let names: Vec<&str> = schema.fields().iter().map(|field| field.name().as_str()).collect();

    let sample: LogRecord = serde_json::from_value(serde_json::json!({
        "timestamp_us": 1,
        "level": "ERROR",
        "service": "auth-api",
        "message": "boom"
    }))
    .expect("round 1 wire contract must be valid JSON");

    let mut expected = [
        "timestamp_us",
        "trace_id",
        "level",
        "service",
        "message",
        "raw_body",
    ];
    let mut from_record = Vec::new();
    from_record.push("timestamp_us");
    if sample.trace_id.is_some() {
        from_record.push("trace_id");
    }
    from_record.push("level");
    from_record.push("service");
    from_record.push("message");
    if sample.raw_body.is_some() {
        from_record.push("raw_body");
    }

    expected.sort_unstable();
    from_record.sort_unstable();
    for name in &from_record {
        assert!(expected.contains(name), "unexpected field {name}");
    }

    assert_eq!(names.len(), expected.len());
    for name in names {
        assert!(expected.contains(&name), "schema must contain field {name}");
    }
}

#[test]
fn engine_error_converts_from_io_and_arrow() {
    let io_err: EngineError = std::io::Error::new(std::io::ErrorKind::NotFound, "nope").into();
    assert!(matches!(io_err, EngineError::IoError(_)));

    let arrow_err: EngineError = arrow::error::ArrowError::SchemaError("bad".into()).into();
    assert!(matches!(arrow_err, EngineError::ArrowError(_)));
}