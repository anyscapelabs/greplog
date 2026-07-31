//! # Greplog — Rust SDK
//!
//! Observability agent client for Rust applications.
//!
//! ## Quick start
//!
//! ```ignore
//! use greplog;
//!
//! greplog::init();
//! greplog::info!("server started");
//! greplog::error!("db connection failed", &[("host", "prod-1")]);
//! ```
//!
//! ## What `init()` covers
//!
//! - **Automatic panic capture**: a `std::panic::set_hook` is installed —
//!   any uncaught panic is forwarded to the agent before the process exits.
//! - **Automatic tracing capture**: a `tracing_subscriber::Layer` is
//!   registered — any app already emitting spans/events via the `tracing`
//!   crate (including `tower_http::TraceLayer` in axum apps) has those
//!   events captured automatically.
//! - **Manual logging macros**: `info!`, `warn!`, `error!`, `debug!` work
//!   regardless of whether `init()` has been called (fail-open).
//! - **Transport**: UDS/TCP dual-transport to the local greplog agent.
//!
//! ## What `init()` does NOT cover (honest constraints)
//!
//! - **Non-tracing HTTP middleware**: if your axum app does not already use
//!   `tracing` (or `tower_http::TraceLayer`) to emit HTTP spans, you must
//!   add the axum middleware explicitly:
//!
//!   ```ignore
//!   use tower::ServiceBuilder;
//!
//!   Router::new()
//!       .route("/", get(handler))
//!       .layer(ServiceBuilder::new().layer(greplog::axum_layer()));
//!   ```
//!
//!   Without this, HTTP request/response data is not captured — the
//!   `tracing` layer in `init()` captures *tracing spans/events only*,
//!   not raw HTTP requests.
//!
//! All public functions are fail-open: if the agent is unreachable, calls
//! are silently dropped — your application will never panic due to a
//! transport failure.

mod axum_layer;
mod redact;
mod tracing_layer;
mod transport;
mod ulid;

pub use axum_layer::GreplogAxumLayer;
pub use greplog_core::gen;
pub use tracing_layer::GreplogLayer;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use greplog_core::gen::{IngestBatch, LogEvent, Span};

use redact::redact_attributes;
use transport::Transport;
use ulid::generate_ulid;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<State>> = Mutex::new(None);

struct State {
    transport: Transport,
    batch_seq: i64,
    service_name: String,
    instance_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

pub(crate) fn service_name() -> String {
    let lock = STATE.lock().unwrap();
    lock.as_ref()
        .map(|s| s.service_name.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::SeqCst)
}

pub(crate) fn send_event(event: LogEvent) {
    let lock = STATE.lock().unwrap();
    let state = match lock.as_ref() {
        Some(s) => s,
        None => return,
    };

    let batch = IngestBatch {
        service_name: state.service_name.clone(),
        instance_id: state.instance_id.clone(),
        batch_seq: state.batch_seq,
        logs: vec![event],
        spans: vec![],
        ..Default::default()
    };

    let mut data = Vec::new();
    if prost::Message::encode(&batch, &mut data).is_ok() {
        state.transport.send(data);
    }
}

pub(crate) fn send_span(span: Span) {
    let lock = STATE.lock().unwrap();
    let state = match lock.as_ref() {
        Some(s) => s,
        None => return,
    };

    let batch = IngestBatch {
        service_name: state.service_name.clone(),
        instance_id: state.instance_id.clone(),
        batch_seq: state.batch_seq,
        logs: vec![],
        spans: vec![span],
        ..Default::default()
    };

    let mut data = Vec::new();
    if prost::Message::encode(&batch, &mut data).is_ok() {
        state.transport.send(data);
    }
}

pub fn manual_log(level: Level, message: &str, details: &[(&str, &str)]) {
    if !is_initialized() {
        return;
    }

    let mut attrs = HashMap::new();
    for (k, v) in details {
        attrs.insert(k.to_string(), v.to_string());
    }
    attrs = redact_attributes(&attrs);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64;

    let event = LogEvent {
        service_name: service_name(),
        message: message.to_string(),
        level: level.as_str().to_string(),
        timestamp_ns: timestamp,
        logger_name: "greplog".to_string(),
        file: String::new(),
        line: 0,
        attributes: attrs,
        event_id: generate_ulid(),
        ..Default::default()
    };

    send_event(event);
}

#[macro_export]
macro_rules! error {
    ($msg:expr) => {
        $crate::manual_log($crate::Level::Error, $msg, &[])
    };
    ($msg:expr, $details:expr) => {
        $crate::manual_log($crate::Level::Error, $msg, $details)
    };
}

#[macro_export]
macro_rules! warn {
    ($msg:expr) => {
        $crate::manual_log($crate::Level::Warn, $msg, &[])
    };
    ($msg:expr, $details:expr) => {
        $crate::manual_log($crate::Level::Warn, $msg, $details)
    };
}

#[macro_export]
macro_rules! info {
    ($msg:expr) => {
        $crate::manual_log($crate::Level::Info, $msg, &[])
    };
    ($msg:expr, $details:expr) => {
        $crate::manual_log($crate::Level::Info, $msg, $details)
    };
}

#[macro_export]
macro_rules! debug {
    ($msg:expr) => {
        $crate::manual_log($crate::Level::Debug, $msg, &[])
    };
    ($msg:expr, $details:expr) => {
        $crate::manual_log($crate::Level::Debug, $msg, $details)
    };
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub service: Option<String>,
    pub socket_path: Option<String>,
    pub tcp_host: Option<String>,
    pub tcp_port: Option<u16>,
}

pub fn init() {
    init_with_config(Config::default());
}

pub fn init_with_config(config: Config) {
    init_internal(
        config.service.as_deref(),
        config.socket_path.as_deref(),
        config.tcp_host.as_deref(),
        config.tcp_port,
    );
}

pub fn init_with_opts(socket_path: Option<&str>, tcp_host: Option<&str>, tcp_port: Option<u16>) {
    init_internal(None, socket_path, tcp_host, tcp_port);
}

fn init_internal(
    service: Option<&str>,
    socket_path: Option<&str>,
    tcp_host: Option<&str>,
    tcp_port: Option<u16>,
) {
    if INITIALIZED.load(Ordering::SeqCst) {
        return;
    }

    let env_port = std::env::var("GREPLOG_TEST_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let env_host = std::env::var("GREPLOG_TEST_HOST").ok();
    let env_sock = std::env::var("GREPLOG_TEST_SOCKET").ok();

    let port = tcp_port.or(env_port);
    let host = tcp_host.or(env_host.as_deref());
    let sock = socket_path.or(env_sock.as_deref());
    let svc_name = service.unwrap_or("greplog-rust").to_string();

    let transport = Transport::new(sock, host, port, None);

    {
        let mut lock = STATE.lock().unwrap();
        *lock = Some(State {
            transport,
            batch_seq: 0,
            service_name: svc_name,
            instance_id: generate_ulid(),
        });
    }

    INITIALIZED.store(true, Ordering::SeqCst);

    install_panic_hook();
    install_tracing_layer();
}

#[doc(hidden)]
pub fn test_reset() {
    INITIALIZED.store(false, Ordering::SeqCst);
    let mut lock = STATE.lock().unwrap();
    *lock = None;
}

fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info.to_string();
        let (file, line) = if let Some(loc) = info.location() {
            (loc.file().to_string(), loc.line() as i32)
        } else {
            (String::new(), 0)
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        let loc_str = std::panic::Location::caller().to_string();

        let event = LogEvent {
            service_name: service_name(),
            message: message.clone(),
            level: "error".to_string(),
            timestamp_ns: timestamp,
            logger_name: "greplog".to_string(),
            file,
            line,
            exception_type: "panic".to_string(),
            exception_message: message,
            stack_trace: vec![loc_str],
            event_id: generate_ulid(),
            ..Default::default()
        };

        send_event(event);

        prev(info);
    }));
}

fn install_tracing_layer() {
    use tracing_subscriber::prelude::*;

    let layer = GreplogLayer;

    let _ = tracing_subscriber::registry().with(layer).try_init();
}

pub fn axum_layer() -> GreplogAxumLayer {
    GreplogAxumLayer
}
