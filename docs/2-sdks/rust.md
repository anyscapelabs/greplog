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

Available levels: `trace!`, `debug!`, `info!`, `warn!`, `error!`, `fatal!`.

## Batching

The SDK uses an MPSC channel to coalesce log records and flush them to `POST /api/log` in batches. Buffered records are flushed:

- when the batch reaches ~1000 records,
- on an interval timer,
- and on a graceful drop of the guard returned by `init`.

## Graceful shutdown

```rust
let guard = greplog::init("auth-service", "production");
// ...
drop(guard); // Flushes any remaining buffered logs
```