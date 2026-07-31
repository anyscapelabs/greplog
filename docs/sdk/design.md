# Greplog SDK Design — Shared Contract & Per-Language Strategy

> **Status:** ✅ Shipped (v0.1)

Goal: `import { greplog } from 'greplog'; greplog.init();` (or the equivalent
two lines in each language) captures logs, errors, uncaught
exceptions/panics, and HTTP request/response metadata automatically, with
zero further config. A manual override (`greplog.error(msg, details)` or
equivalent) is always available for anything a dev wants to log
explicitly. This doc defines what's identical across all four SDKs and
what's genuinely different per language, so implementation rounds don't
silently diverge on the shared contract.

---

## 1. What is identical across every SDK — do not vary this per language

### 1.1 Transport
- Every SDK connects to the local agent over the same channel already
  defined in the agent architecture: Unix domain socket (`.greplog/greplog.sock`)
  on macOS/Linux, named pipe on Windows, with TCP (`127.0.0.1:4318`) as
  the documented fallback for containerized apps that can't reach the
  host's UDS.
- Wire format: length-delimited `IngestBatch` protobuf frames, matching
  the schema already defined in `greplog-core`. No SDK invents its own
  wire format or a JSON-over-HTTP alternative — one shared contract,
  four implementations of the same client.
- **Fail-open, always.** If the agent isn't running, the SDK must no-op
  silently (or buffer briefly and drop) — never throw, never block, never
  add latency to the host app. This is non-negotiable across all four
  languages; test it explicitly in every SDK's test suite (kill the
  agent, confirm the host app runs identically with and without it).

### 1.2 The manual override API — same shape, idiomatic per language
Every SDK exposes the same four log levels plus structured
error-with-context, callable regardless of whether `init()` was called:
- `greplog.error(message, details?)`
- `greplog.warn(message, details?)`
- `greplog.info(message, details?)`
- `greplog.debug(message, details?)`
`details` is a free-form structured object/dict/map — becomes the
`attributes` field on the wire, same as auto-captured events. This must
work identically whether or not the automatic capture below is active;
manual calls are never suppressed or altered by `init()`.

### 1.3 Redaction — applied identically to auto-captured and manual data
- SDK-side redaction (matching the agent's existing `redact.rs` modes:
  Full/Partial/Hash) runs on **every** outgoing event, auto-captured or
  manual, before it leaves the process. Default redacted keys:
  `password`, `token`, `secret` (Full), `email` (Partial) — same defaults
  as the agent, so there's no SDK where sensitive data leaks because the
  SDK-side default differs from the agent-side one.
- This is a second layer, not a replacement for the agent's own
  redaction — defense in depth, not either/or.

### 1.4 Body capture is opt-in, not default-on
- `http.request.body` / `http.response.body` are captured **only** if the
  dev explicitly opts in (e.g. `greplog.init({ captureBodies: true })` or
  equivalent). Default is method/route/status/latency/headers only — no
  request or response payload content, since that's where passwords,
  tokens, and PII actually live. This applies identically across all
  four SDKs; no language gets a different default here.

### 1.5 Noise control for auto-captured logging
- Auto-patching the host language's existing logging output (Node's
  `console`, Python's `logging` root handler) defaults to **warn/error
  level only**, not every debug/info line. A dev's existing `console.log`
  spam shouldn't flood the dashboard by default. This can be raised via
  config (`captureConsoleLevel: 'debug'` or equivalent) but starts
  conservative.

### 1.6 Event ID
- Every SDK assigns an `event_id` (ULID recommended — sortable,
  timestamp-embeddable, more ergonomic than raw UUID) once, at the point
  an event is created, and sends it on the wire. This is what lets the
  agent use it directly instead of falling back to content-hashing
  (per the agent's existing event_id-first/content-hash-fallback dedup
  logic) — every SDK populating this correctly is what makes that fast
  path actually engage in practice, not just in theory.

---

## 2. Per-language auto-instrumentation strategy

### 2.1 Node — closest to fully automatic
- **Uncaught exceptions / unhandled rejections:**
  `process.on('uncaughtException', ...)`, `process.on('unhandledRejection', ...)`.
- **HTTP:** patch at the `node:http`/`node:https` core module level
  (`Server.prototype.emit` intercepting the `'request'` event), not
  per-framework middleware. Since Express/Fastify/Koa/Next.js's custom
  server all sit on Node's core HTTP server, one patch point captures
  every framework's requests automatically (method, path, status,
  latency) with no middleware required. Framework detection is an
  enhancement on top (cleaner route patterns like `/users/:id` instead
  of raw URL), not a requirement for basic capture.
- **Existing console output:** monkey-patch `console.error`/`console.warn`
  (not `console.log` by default, per §1.5) to forward to the agent
  alongside whatever the dev already does with them.

### 2.2 Python — true one-liner via a different mechanism
- **Uncaught exceptions:** `sys.excepthook` for the main thread, **and**
  `threading.excepthook` (3.8+) — the second one is easy to miss and
  without it, exceptions in any spawned thread vanish silently instead of
  being captured.
- **The highest-leverage hook:** attach a `logging.Handler` to the
  **root logger**. Most of the Python ecosystem, including what
  third-party libraries and most web frameworks log, flows through
  stdlib `logging` eventually — this one hook captures a large amount
  "for free," including from dependencies the dev doesn't control.
- **HTTP:** WSGI/ASGI middleware, auto-injected by wrapping the detected
  framework's app callable at `init()` time where the app object is
  reachable (Flask/FastAPI's app is normally accessible this way).

### 2.3 Rust — one honest, documented exception
- `greplog::init()` registers a `tracing_subscriber::Layer` (captures any
  span/event already flowing through the `tracing` crate — which most of
  the Rust web ecosystem, including `axum` apps using
  `tower_http::TraceLayer`, already emits) plus a `std::panic::set_hook`
  for panics. Both fully automatic with the one call.
- **The gap, stated plainly in docs, not hidden:** an app not already
  using `tracing` for HTTP logging needs one additional line —
  `.layer(greplog::axum_layer())` — for automatic per-request
  latency/status capture. This is consistent with how OpenTelemetry and
  every other Rust observability tool already works; it won't feel like
  a broken promise to a Rust developer, but it must be documented as the
  one language where "two lines" sometimes means three.

### 2.4 Go — the real, unfixable exception
- Go has no runtime monkey-patching and no way to intercept an arbitrary
  function call or recover a panic in a goroutine other than the one that
  panicked — this is a language constraint, not an implementation gap.
- **Goroutine panics:** provide `greplog.Go(func() { ... })` as a
  drop-in replacement for `go func() { ... }()`, wrapping the goroutine
  with automatic recover-and-log. One small syntax change per goroutine
  — the most automatic capture Go actually permits.
- **HTTP:** one line of middleware —
  `router.Use(greplog.Middleware())` for a detected framework
  (gin/echo/fiber), or wrapping the handler directly for raw `net/http`.
- Document this honestly as "two lines for panics and manual logs,
  three for HTTP or goroutine safety" — don't oversell Go to the same
  standard as Node/Python.

---

## 3. Framework auto-detection (enhancement layer, not required for basic capture)

At `init()` time, each SDK inspects the project for known framework
signatures (Node: `package.json` deps; Python: installed packages via
`importlib.metadata`; Go: `go.mod` deps; Rust: `Cargo.toml` deps) and
writes a `greplog.config.json` recording what was found. This upgrades
raw-URL capture into clean route-pattern capture (`/users/:id` instead of
`/users/482`) where the detected framework's routing info is reachable.
Detection failing or finding nothing must never prevent basic capture
(uncaught exceptions, manual API) from working — this is additive, not a
prerequisite.

---

## 4. Suggested build order

1. Node SDK — highest reach, closest to fully automatic, validates the
   shared transport/redaction/manual-API contract end-to-end first.
2. Python SDK — second-highest reach, validates the contract against a
   meaningfully different auto-capture mechanism (root logger vs. core
   module patching).
3. Rust SDK — smaller ecosystem but shares a language/runtime with the
   agent itself; can reuse `greplog-core`'s types directly rather than
   needing a fresh wire-format implementation.
4. Go SDK — build last; the one requiring the most honest documentation
   about what "automatic" actually means, best done once the pattern is
   well-established from the other three.
