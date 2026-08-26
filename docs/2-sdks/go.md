# Go SDK

The `greplog` Go module (source: [`sdk/go`](../../sdk/go)) integrates with the
standard library `log/slog` package. After a one-line call, every ordinary
`slog.Info` / `slog.Error` (and `slog.Warn`, `slog.Debug`) in your process is
asynchronously streamed to the Greplog ingest agent — no transport code in
your application.

## Add the dependency

The module is published as `greplog`; from the repo it can be used via a
`replace` directive:

```go
require greplog v0.0.0
replace greplog => /path/to/greplog/sdk/go
```

## Initialize

```go
package main

import (
	"log/slog"

	greplog "greplog/greplog"
)

func main() {
	cleanup := greplog.MustInit(greplog.Config{
		Service: "payment-gateway",
		Env:     "development",
		// Endpoint defaults to http://127.0.0.1:5050/api/log
	})
	defer cleanup() // flushes pending logs before exit

	slog.Info("Server running", slog.String("port", "4001"))
}
```

`Init` builds a `log/slog.Handler` and calls `slog.SetDefault`, so all standard
`log/slog` calls route to Greplog automatically. It returns a cleanup function
and an error; call the cleanup (typically via `defer`) to stop the batching
worker and flush any pending logs before the process exits. The error reports
configuration problems before any log is sent — above all an invalid service
name: valid names are 1–64 characters of `a-z A-Z 0-9 _ . -` (the name becomes
a storage directory, so `/` and `..` are not allowed).

For the common case where invalid configuration should simply stop startup,
use `MustInit`, which panics on that error instead:

```go
cleanup := greplog.MustInit(greplog.Config{Service: "payment-gateway"})
defer cleanup()
```

Missing configuration values fall back to the environment:

- `GREPLOG_SERVICE_NAME` — default `"go-app"`
- `GREPLOG_ENV` — default `"development"`
- `GREPLOG_URL` — default `http://127.0.0.1:5050/api/log`

## Log

Because the handler is global, you use `log/slog` directly:

```go
slog.Info("payment processed",
	slog.Int("amount", req.Amount),
	slog.String("user_id", req.UserID),
)
```

All `slog` levels are forwarded (`DEBUG`, `INFO`, `WARN`, `ERROR`). Structured
attributes and `slog.With(...)` / `slog.Group(...)` context are serialized into
the record's `raw_body` JSON payload.

## Batching

Records are **not** sent one-by-one. Logs are written to a buffered channel
(non-blocking — when the channel is full the entry is dropped rather than
stalling the host) and a background worker streams them to `POST /api/log` in
batches:

- Flush when a batch reaches `BatchSize` records (default 100).
- Flush on a `FlushInterval` timer if it never fills (default `500ms`).
- Non-blocking send: a full channel drops the newest entry instead of blocking.

## Graceful shutdown

```go
cleanup := greplog.MustInit(greplog.Config{Service: "payment-gateway"})
defer cleanup()
```

`cleanup()` closes the channel and flushes every remaining buffered batch
before returning. It is idempotent, so it is safe in signal handlers.

## Configuration reference

```go
cleanup := greplog.MustInit(greplog.Config{
	Service:         "payment-gateway",
	Env:             "production",
	Endpoint:        "http://127.0.0.1:5050/api/log",
	BatchSize:       500,
	FlushInterval:   time.Second,
	ChannelCapacity: 50000,
})
defer cleanup()
```

For a full runnable reference, see the
[Go example app](../../example/go-example).