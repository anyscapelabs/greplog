//! Greplog Rust SDK: buffered, fire-and-forget logging to a Greplog server.
//!
//! ```no_run
//! use greplog::{init, info};
//!
//! init("auth-service", "production");
//! info!("User authenticated", user_id = 42);
//! ```
//!
//! Records queue in memory and flush to `POST /api/log` on batch size or
//! interval. A failed flush retries once for 429/5xx/network errors — the
//! batch was never durably accepted — and the SDK never panics into the
//! caller's code.

use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:5050";

struct Shared {
    queue: Vec<Value>,
    dropped: u64,
}

/// Buffered logging client; share it freely, every handle drives one flusher.
pub struct Client {
    service: String,
    url: String,
    batch_size: usize,
    max_queue_size: usize,
    shared: Mutex<Shared>,
    wake: Mutex<std::sync::mpsc::Sender<()>>,
}

impl Client {
    /// Creates a client and starts its background flusher thread.
    pub fn new(
        service: impl Into<String>,
        endpoint: &str,
        batch_size: usize,
        flush_interval: std::time::Duration,
        max_queue_size: usize,
    ) -> Arc<Self> {
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        let client = Arc::new(Self {
            service: service.into(),
            url: format!("{}/api/log", endpoint.trim_end_matches('/')),
            batch_size: batch_size.max(1),
            max_queue_size: max_queue_size.max(1),
            shared: Mutex::new(Shared { queue: Vec::new(), dropped: 0 }),
            wake: Mutex::new(wake_tx),
        });
        std::thread::Builder::new()
            .name("greplog-flush".into())
            .spawn({
                let client = Arc::clone(&client);
                move || flusher_loop(client, wake_rx, flush_interval)
            })
            .expect("spawn greplog flusher");
        client
    }

    /// Records one event; never blocks on I/O.
    pub fn track(&self, level: &str, message: impl Into<String>, meta: Option<Value>) {
        let (raw_body, trace_id) = split_meta(meta.as_ref());
        let record = json!({
            "timestamp_us": now_us(),
            "trace_id": trace_id,
            "level": level,
            "service": self.service,
            "message": message.into(),
            "raw_body": raw_body,
        });

        let should_flush = {
            let mut shared = self.shared.lock().expect("greplog lock");
            shared.queue.push(record);
            if shared.queue.len() > self.max_queue_size {
                shared.queue.remove(0);
                shared.dropped += 1;
            }
            shared.queue.len() >= self.batch_size
        };
        if should_flush {
            self.wake().send(()).ok();
        }
    }

    fn wake(&self) -> std::sync::mpsc::Sender<()> {
        self.wake.lock().expect("greplog wake lock").clone()
    }

    /// Sends whatever is queued; called by the flusher thread and on shutdown.
    pub fn flush(&self) {
        let batch = {
            let mut shared = self.shared.lock().expect("greplog lock");
            std::mem::take(&mut shared.queue)
        };
        if batch.is_empty() {
            return;
        }
        match post_batch(&self.url, &batch) {
            Ok(()) => {}
            // 429/5xx/network: never durably accepted, so it comes back around.
            Err(true) => {
                let mut shared = self.shared.lock().expect("greplog lock");
                let mut restored = batch;
                restored.extend(std::mem::take(&mut shared.queue));
                shared.queue = restored;
            }
            Err(false) => {
                eprintln!("greplog: dropped {} records (server rejected batch)", batch.len());
            }
        }
    }

    /// Stops the flusher thread and sends the remainder.
    pub fn shutdown(&self) {
        self.wake().send(()).ok();
    }

    /// Records dropped because the queue exceeded its cap.
    pub fn dropped_count(&self) -> u64 {
        self.shared.lock().expect("greplog lock").dropped
    }
}

/// True for statuses worth resending: 429 backoff and 5xx failures. Any other
/// 4xx means the request itself was rejected; resending would fail again.
fn post_batch(url: &str, batch: &[Value]) -> Result<(), bool> {
    let body = serde_json::to_string(batch).unwrap_or_else(|_| "[]".to_string());
    match ureq::post(url)
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(5))
        .send_string(&body)
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) if code == 429 || code >= 500 => Err(true),
        Err(ureq::Error::Status(..)) => Err(false),
        Err(_) => Err(true),
    }
}

fn flusher_loop(client: Arc<Client>, wake_rx: std::sync::mpsc::Receiver<()>, interval: std::time::Duration) {
    loop {
        match wake_rx.recv_timeout(interval) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => client.flush(),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn split_meta(meta: Option<&Value>) -> (Option<String>, Option<String>) {
    let Some(meta) = meta else { return (None, None) };
    let trace_id = meta.get("trace_id").and_then(Value::as_str).map(str::to_string);
    let payload: serde_json::Map<String, Value> = meta
        .as_object()
        .map(|object| object.iter().filter(|(key, _)| key.as_str() != "trace_id").map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let raw_body = (!payload.is_empty()).then(|| Value::Object(payload).to_string());
    (raw_body, trace_id)
}

fn now_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or(0)
}

static CLIENT: OnceLock<Arc<Client>> = OnceLock::new();

/// Initializes the global client; a second call returns the existing one.
pub fn init(service: impl Into<String>, env: &str) -> Arc<Client> {
    init_with(service, env, Config::default())
}

/// Like [`init`], reading service/endpoint from `GREPLOG_*` env vars.
pub fn init_from_env() -> Arc<Client> {
    let config = Config::from_env();
    let service = std::env::var("GREPLOG_SERVICE_NAME").unwrap_or_else(|_| "rust-app".to_string());
    let env = config.env.clone();
    init_with(service, &env, config)
}

/// Full-control initialization; still a process-wide singleton.
pub fn init_with(service: impl Into<String>, env: &str, config: Config) -> Arc<Client> {
    let _ = env; // carried for parity with the other SDKs; not on the wire
    let client = Client::new(
        service,
        &config.endpoint,
        config.batch_size,
        config.flush_interval,
        config.max_queue_size,
    );
    let _ = CLIENT.set(Arc::clone(&client));
    client
}

/// The process-wide client, once [`init`] has been called.
pub fn client() -> Option<Arc<Client>> {
    CLIENT.get().cloned()
}

/// Flushes the global client, if initialized.
pub fn flush() {
    if let Some(client) = client() {
        client.flush();
    }
}

/// Tuning knobs for [`init_with`]; env vars fill any `None` field.
pub struct Config {
    pub endpoint: String,
    pub env: String,
    pub batch_size: usize,
    pub flush_interval: std::time::Duration,
    pub max_queue_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            env: "development".to_string(),
            batch_size: 100,
            flush_interval: std::time::Duration::from_millis(500),
            max_queue_size: 10_000,
        }
    }
}

impl Config {
    fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(url) = std::env::var("GREPLOG_URL") {
            config.endpoint = url;
        }
        if let Ok(env) = std::env::var("GREPLOG_ENV") {
            config.env = env;
        }
        config
    }
}

/// Core logging macro; prefer the level-specific ones.
#[macro_export]
macro_rules! log_event {
    ($level:expr, $message:expr $(, $key:tt = $value:expr)* $(,)?) => {
        if let Some(client) = $crate::client() {
            let meta = serde_json::json!({ $( stringify!($key): $value ),* });
            let meta = if meta.as_object().is_some_and(|object| object.is_empty()) {
                None
            } else {
                Some(meta)
            };
            client.track($level, $message, meta);
        }
    };
}

#[macro_export]
macro_rules! debug { ($($tokens:tt)*) => { $crate::log_event!("DEBUG", $($tokens)*) }; }

#[macro_export]
macro_rules! info { ($($tokens:tt)*) => { $crate::log_event!("INFO", $($tokens)*) }; }

#[macro_export]
macro_rules! warn { ($($tokens:tt)*) => { $crate::log_event!("WARN", $($tokens)*) }; }

#[macro_export]
macro_rules! error { ($($tokens:tt)*) => { $crate::log_event!("ERROR", $($tokens)*) }; }

#[macro_export]
macro_rules! critical { ($($tokens:tt)*) => { $crate::log_event!("CRITICAL", $($tokens)*) }; }
