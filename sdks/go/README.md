# Greplog Go SDK

Go SDK for Greplog — `greplog.Init()` with framework auto-detection and fail-open guarantees.

## Usage

```go
import greplog "github.com/greplog/greplog-go/src"

greplog.Init()
greplog.Info("server started", greplog.Details{"port": "4000"})
```

See [examples/basic.go](examples/basic.go) for a full working example showing configuration and manual logging.

## What this is

Provides `greplog.Init()` for automatic log/error capture, middleware for Gin/Echo/Fiber HTTP capture, and `greplog.Go()` for goroutine panic safety. Uses Protobuf over UDS/TCP to the local agent. Since Go lacks runtime monkey-patching, it requires one extra line for HTTP middleware and goroutine wrapping.

## Building

```sh
cd sdks/go && go build ./...
```

## Testing

```sh
cd sdks/go && go test ./tests/...
```

Tests are in `tests/` as a separate Go package (requiring `src/testexport.go` for symbol export).

## Structure

```
src/      — greplog.go, middleware.go, transport.go, redact.go, detect.go, serialize.go,
            gowrapper.go, ulid.go, types.go, testexport.go (test helpers exported for tests/)
examples/ — basic.go runnable usage example
tests/    — greplog_test.go
core/     — shared protobuf types
proto/    — events.proto (copy)
```

## Relationship to the rest of greplog

One of four SDKs that connect to the greplog agent. See [docs/sdk/design.md](/docs/sdk/design.md) for the shared contract and per-language differences.

