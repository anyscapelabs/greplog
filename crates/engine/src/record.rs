//! The strongly-typed data model for incoming log records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;

/// Longest accepted service name: it becomes one Hive partition directory,
/// so it is capped like a filesystem component.
pub const SERVICE_NAME_MAX_LEN: usize = 64;
pub const TRACE_ID_MAX_LEN: usize = 64;
pub const SPAN_ID_MAX_LEN: usize = 64;

#[must_use]
pub fn is_valid_service_name(name: &str) -> bool {
    (1..=SERVICE_NAME_MAX_LEN).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

#[must_use]
pub fn is_valid_trace_id(id: &str) -> bool {
    (1..=TRACE_ID_MAX_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

#[must_use]
pub fn is_valid_span_id(id: &str) -> bool {
    (1..=SPAN_ID_MAX_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

#[must_use]
pub fn is_valid_duration_ms(value: i64) -> bool {
    (0..=86_400_000).contains(&value)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp_us: i64,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub span_id: Option<String>,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    pub level: String,
    pub service: String,
    pub message: String,
    #[serde(default)]
    pub raw_body: Option<String>,
}

impl LogRecord {
    /// Creates a record with the current wall-clock time; ingestion paths set
    /// the exact call-site timestamp instead.
    #[must_use]
    pub fn new(
        level: impl Into<String>,
        service: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_us: Utc::now().timestamp_micros(),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            duration_ms: None,
            level: level.into(),
            service: service.into(),
            message: message.into(),
            raw_body: None,
        }
    }

    pub fn timestamp(&self) -> Result<DateTime<Utc>, EngineError> {
        let seconds = self.timestamp_us.div_euclid(1_000_000);
        let micros = u32::try_from(self.timestamp_us.rem_euclid(1_000_000)).map_err(|_| {
            EngineError::ParseError(format!(
                "timestamp_us {:#} cannot fit in a u32",
                self.timestamp_us
            ))
        })?;
        DateTime::from_timestamp(seconds, micros.saturating_mul(1_000)).ok_or_else(|| {
            EngineError::ParseError(format!(
                "timestamp_us {:#} falls outside the chrono representable range",
                self.timestamp_us
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{is_valid_duration_ms, is_valid_service_name, is_valid_span_id, is_valid_trace_id, LogRecord};
    use serde_json::json;

    const SAMPLE: &str = r#"{
        "timestamp_us": 1723000000123456,
        "trace_id": "job_abc",
        "span_id": "span_1",
        "parent_span_id": "parent_0",
        "duration_ms": 123,
        "level": "ERROR",
        "service": "payment-worker",
        "message": "Payment failed",
        "raw_body": "{\"order_id\":9876,\"reason\":\"card_declined\"}"
    }"#;

    #[test]
    fn round_trips_full_record() {
        // Full record with all optional tracing fields must survive serde round-trip
        let parsed: LogRecord = serde_json::from_str(SAMPLE).expect("sample must deserialize");
        let reserialized = serde_json::to_string(&parsed).expect("record must serialize");
        let reparsed: LogRecord =
            serde_json::from_str(&reserialized).expect("must deserialize again");
        assert_eq!(parsed, reparsed);
        assert_eq!(parsed.trace_id.as_deref(), Some("job_abc"));
        assert_eq!(parsed.span_id.as_deref(), Some("span_1"));
        assert_eq!(parsed.parent_span_id.as_deref(), Some("parent_0"));
        assert_eq!(parsed.duration_ms, Some(123));
        assert_eq!(
            parsed.raw_body.as_deref(),
            Some(r#"{"order_id":9876,"reason":"card_declined"}"#)
        );
    }

    #[test]
    fn missing_optional_fields_default_to_none() {
        // v0.1.0 payloads lacked the new tracing columns — they should default to None, not error
        let json = json!({
            "timestamp_us": 10,
            "level": "INFO",
            "service": "auth-api",
            "message": "signed in"
        });
        let record: LogRecord =
            serde_json::from_value(json).expect("must deserialize without optionals");
        assert_eq!(record.trace_id, None);
        assert_eq!(record.span_id, None);
        assert_eq!(record.parent_span_id, None);
        assert_eq!(record.duration_ms, None);
        assert_eq!(record.raw_body, None);
    }

    #[test]
    fn explicit_nulls_stay_null() {
        let json = json!({
            "timestamp_us": 10,
            "trace_id": null,
            "span_id": null,
            "parent_span_id": null,
            "duration_ms": null,
            "level": "INFO",
            "service": "auth-api",
            "message": "signed in",
            "raw_body": null
        });
        let record: LogRecord = serde_json::from_value(json).expect("must deserialize");
        assert_eq!(record.trace_id, None);
        assert_eq!(record.span_id, None);
        assert_eq!(record.parent_span_id, None);
        assert_eq!(record.duration_ms, None);
    }

    #[test]
    fn timestamp_round_trips_to_utc() {
        let json = json!({
        "timestamp_us": 1_723_000_000_123_456_i64,
            "level": "INFO",
            "service": "auth-api",
            "message": "ok"
        });
        let record: LogRecord = serde_json::from_value(json).expect("must deserialize");
        let dt = record.timestamp().expect("timestamp must convert");
        assert_eq!(dt.timestamp_micros(), 1_723_000_000_123_456_i64);
    }

    #[test]
    fn new_records_have_no_optional_fields() {
        let record = LogRecord::new("INFO", "auth-api", "booted");
        assert!(record.trace_id.is_none());
        assert!(record.span_id.is_none());
        assert!(record.parent_span_id.is_none());
        assert!(record.duration_ms.is_none());
        assert!(record.timestamp_us > 0);
    }

    #[test]
    fn ordinary_service_names_are_accepted() {
        assert!(is_valid_service_name("auth-api"));
        assert!(is_valid_service_name("payment_worker.2"));
        assert!(is_valid_service_name("a"));
        assert!(is_valid_service_name("A-1_2.b"));
        assert!(is_valid_service_name(&"x".repeat(64)));
    }

    #[test]
    fn path_traversing_and_oversized_service_names_are_rejected() {
        // The traversal attempts that motivated the check.
        assert!(!is_valid_service_name("../evil"));
        assert!(!is_valid_service_name("../../../../etc"));
        assert!(!is_valid_service_name("..\\windows"));
        assert!(!is_valid_service_name("a/b"));
        assert!(!is_valid_service_name(""));
        assert!(!is_valid_service_name(&"x".repeat(65)));
        assert!(!is_valid_service_name("has space"));
        assert!(!is_valid_service_name("süß"));
        assert!(!is_valid_service_name("line\nbreak"));
    }

    #[test]
    fn trace_and_span_ids_follow_permissive_charset() {
        // Permissive 1-64 [a-zA-Z0-9._-] allows business IDs like job_abc123, order-65933
        assert!(is_valid_trace_id("job_abc123"));
        assert!(is_valid_trace_id("order-65933"));
        assert!(is_valid_trace_id("trace_1.2-3"));
        assert!(is_valid_span_id("span_1"));
        assert!(is_valid_span_id(&"x".repeat(64)));
        // Hex-only would reject these — we intentionally accept them
        assert!(is_valid_trace_id("abcDEF123_.-"));

        assert!(!is_valid_trace_id(""));
        assert!(!is_valid_trace_id(&"x".repeat(65)));
        assert!(!is_valid_trace_id("has space"));
        assert!(!is_valid_trace_id("a/b"));
        assert!(!is_valid_trace_id("süß"));
        assert!(!is_valid_span_id(""));
        assert!(!is_valid_span_id(&"x".repeat(65)));
        assert!(!is_valid_span_id("a/b"));
    }

    #[test]
    fn duration_ms_range_is_bounded() {
        // Valid range 0..=86_400_000 (24h) prevents negative or absurd latencies from polluting bars
        assert!(is_valid_duration_ms(0));
        assert!(is_valid_duration_ms(123));
        assert!(is_valid_duration_ms(86_400_000));
        assert!(!is_valid_duration_ms(-1));
        assert!(!is_valid_duration_ms(86_400_001));
        assert!(!is_valid_duration_ms(i64::MAX));
        assert!(!is_valid_duration_ms(i64::MIN));
    }

    #[test]
    fn legacy_payload_without_tracing_still_deserializes() {
        // Simulates v0.1.0 WAL/JSON that had only 6 fields — bincode/JSON must not break
        let legacy = json!({
            "timestamp_us": 1723000000123456_i64,
            "trace_id": "job_abc",
            "level": "ERROR",
            "service": "payment-worker",
            "message": "Payment failed",
            "raw_body": null
        });
        let record: LogRecord = serde_json::from_value(legacy).expect("legacy must deserialize");
        assert_eq!(record.span_id, None);
        assert_eq!(record.parent_span_id, None);
        assert_eq!(record.duration_ms, None);
    }
}
