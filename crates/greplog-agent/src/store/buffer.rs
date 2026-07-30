use anyhow::Result;
use arrow::array::*;
use arrow::record_batch::RecordBatch;
use greplog_core::arrow_schema;
use greplog_core::gen::{LogEvent, Metric, Span};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-table Arrow ArrayBuilders
// ---------------------------------------------------------------------------

pub struct LogBuilders {
    pub id: StringBuilder,
    pub service: StringBuilder,
    pub timestamp: TimestampMicrosecondBuilder,
    pub level: StringBuilder,
    pub route: StringBuilder,
    pub message: StringBuilder,
    pub attributes: StringBuilder,
}

pub struct SpanBuilders {
    pub id: StringBuilder,
    pub correlation_id: StringBuilder,
    pub parent_correlation_id: StringBuilder,
    pub service: StringBuilder,
    pub name: StringBuilder,
    pub route: StringBuilder,
    pub method: StringBuilder,
    pub status_code: Int32Builder,
    pub latency_ms: Float64Builder,
    pub is_error: BooleanBuilder,
    pub start_time: TimestampMicrosecondBuilder,
    pub end_time: TimestampMicrosecondBuilder,
    pub attributes: StringBuilder,
}

pub struct MetricBuilders {
    pub id: StringBuilder,
    pub service: StringBuilder,
    pub name: StringBuilder,
    pub timestamp: TimestampMicrosecondBuilder,
    pub value: Float64Builder,
    pub labels: StringBuilder,
}

// ---------------------------------------------------------------------------
// LogBuilders
// ---------------------------------------------------------------------------

impl Default for LogBuilders {
    fn default() -> Self {
        Self::new()
    }
}

impl LogBuilders {
    pub fn new() -> Self {
        Self {
            id: StringBuilder::new(),
            service: StringBuilder::new(),
            timestamp: TimestampMicrosecondBuilder::new(),
            level: StringBuilder::new(),
            route: StringBuilder::new(),
            message: StringBuilder::new(),
            attributes: StringBuilder::new(),
        }
    }

    pub fn append(&mut self, log: &LogEvent, service: &str) {
        let id = if log.event_id.is_empty() {
            super::flush::log_content_id(log)
        } else {
            log.event_id.clone()
        };
        self.id.append_value(&id);
        self.service.append_value(service);

        let ts = super::flush::ns_to_micros(log.timestamp_ns);
        self.timestamp.append_value(ts);

        match super::flush::non_empty(&log.level) {
            Some(v) => self.level.append_value(&v),
            None => self.level.append_null(),
        }
        self.route.append_null();

        match super::flush::non_empty(&log.message) {
            Some(v) => self.message.append_value(&v),
            None => self.message.append_null(),
        }

        if log.attributes.is_empty() {
            self.attributes.append_null();
        } else {
            let json = serde_json::to_string(&log.attributes).unwrap_or_default();
            self.attributes.append_value(&json);
        }
    }

    pub fn finish(&mut self) -> Result<RecordBatch> {
        let schema = arrow_schema::log_schema();
        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(self.id.finish()),
                Arc::new(self.service.finish()),
                Arc::new(self.timestamp.finish()),
                Arc::new(self.level.finish()),
                Arc::new(self.route.finish()),
                Arc::new(self.message.finish()),
                Arc::new(self.attributes.finish()),
            ],
        )
        .map_err(|e| anyhow::anyhow!("finish log batch: {e}"))
    }
}

// ---------------------------------------------------------------------------
// SpanBuilders
// ---------------------------------------------------------------------------

impl Default for SpanBuilders {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanBuilders {
    pub fn new() -> Self {
        Self {
            id: StringBuilder::new(),
            correlation_id: StringBuilder::new(),
            parent_correlation_id: StringBuilder::new(),
            service: StringBuilder::new(),
            name: StringBuilder::new(),
            route: StringBuilder::new(),
            method: StringBuilder::new(),
            status_code: Int32Builder::new(),
            latency_ms: Float64Builder::new(),
            is_error: BooleanBuilder::new(),
            start_time: TimestampMicrosecondBuilder::new(),
            end_time: TimestampMicrosecondBuilder::new(),
            attributes: StringBuilder::new(),
        }
    }

    pub fn append(&mut self, span: &Span, service: &str) {
        let id = if span.event_id.is_empty() {
            super::flush::span_content_id(span)
        } else {
            span.event_id.clone()
        };
        self.id.append_value(&id);
        self.service.append_value(service);

        match super::flush::non_empty(&span.correlation_id) {
            Some(v) => self.correlation_id.append_value(&v),
            None => self.correlation_id.append_null(),
        }
        match super::flush::non_empty(&span.parent_correlation_id) {
            Some(v) => self.parent_correlation_id.append_value(&v),
            None => self.parent_correlation_id.append_null(),
        }
        match super::flush::non_empty(&span.name) {
            Some(v) => self.name.append_value(&v),
            None => self.name.append_null(),
        }

        match super::flush::non_empty(&span.route) {
            Some(v) => self.route.append_value(&v),
            None => self.route.append_null(),
        }
        match super::flush::non_empty(&span.method) {
            Some(v) => self.method.append_value(&v),
            None => self.method.append_null(),
        }
        if span.status_code != 0 {
            self.status_code.append_value(span.status_code);
        } else {
            self.status_code.append_null();
        }
        let latency_ms = (span.end_time_ns - span.start_time_ns) as f64 / 1_000_000.0;
        self.latency_ms.append_value(latency_ms);
        self.is_error.append_value(!span.error.is_empty());
        self.start_time
            .append_value(super::flush::ns_to_micros(span.start_time_ns));
        self.end_time
            .append_value(super::flush::ns_to_micros(span.end_time_ns));

        if span.attributes.is_empty() {
            self.attributes.append_null();
        } else {
            let json = serde_json::to_string(&span.attributes).unwrap_or_default();
            self.attributes.append_value(&json);
        }
    }

    pub fn finish(&mut self) -> Result<RecordBatch> {
        let schema = arrow_schema::span_schema();
        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(self.id.finish()),
                Arc::new(self.correlation_id.finish()),
                Arc::new(self.parent_correlation_id.finish()),
                Arc::new(self.service.finish()),
                Arc::new(self.name.finish()),
                Arc::new(self.route.finish()),
                Arc::new(self.method.finish()),
                Arc::new(self.status_code.finish()),
                Arc::new(self.latency_ms.finish()),
                Arc::new(self.is_error.finish()),
                Arc::new(self.start_time.finish()),
                Arc::new(self.end_time.finish()),
                Arc::new(self.attributes.finish()),
            ],
        )
        .map_err(|e| anyhow::anyhow!("finish span batch: {e}"))
    }
}

// ---------------------------------------------------------------------------
// MetricBuilders
// ---------------------------------------------------------------------------

impl Default for MetricBuilders {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricBuilders {
    pub fn new() -> Self {
        Self {
            id: StringBuilder::new(),
            service: StringBuilder::new(),
            name: StringBuilder::new(),
            timestamp: TimestampMicrosecondBuilder::new(),
            value: Float64Builder::new(),
            labels: StringBuilder::new(),
        }
    }

    pub fn append(&mut self, metric: &Metric, service: &str) {
        let id = if metric.event_id.is_empty() {
            super::flush::metric_content_id(metric)
        } else {
            metric.event_id.clone()
        };
        self.id.append_value(&id);
        self.service.append_value(service);
        self.name.append_value(&metric.name);
        self.timestamp
            .append_value(super::flush::ns_to_micros(metric.timestamp_ns));
        self.value.append_value(metric.value);

        if metric.labels.is_empty() {
            self.labels.append_null();
        } else {
            let json = serde_json::to_string(&metric.labels).unwrap_or_default();
            self.labels.append_value(&json);
        }
    }

    pub fn finish(&mut self) -> Result<RecordBatch> {
        let schema = arrow_schema::metric_schema();
        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(self.id.finish()),
                Arc::new(self.service.finish()),
                Arc::new(self.name.finish()),
                Arc::new(self.timestamp.finish()),
                Arc::new(self.value.finish()),
                Arc::new(self.labels.finish()),
            ],
        )
        .map_err(|e| anyhow::anyhow!("finish metric batch: {e}"))
    }
}
