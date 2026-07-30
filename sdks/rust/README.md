# greplog — Rust SDK

Rust SDK for Greplog — `greplog::init()` for automatic tracing, panic, and HTTP capture.

## What this is

Registers a `tracing_subscriber::Layer` for span/event capture and a `std::panic::set_hook` for panic capture. Provides `greplog::axum_layer()` for automatic per-request latency/status capture in Axum apps. Uses the shared `events.proto` schema over UDS/TCP to the local agent. Fail-open guaranteed.

## Building

```sh
cargo build -p greplog
```

## Testing

```sh
cargo test -p greplog
```

Tests cover basic init, panic capture, tracing integration, redaction, idempotency, and Axum integration.

## Structure

```
src/     — lib.rs, transport.rs, redact.rs, ulid.rs, tracing_layer.rs, axum_layer.rs
tests/   — integration tests (one file per concern)
```

## Relationship to the rest of greplog

One of four SDKs that connect to the greplog agent. Shares the same Protobuf wire format and fail-open contract. See [docs/sdk/design.md](/docs/sdk/design.md).
