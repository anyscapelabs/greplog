# greplog-core

Shared data model, Protobuf wire format, and redaction utilities.

## What this is

Defines the Protobuf schema (`proto/greplog/v1/events.proto`) compiled via `prost-build` into native Rust structs, plus the redaction module (`src/redact.rs`) and well-known schema constants (`src/schema.rs`). Has zero runtime dependencies beyond Prost. Used by every other crate in the workspace but never runs standalone.

## Building

```sh
cargo build -p greplog-core
```

## Testing

```sh
cargo test -p greplog-core
```

Tests cover schema edge cases and redaction strategies.

## Structure

```
src/     — redact.rs, schema.rs, proto module (generated)
tests/   — arrow_schema.rs, redact.rs
```

## Relationship to the rest of greplog

`greplog-core` is a dependency of `greplog-agent` and `greplog-cli`. All SDKs share the same `events.proto` schema independently. See [docs/architecture/overview.md](/docs/architecture/overview.md).
