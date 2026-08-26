//! The strongly-typed data model for incoming log records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;

/// Longest accepted service name: it becomes one Hive partition directory,
/// so it is capped like a filesystem component.
pub const SERVICE_NAME_MAX_LEN: usize = 64;

/// True when `name` is safe to turn into a `service=<name>` partition path:
/// 1–64 characters of ASCII letters, digits, `_`, `.` or `-`.
///
/// The ingest boundary rejects everything else. A name containing `/` would
/// nest directories outside the service layout, and one containing `..`
/// could escape the data directory entirely, so path safety — not style —
/// is what this check guards.
#[must_use]
pub fn is_valid_service_name(name: &str) -> bool {
    (1..=SERVICE_NAME_MAX_LEN).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp_us: i64,
    pub trace_id: Option<String>,
    pub level: String,
    pub service: String,
    pub message: String,
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
    use super::{is_valid_service_name, LogRecord};
    use serde_json::json;

    const SAMPLE: &str = r#"{
        "timestamp_us": 1723000000123456,
        "trace_id": "job_abc",
        "level": "ERROR",
        "service": "payment-worker",
        "message": "Payment failed",
        "raw_body": "{\"order_id\":9876,\"reason\":\"card_declined\"}"
    }"#;

    #[test]
    fn round_trips_full_record() {
        let parsed: LogRecord = serde_json::from_str(SAMPLE).expect("sample must deserialize");
        let reserialized = serde_json::to_string(&parsed).expect("record must serialize");

        let reparsed: LogRecord =
            serde_json::from_str(&reserialized).expect("must deserialize again");
        assert_eq!(parsed, reparsed);
        assert_eq!(parsed.trace_id.as_deref(), Some("job_abc"));
        assert_eq!(
            parsed.raw_body.as_deref(),
            Some(r#"{"order_id":9876,"reason":"card_declined"}"#)
        );
    }

    #[test]
    fn missing_optional_fields_default_to_none() {
        let json = json!({
            "timestamp_us": 10,
            "level": "INFO",
            "service": "auth-api",
            "message": "signed in"
        });

        let record: LogRecord =
            serde_json::from_value(json).expect("must deserialize without optionals");
        assert_eq!(record.trace_id, None);
        assert_eq!(record.raw_body, None);
    }

    #[test]
    fn explicit_nulls_stay_null() {
        let json = json!({
            "timestamp_us": 10,
            "trace_id": null,
            "level": "INFO",
            "service": "auth-api",
            "message": "signed in",
            "raw_body": null
        });

        let record: LogRecord = serde_json::from_value(json).expect("must deserialize");
        assert_eq!(record.trace_id, None);
        assert_eq!(record.raw_body, None);
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
        assert!(record.raw_body.is_none());
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

        // Empty, oversized, and non-path-safe characters.
        assert!(!is_valid_service_name(""));
        assert!(!is_valid_service_name(&"x".repeat(65)));
        assert!(!is_valid_service_name("has space"));
        assert!(!is_valid_service_name("süß"));
        assert!(!is_valid_service_name("line\nbreak"));
    }
}
