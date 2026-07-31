# SDK API Reference

> Status: ✅ Shipped (v0.1.0)

Covers all 4 first-party SDKs: **Node** (`greplog` 0.1.0, Node >=18), **Rust** (`greplog` 0.1.0, edition 2021), **Go** (`github.com/greplog/greplog-go`), **Python** (`greplog` 0.1.0, >=3.8).

---

## 1. Init

### Node

```ts
import { greplog } from 'greplog';

greplog.init();
// or with options:
greplog.init({ service: 'my-app', captureBodies: true });
```

| Option | Type | Default |
|--------|------|---------|
| `serviceName` / `service` | `string` | Detected from `package.json` name |
| `socketPath` | `string` | `'.greplog/greplog.sock'` |
| `tcpPort` | `number` | `4318` |
| `captureBodies` | `boolean` | `false` |
| `captureConsoleLevel` | `'debug' \| 'info' \| 'warn' \| 'error'` | `'warn'` |

### Rust

```rust
greplog::init();
// or with config options:
greplog::init_with_config(greplog::Config { service: Some("my-app".into()), ..Default::default() });
```

- `init()` calls `init_with_config(Config::default())`.
- Default service name is `"greplog-rust"` (overrideable via `Config.service`).
- Reads env var overrides: `GREPLOG_TEST_PORT`, `GREPLOG_TEST_HOST`, `GREPLOG_TEST_SOCKET`.

### Go

```go
import greplog "github.com/greplog/greplog-go/src"

greplog.Init()
// or with options:
greplog.Init(greplog.Config{Service: "my-service"})
```

| Option | Type | Default |
|--------|------|---------|
| `ServiceName` / `Service` | `string` | Detected from `go.mod` module path |
| `SocketPath` | `string` | `'.greplog/greplog.sock'` |
| `TCPHost` | `string` | `'127.0.0.1'` |
| `TCPPort` | `int` | `4318` |
| `CaptureBodies` | `bool` | `false` |
| `ReconnectDelay` | `time.Duration` | `5 * time.Second` |
| `PanicPolicy` | `PanicPolicy` | `PanicPolicyLog` |

### Python

```python
import greplog

greplog.init()
# with options:
greplog.init(service="my-app", capture_bodies=True)
```

| Option | Type | Default |
|--------|------|---------|
| `service_name` | `Optional[str]` | Detected from `pyproject.toml` / `setup.py` |
| `socket_path` | `Optional[str]` | `'.greplog/greplog.sock'` |
| `tcp_port` | `Optional[int]` | `4318` |
| `capture_bodies` | `bool` | `False` |
| `capture_log_level` | `str` | `"WARNING"` |
| `app` | `Any` | `None` (Flask/FastAPI app to wrap automatically) |

---

## 2. Shutdown

All 4 SDKs provide a shutdown for graceful teardown:

| SDK | Function |
|-----|----------|
| Node | `shutdown()` |
| Rust | (transport dropped on drop; no explicit shutdown fn) |
| Go | `Shutdown()` |
| Python | `shutdown()` |

---

## 3. Logging Functions

All 4 SDKs expose 4 level-specific functions with the same signature pattern:

| SDK | Signature | Details Type |
|-----|-----------|-------------|
| Node | `info(msg, details?)` / `warn(msg, details?)` / `error(msg, details?)` / `debug(msg, details?)` | `Record<string, string>` |
| Rust | `info!(msg)` / `info!(msg, details)` (4 macros) + `manual_log(level, msg, details)` | `&[(&str, &str)]` |
| Go | `Info(msg, details...)` / `Warn(msg, details...)` / `Error(msg, details...)` / `Debug(msg, details...)` | `...map[string]string` (0 or 1) |
| Python | `info(message, details=None)` / `warn(message, details=None)` / `error(message, details=None)` / `debug(message, details=None)` | `Optional[Dict[str, Any]]` |

All are **fail-open**: if `init()` was not called, events are silently dropped. Never throw.

---

## 4. Redaction

### Default rules

Every attribute key is checked case-insensitively against these patterns:

| Pattern | Mode | Effect |
|---------|------|--------|
| `password` | Full | `[REDACTED]` |
| `token` | Full | `[REDACTED]` |
| `secret` | Full | `[REDACTED]` |
| `email` | Partial | `us***om` (preserves first 2 + last 2 chars) |

### Modes

| Mode | Behavior |
|------|----------|
| `Full` | Replaces value with `[REDACTED]` |
| `Partial` | Shows first 2 + last 2 characters; `[***]` if length <= 4 |
| `Hash` | Replaces with a hash fingerprint |

### Custom keys

Node and Python accept custom redaction keys via an internal argument to `redactAttributes()`, but this is **not wired** into the public log functions — custom keys cannot currently be configured by end users. Rust and Go redaction is hardcoded.

---

## 5. Auto-Capture

### Node

On `init()`, automatically captures:

1. **Uncaught exceptions** (`process.on('uncaughtException')`) → level `"fatal"`, includes stack trace, type, message.
2. **Unhandled rejections** (`process.on('unhandledRejection')`) → level `"error"`, includes stack trace.
3. **HTTP requests** — monkey-patches `http.Server.prototype.emit` / `https.Server.prototype.emit`. Captures method, URL, status code, latency, headers (redacted). Optionally captures body if `captureBodies: true`. Framework-agnostic (Express, Fastify, raw http all covered).
4. **Console output** — patches `console.error` and (depending on `captureConsoleLevel`) `console.warn`. Tagged with `logger_name: "console"`.
5. **Framework detection** — reads `package.json` dependencies to detect Next.js, Express, Fastify, NestJS, Koa. Writes `greplog.config.json`.

### Rust

On `init()`, automatically captures:

1. **Panics** — `std::panic::set_hook` installed. Captures panic message, file/line, stack trace as `level: "error"`, `exception_type: "panic"`.
2. **tracing events** — registers a `GreplogLayer` on `tracing_subscriber::registry()`. Any `tracing::info!()` / `tracing::error!()` etc. are forwarded to Greplog.
3. **tracing spans** — `on_new_span` / `on_close` produce `Span` protobufs (name, start/end times). This means `tower_http::TraceLayer` spans in axum are auto-captured.
4. Does **not** write `greplog.config.json`.

### Go

On `init()`, automatically captures:

1. **Framework detection** — reads `go.mod` to detect Gin, Echo, Fiber. Writes `greplog.config.json`.
2. **Service name** — extracted from `go.mod` module path.
3. Does **not** auto-capture panics or goroutine crashes (Go has no hook equivalent). Use `greplog.Go(fn)` to safely spawn goroutines with panic capture.

### Python

On `init()`, automatically captures:

1. **Main thread uncaught exceptions** — wraps `sys.excepthook`. Level `"fatal"`, includes stack trace, exception type.
2. **Thread uncaught exceptions** — wraps `threading.excepthook`.
3. **stdlib logging** — registers a `GreplogHandler` on the root logger at `capture_log_level` threshold (default `WARNING`). Maps: CRITICAL→fatal, ERROR→error, WARNING→warn, INFO→info, DEBUG→debug. Includes logger name, file path, line number.
4. **HTTP requests** — if `app=` is passed to `init()`, wraps with WSGI or ASGI middleware.
5. **Framework detection** — scans installed packages for FastAPI, Flask, Django. Writes `greplog.config.json`.

---

## 6. HTTP Middleware

### Node — Automatic (no middleware needed)

HTTP capture is handled by monkey-patching `http.Server.prototype.emit` on `init()`. Every framework built on Node's http module is covered automatically. No extra import or `.use()` call needed.

### Rust — Axum layer (explicit)

```rust
use tower::ServiceBuilder;
use greplog::axum_layer;

Router::new()
    .route("/", get(handler))
    .layer(ServiceBuilder::new().layer(axum_layer()));
```

Produces `Span` protobufs (not log events) with method, URI path, redacted headers, status code, start/end times.

Additionally, if your app uses `tower_http::TraceLayer`, its spans are auto-captured by the tracing `GreplogLayer`.

### Go — Gin middleware + net/http wrapper

```go
// Gin
r := gin.New()
r.Use(greplog.Middleware())

// net/http
handler := greplog.WrapHandler(myHandler)
```

Both produce `Span` protobufs. Supports body capture via `Options.CaptureBodies`.

No middleware for Echo or Fiber (frames are detected but no middleware shipped).

### Python — App wrapping

Pass the app to `init()`:

```python
from fastapi import FastAPI
from greplog import init

app = FastAPI()
init(app=app)
```

Or wrap manually via `wrap_app()`:

```python
from greplog.middleware import wrap_app

app = wrap_app(app)
```

Two middleware classes handle WSGI (Flask, Django) and ASGI (FastAPI, Starlette) respectively. Auto-detected based on the app interface. Produces log events with `logger_name: "greplog.http"`.

---

## 7. Transport

### Protocol

All 4 SDKs send **length-prefixed protobuf frames** over TCP or Unix sockets:
- 4-byte little-endian length header
- Protobuf-encoded `IngestBatch` message body

### Connection strategy

1. Try Unix socket at configured `socketPath` (default `.greplog/greplog.sock`).
2. On failure, fall back to TCP at `tcpHost:tcpPort` (default `127.0.0.1:4318`).
3. On Windows, skip UDS and go straight to TCP.

### Queue and batching

| SDK | Max queue | Batch size | Flush trigger |
|-----|-----------|------------|---------------|
| Node | 1,000 events | 100 | Every `pushEvent()` |
| Rust | Unlimited (Vec) | 1 | Every 5s background tick |
| Go | 1,000 frames | 100 | Every `ReconnectDelay` tick + each `logEvent()` |
| Python | 1,000 events | 100 | Every `pushEvent()` |

### Reconnect

All 4 SDKs retry connection every 5 seconds on failure (Go is configurable via `ReconnectDelay`). Socket timeout is 5 seconds everywhere.

---

## 8. Additional Utilities

| Feature | Node | Rust | Go | Python |
|---------|------|------|----|--------|
| Safe goroutine spawner | — | — | `Go(fn func())` | — |
| Test utilities | `resetState()` | `test_reset()` | `ResetTestState()`, `StartTestServer()` | `reset_*_flags()`, `MockAgent` |
| ULID generation | `generateULID()` | — | `generateULID()` | `generate_ulid()` |
| Redaction utilities (exported) | `redactAttributes()`, `redactHeaders()`, `RedactionMode` | — (crate-internal) | — (package-private) | `redact_attributes()`, `redact_headers()`, `RedactionMode` |
| Framework detection | `detectFramework()`, `writeConfig()` | — | — (internal) | `detect_framework()`, `write_config()` |
| Encode/decode protobuf (exported) | `encodeLogEvent()`, `encodeIngestBatch()`, `decodeIngestResponse()` | — (greplog_core re-exported) | — | — |
| Axum tower Layer | — | `GreplogAxumLayer`, `GreplogLayer` | — | — |
| HTTP middleware | — (automatic) | Axum only | Gin + net/http | Flask, FastAPI, Starlette |
| Config file written | `greplog.config.json` | ❌ | `greplog.config.json` | `greplog.config.json` |

---

## Quickstart per SDK

### Node — 2-line setup

```ts
import { greplog } from 'greplog';
greplog.init({ service: 'my-app' });
// HTTP, console, and crash capture are automatic.
// Custom logs:
greplog.info('server started', { port: '3000' });
```

### Rust — 2-line setup

```rust
greplog::init();
greplog::warn!("disk space low");
greplog::info!("server started");
// Axum middleware:
// .layer(ServiceBuilder::new().layer(greplog::axum_layer()))
```

### Go — 2-line setup

```go
import greplog "github.com/greplog/greplog-go/src"

greplog.Init()
greplog.Info("server started")

// Gin middleware:
// r.Use(greplog.Middleware())
```

### Python — 2-line setup

```python
import greplog
greplog.init(service="my-app")
greplog.info("server started")
```