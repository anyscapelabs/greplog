use crate::gen;

/// Known attribute keys for well-known log/span attributes.
pub mod keys {
    pub const SERVICE_NAME: &str = "service.name";
    pub const DEPLOYMENT_ENV: &str = "deployment.environment";
    pub const HOSTNAME: &str = "host.hostname";

    pub const HTTP_METHOD: &str = "http.method";
    pub const HTTP_ROUTE: &str = "http.route";
    pub const HTTP_STATUS_CODE: &str = "http.status_code";
    pub const HTTP_REQUEST_BODY: &str = "http.request.body";
    pub const HTTP_RESPONSE_BODY: &str = "http.response.body";
    pub const HTTP_LATENCY_MS: &str = "http.latency_ms";

    pub const DB_SYSTEM: &str = "db.system";
    pub const DB_STATEMENT: &str = "db.statement";
    pub const DB_OPERATION: &str = "db.operation";

    pub const EXCEPTION_TYPE: &str = "exception.type";
    pub const EXCEPTION_MESSAGE: &str = "exception.message";
    pub const EXCEPTION_STACKTRACE: &str = "exception.stacktrace";
}

/// Standard log level values.
pub mod levels {
    pub const TRACE: &str = "trace";
    pub const DEBUG: &str = "debug";
    pub const INFO: &str = "info";
    pub const WARN: &str = "warn";
    pub const ERROR: &str = "error";
    pub const FATAL: &str = "fatal";
}

impl gen::LogEvent {
    pub fn is_error(&self) -> bool {
        matches!(self.level.as_str(), "error" | "fatal")
    }
}

impl gen::Span {
    pub fn duration_ns(&self) -> i64 {
        self.end_time_ns - self.start_time_ns
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration_ns() as f64 / 1_000_000.0
    }

    pub fn is_http(&self) -> bool {
        !self.method.is_empty() || !self.route.is_empty()
    }
}

impl gen::Metric {
    pub fn is_counter(&self) -> bool {
        self.r#type == gen::MetricType::Counter as i32
    }

    pub fn is_gauge(&self) -> bool {
        self.r#type == gen::MetricType::Gauge as i32
    }

    pub fn is_histogram(&self) -> bool {
        self.r#type == gen::MetricType::Histogram as i32
    }
}
