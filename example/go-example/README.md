# Greplog Go Example

A minimal Go HTTP API demonstrating seamless integration of the
[Greplog Go SDK](../../sdk/go) with the standard library `log/slog`
structured logging package.

After `greplog.Init(...)`, ordinary `slog.Info` / `slog.Error` calls are
automatically streamed to the Greplog ingest backend — no manual transport
code required.

## What this example shows

- **One-call initialization** — `greplog.Init` registers a global `slog`
  handler and reports invalid configuration (like a bad service name) before
  any log is sent.
- **Zero magic** — the route handlers use only the native `log/slog` API;
  Greplog captures every structured attribute (`slog.String`, `slog.Int`, …).
- **Graceful shutdown** — `defer cleanup()` drains the asynchronous channel
  so pending logs are flushed before the process exits.

## Running the example

### 1. Start the Greplog engine

In a separate terminal, from the repository root:

```bash
cargo run -p greplog-cli dev
```

This launches the Rust ingest agent (port `5050`) and the query/dashboard
server (port `3000`).

### 2. Run the Go server

```bash
cd example/go-example
go run .
```

You should see the startup logs routed through Greplog:

```
Server running on port 4001 port=4001
Greplog auto-instrumentation active.
```

### 3. Send a test request

```bash
curl -X POST http://localhost:4001/api/charge \
  -H "Content-Type: application/json" \
  -d '{"amount": 15000, "user_id": "usr_9921"}'
```

The structured request/response logs appear instantly in the Greplog
dashboard (`http://127.0.0.1:3000`).

## Project layout

```
example/go-example/
├── go.mod   # module greplog-go-example (uses a replace directive to the local SDK)
├── main.go  # one-line Init + standard slog usage + /api/charge handler
└── README.md
```

## Notes

The `go.mod` `replace` directive points at the local SDK so you can edit
`../../sdk/go` and test changes immediately:

```go
require github.com/anyscapelabs/greplog/sdk/go v0.0.0
replace github.com/anyscapelabs/greplog/sdk/go => ../../sdk/go
```

The example uses a local `replace` for development; in your own app you can
simply run `go get github.com/anyscapelabs/greplog/sdk/go@latest`. The SDK
package lives at `github.com/anyscapelabs/greplog/sdk/go/greplog`, so it is
imported as:

```go
import greplog "github.com/anyscapelabs/greplog/sdk/go/greplog"
```
