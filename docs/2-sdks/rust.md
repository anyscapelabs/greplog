# Rust SDK

The `greplog` crate is the reference SDK — it uses the same batch-and-flush pipeline as the other SDKs.

## Add the dependency

```bash
cargo add greplog
```

## Initialize

```rust
use greplog;

fn main() {
    greplog::init("auth-service", "production"); // Or greplog::init_from_env()
}
```

`init(service_name, env)` accepts values explicitly, or `init_from_env()` reads `GREPLOG_SERVICE_NAME`, `GREPLOG_ENV`, and `GREPLOG_URL` from the environment.

## Log

```rust
greplog::info!("User authenticated", user_id = 42);
greplog::error!("Token generation failed", reason = "Key expired");
```

Available macros: `debug!`, `info!`, `warn!`, `error!`, `critical!`.

## Batching

Records queue in memory and flush to `POST /api/log`:

- when the batch fills (100 records by default, tune via [`Config`](https://docs.rs/greplog)),
- every 500 ms,
- or immediately from `greplog::flush()`.

Past a 10,000-record cap the oldest records are dropped; `Client::dropped_count()` reports how many.

A flush that fails with `429`/`5xx`/a network error retries once; other rejections drop the batch rather than resend bytes the server refused.

## Graceful shutdown

```rust
greplog::flush(); // send whatever is still buffered
```

`init*` returns an `Arc<Client>` handle; dropping the process without flushing loses at most one interval's worth of records.