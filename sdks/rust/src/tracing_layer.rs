use std::time::SystemTime;

use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use greplog_core::gen::{LogEvent, Span, SpanKind};

use crate::redact::redact_attributes;
use crate::{generate_ulid, is_initialized, send_event, send_span, service_name};

pub struct GreplogLayer;

impl<S> Layer<S> for GreplogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if !is_initialized() {
            return;
        }

        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target();

        let mut message = String::new();
        let mut attrs = std::collections::HashMap::new();

        let mut visitor = GreplogVisitor::new(&mut message, &mut attrs);
        event.record(&mut visitor);

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        let mut log = LogEvent {
            service_name: service_name(),
            message,
            level,
            timestamp_ns: timestamp,
            logger_name: target.to_string(),
            file: metadata.file().unwrap_or("").to_string(),
            line: metadata.line().unwrap_or(0) as i32,
            ..Default::default()
        };

        log.attributes = redact_attributes(&attrs);
        log.event_id = generate_ulid();

        send_event(log);
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if !is_initialized() {
            return;
        }

        let Some(span_ref) = ctx.span(&id) else {
            return;
        };

        let metadata = span_ref.metadata();
        let name = metadata.name().to_string();
        let _target = metadata.target().to_string();

        let attrs = std::collections::HashMap::new();
        let extensions = span_ref.extensions();

        if let Some(data) = extensions.get::<SpanTiming>() {
            let timestamp = data.start_ns;
            let end_ns = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64;

            let span = Span {
                service_name: service_name(),
                name,
                start_time_ns: timestamp,
                end_time_ns: end_ns,
                route: String::new(),
                method: String::new(),
                status_code: 0,
                kind: SpanKind::Internal as i32,
                attributes: redact_attributes(&attrs),
                event_id: generate_ulid(),
                ..Default::default()
            };

            send_span(span);
        }

        drop(extensions);
    }

    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if !is_initialized() {
            return;
        }

        let Some(span_ref) = ctx.span(id) else {
            return;
        };

        let start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        span_ref
            .extensions_mut()
            .insert(SpanTiming { start_ns: start });
    }
}

pub struct SpanTiming {
    pub start_ns: i64,
}

struct GreplogVisitor<'a> {
    message: &'a mut String,
    attrs: &'a mut std::collections::HashMap<String, String>,
}

impl<'a> GreplogVisitor<'a> {
    fn new(
        message: &'a mut String,
        attrs: &'a mut std::collections::HashMap<String, String>,
    ) -> Self {
        GreplogVisitor { message, attrs }
    }
}

impl<'a> tracing::field::Visit for GreplogVisitor<'a> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message.push_str(value);
        } else {
            self.attrs
                .insert(field.name().to_string(), value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        if field.name() == "message" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message.push_str(&s);
        } else {
            self.attrs.insert(field.name().to_string(), s);
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.attrs
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.attrs
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.attrs
            .insert(field.name().to_string(), value.to_string());
    }
}
