use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use greplog_core::gen::{LogEvent, Metric, Span};
use std::sync::Arc;

pub fn log_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("level", DataType::Utf8, true),
        Field::new("route", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, true),
        Field::new("logger_name", DataType::Utf8, true),
        Field::new("file", DataType::Utf8, true),
        Field::new("line", DataType::Int32, true),
        Field::new("correlation_id", DataType::Utf8, true),
        Field::new("attributes", DataType::Utf8, true),
        Field::new("service_name", DataType::Utf8, false),
        Field::new("date", DataType::Utf8, false),
    ])
}

pub fn span_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("correlation_id", DataType::Utf8, true),
        Field::new("parent_correlation_id", DataType::Utf8, true),
        Field::new("name", DataType::Utf8, true),
        Field::new("route", DataType::Utf8, true),
        Field::new("method", DataType::Utf8, true),
        Field::new("status_code", DataType::Int32, true),
        Field::new("latency_ms", DataType::Float64, true),
        Field::new("is_error", DataType::Boolean, true),
        Field::new("start_time", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("end_time", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("attributes", DataType::Utf8, true),
        Field::new("service_name", DataType::Utf8, false),
        Field::new("date", DataType::Utf8, false),
    ])
}

pub fn metric_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("value", DataType::Float64, false),
        Field::new("labels", DataType::Utf8, true),
        Field::new("service_name", DataType::Utf8, false),
        Field::new("date", DataType::Utf8, false),
    ])
}

pub fn log_table_name() -> &'static str { "logs" }
pub fn span_table_name() -> &'static str { "spans" }
pub fn metric_table_name() -> &'static str { "metrics" }

pub fn date_partition_key() -> &'static str { "date" }
pub fn service_partition_key() -> &'static str { "service_name" }

pub fn partition_columns() -> &'static [&'static str] {
    &["service_name", "date"]
}

pub fn ns_to_micros(ns: i64) -> i64 {
    ns / 1000
}

fn utc_date_string() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let y = 1970;
    let mut remaining = days as i64;
    let mut year = y as i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            month = i + 1;
            break;
        }
        remaining -= md;
    }
    if month == 0 { month = 12; remaining = 31; }
    format!("{:04}-{:02}-{:02}", year, month, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn log_partition_path(base: &std::path::Path, table_type: &str, service: &str, date: &str) -> std::path::PathBuf {
    base
        .join(format!("table_type={}", table_type))
        .join(format!("service_name={}", service))
        .join(format!("date={}", date))
}

pub struct LogBatchBuilder {
    pub ids: Vec<String>,
    pub timestamps: Vec<i64>,
    pub levels: Vec<Option<String>>,
    pub routes: Vec<Option<String>>,
    pub messages: Vec<Option<String>>,
    pub logger_names: Vec<Option<String>>,
    pub files: Vec<Option<String>>,
    pub lines: Vec<Option<i32>>,
    pub correlation_ids: Vec<Option<String>>,
    pub attributes: Vec<Option<String>>,
    pub service_names: Vec<String>,
    pub dates: Vec<String>,
    pub service: String,
    pub date_str: String,
}

impl LogBatchBuilder {
    pub fn new(service: &str) -> Self {
        let date_str = utc_date_string();
        Self {
            ids: Vec::new(),
            timestamps: Vec::new(),
            levels: Vec::new(),
            routes: Vec::new(),
            messages: Vec::new(),
            logger_names: Vec::new(),
            files: Vec::new(),
            lines: Vec::new(),
            correlation_ids: Vec::new(),
            attributes: Vec::new(),
            service_names: Vec::new(),
            dates: Vec::new(),
            service: service.to_string(),
            date_str,
        }
    }

    pub fn push(&mut self, log: &LogEvent, id: &str, attrs_json: &str) {
        self.ids.push(id.to_string());
        self.timestamps.push(ns_to_micros(log.timestamp_ns));
        self.levels.push(non_empty(&log.level));
        self.routes.push(None); // set externally via attrs.remove
        self.messages.push(non_empty(&log.message));
        self.logger_names.push(non_empty(&log.logger_name));
        self.files.push(non_empty(&log.file));
        self.lines.push(if log.line != 0 { Some(log.line) } else { None });
        self.correlation_ids.push(non_empty(&log.correlation_id));
        self.attributes.push(non_empty(attrs_json));
        self.service_names.push(self.service.clone());
        self.dates.push(self.date_str.clone());
    }

    pub fn set_route(&mut self, idx: usize, route: Option<String>) {
        self.routes[idx] = route;
    }

    pub fn is_empty(&self) -> bool { self.ids.is_empty() }

    pub fn build(&self) -> Option<RecordBatch> {
        if self.ids.is_empty() { return None; }
        let len = self.ids.len();
        let batch = RecordBatch::try_new(
            Arc::new(log_schema()),
            vec![
                Arc::new(StringArray::from(self.ids.clone())),
                Arc::new(TimestampMicrosecondArray::from(self.timestamps.clone())),
                Arc::new(StringArray::from_iter(self.levels.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.routes.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.messages.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.logger_names.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.files.iter().map(|x| x.as_deref()))),
                Arc::new(Int32Array::from_iter(self.lines.iter().map(|x| *x))),
                Arc::new(StringArray::from_iter(self.correlation_ids.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.attributes.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from(self.service_names.clone())),
                Arc::new(StringArray::from(self.dates.clone())),
            ],
        ).ok()?;
        Some(batch)
    }
}

pub struct SpanBatchBuilder {
    pub ids: Vec<String>,
    pub correlation_ids: Vec<Option<String>>,
    pub parent_correlation_ids: Vec<Option<String>>,
    pub names: Vec<Option<String>>,
    pub routes: Vec<Option<String>>,
    pub methods: Vec<Option<String>>,
    pub status_codes: Vec<Option<i32>>,
    pub latency_ms: Vec<Option<f64>>,
    pub is_error: Vec<Option<bool>>,
    pub start_times: Vec<i64>,
    pub end_times: Vec<i64>,
    pub attributes: Vec<Option<String>>,
    pub service_names: Vec<String>,
    pub dates: Vec<String>,
    pub service: String,
    pub date_str: String,
}

impl SpanBatchBuilder {
    pub fn new(service: &str) -> Self {
        let date_str = utc_date_string();
        Self {
            ids: Vec::new(),
            correlation_ids: Vec::new(),
            parent_correlation_ids: Vec::new(),
            names: Vec::new(),
            routes: Vec::new(),
            methods: Vec::new(),
            status_codes: Vec::new(),
            latency_ms: Vec::new(),
            is_error: Vec::new(),
            start_times: Vec::new(),
            end_times: Vec::new(),
            attributes: Vec::new(),
            service_names: Vec::new(),
            dates: Vec::new(),
            service: service.to_string(),
            date_str,
        }
    }

    pub fn push(&mut self, span: &Span, id: &str, attrs_json: &str,
                route: &str, method: &str, status_code: Option<i32>,
                latency: Option<f64>, is_err: bool) {
        self.ids.push(id.to_string());
        self.correlation_ids.push(non_empty(&span.correlation_id));
        self.parent_correlation_ids.push(non_empty(&span.parent_correlation_id));
        self.names.push(non_empty(&span.name));
        self.routes.push(non_empty(route));
        self.methods.push(non_empty(method));
        self.status_codes.push(status_code);
        self.latency_ms.push(latency);
        self.is_error.push(Some(is_err));
        self.start_times.push(ns_to_micros(span.start_time_ns));
        self.end_times.push(ns_to_micros(span.end_time_ns));
        self.attributes.push(non_empty(attrs_json));
        self.service_names.push(self.service.clone());
        self.dates.push(self.date_str.clone());
    }

    pub fn is_empty(&self) -> bool { self.ids.is_empty() }

    pub fn build(&self) -> Option<RecordBatch> {
        if self.ids.is_empty() { return None; }
        let batch = RecordBatch::try_new(
            Arc::new(span_schema()),
            vec![
                Arc::new(StringArray::from(self.ids.clone())),
                Arc::new(StringArray::from_iter(self.correlation_ids.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.parent_correlation_ids.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.names.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.routes.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from_iter(self.methods.iter().map(|x| x.as_deref()))),
                Arc::new(Int32Array::from_iter(self.status_codes.iter().map(|x| *x))),
                Arc::new(Float64Array::from_iter(self.latency_ms.iter().map(|x| *x))),
                Arc::new(BooleanArray::from_iter(self.is_error.iter().map(|x| *x))),
                Arc::new(TimestampMicrosecondArray::from(self.start_times.clone())),
                Arc::new(TimestampMicrosecondArray::from(self.end_times.clone())),
                Arc::new(StringArray::from_iter(self.attributes.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from(self.service_names.clone())),
                Arc::new(StringArray::from(self.dates.clone())),
            ],
        ).ok()?;
        Some(batch)
    }
}

pub struct MetricBatchBuilder {
    pub ids: Vec<String>,
    pub names: Vec<String>,
    pub timestamps: Vec<i64>,
    pub values: Vec<f64>,
    pub labels: Vec<Option<String>>,
    pub service_names: Vec<String>,
    pub dates: Vec<String>,
    pub service: String,
    pub date_str: String,
}

impl MetricBatchBuilder {
    pub fn new(service: &str) -> Self {
        let date_str = utc_date_string();
        Self {
            ids: Vec::new(),
            names: Vec::new(),
            timestamps: Vec::new(),
            values: Vec::new(),
            labels: Vec::new(),
            service_names: Vec::new(),
            dates: Vec::new(),
            service: service.to_string(),
            date_str,
        }
    }

    pub fn push(&mut self, metric: &Metric, id: &str, labels_json: &str) {
        self.ids.push(id.to_string());
        self.names.push(metric.name.clone());
        self.timestamps.push(ns_to_micros(metric.timestamp_ns));
        self.values.push(metric.value);
        self.labels.push(non_empty(labels_json));
        self.service_names.push(self.service.clone());
        self.dates.push(self.date_str.clone());
    }

    pub fn is_empty(&self) -> bool { self.ids.is_empty() }

    pub fn build(&self) -> Option<RecordBatch> {
        if self.ids.is_empty() { return None; }
        let batch = RecordBatch::try_new(
            Arc::new(metric_schema()),
            vec![
                Arc::new(StringArray::from(self.ids.clone())),
                Arc::new(StringArray::from(self.names.clone())),
                Arc::new(TimestampMicrosecondArray::from(self.timestamps.clone())),
                Arc::new(Float64Array::from(self.values.clone())),
                Arc::new(StringArray::from_iter(self.labels.iter().map(|x| x.as_deref()))),
                Arc::new(StringArray::from(self.service_names.clone())),
                Arc::new(StringArray::from(self.dates.clone())),
            ],
        ).ok()?;
        Some(batch)
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn non_empty_ref(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}
