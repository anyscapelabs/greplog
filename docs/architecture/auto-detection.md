# Framework Auto-Detection & Service Identity

`greplog init` (or the first `greplog.init()` call in-process) runs a detection pass:

## Service Identity

The SDK attempts to auto-detect its `service_name` (e.g., reading the `name` field from `package.json`, or module name from `go.mod`). This allows the dashboard to distinguish frontend logs from backend logs. A user can manually override this: `greplog.init({ service: "api-backend" })`.

## Per-Language Detection

- **Node:** Reads `package.json` dependencies → matches known framework signatures (`express`, `fastify`, `@nestjs/core`, `next`) → attaches the right middleware/hook automatically via monkey-patching.
- **Python:** Inspects installed packages for `flask`, `django`, `fastapi` → hooks `sys.excepthook`, adds a `logging.Handler`, and wraps WSGI/ASGI middleware.
- **Go:** Scans `go.mod` for known router deps (`gin`, `echo`, `fiber`) at init time.
- **Rust:** Hooks into the `tracing` crate ecosystem (`tracing_subscriber::Layer`) and registers a panic hook. Apps not using `tracing` can add `.layer(greplog::axum_layer())`.

Writes a minimal `greplog.config.json` at the project root documenting what it found so re-runs are instant.
