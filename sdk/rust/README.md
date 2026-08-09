# greplog-rust SDK

The reference Rust SDK (`cargo add greplog`). Batches logs client-side and POSTs
them to the Greplog ingest endpoint.

```rust
greplog::init("auth-service", "production"); // or greplog::init_from_env()
greplog::info!("User authenticated", user_id = 42);
greplog::error!("Token generation failed", reason = "Key expired");
```

> **Status:** planned. See [`docs/2-sdks/rust.md`](../../docs/2-sdks/rust.md).