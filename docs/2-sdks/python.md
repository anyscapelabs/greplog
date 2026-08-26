# Python SDK

The `greplog-sdk` pip package (import name stays `greplog`) matches the Node.js SDK's zero-friction initialization.

## Install

```bash
pip install greplog-sdk
```

## Initialize

```python
import greplog

greplog.init("payment-service", "production")  # Or greplog.init() to read from os.environ
```

`init(service_name, env)` accepts values explicitly, or reads `GREPLOG_SERVICE_NAME`, `GREPLOG_ENV`, and `GREPLOG_URL` from the environment when called with no arguments.

The service name must be 1-64 characters of `a-z A-Z 0-9 _ . -` (it becomes a
storage directory on the server, so `/` and `..` are not allowed). An invalid
name fails at init with a clear error instead of every record being rejected
by the server later.

## Log

```python
greplog.info("Processing order", {"order_id": 9876})
greplog.error("Database connection lost", {"retry_count": 3})
```

Available functions: `debug`, `info`, `warning`, `error`, `critical`.

## Batching

Records are buffered and flushed to `POST /api/log` when the batch fills (100 records), every 500 ms, or at interpreter exit (`atexit`). Past a 10,000-record cap the oldest are dropped (`client.dropped_count()`); a failed flush retries once for `429`/`5xx`/network errors.

Tune via `greplog.init(..., batch_size=100, flush_interval=0.5, max_queue_size=10_000)`.

## Graceful shutdown

```python
greplog.flush()
```

Call before your process exits (or use `atexit`) to flush remaining buffered logs.