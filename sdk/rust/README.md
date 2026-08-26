# greplog-sdk (Rust)

Buffered, fire-and-forget logging to a Greplog ingest server.

```toml
[dependencies]
greplog-sdk = "0.1"
```

## Quick start

```rust
use greplog::{init, info};

init("auth-service", "production"); // or init_from_env()

info!("User authenticated", user_id = 42);
error!("Token generation failed", reason = "Key expired");
```

Records queue in memory and flush to `POST /api/log` when the batch fills
(100 records) or every 500 ms. A failed flush retries once for `429`/`5xx`/
network errors; the SDK never panics into your code path.

## Levels

`debug!`, `info!`, `warn!`, `error!`, `critical!` — all take a message plus
optional `key = value` metadata that lands in the record's `raw_body`
(`trace_id = "…"` is lifted into its own field instead).

## Configuration

```rust
use greplog::Config;

greplog::init_with("auth-service", "production", Config {
    endpoint: "http://127.0.0.1:5050".into(),
    batch_size: 200,
    ..Default::default()
});
```

`init_from_env()` reads `GREPLOG_URL`, `GREPLOG_SERVICE_NAME`, and
`GREPLOG_ENV`. `Client::dropped_count()` reports records shed past the
queue cap; `greplog::flush()` sends whatever is pending.

## Development

```bash
cargo test
```
