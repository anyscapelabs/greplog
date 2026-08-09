# Python SDK

The `greplog` pip package matches the Node.js SDK's zero-friction initialization.

## Install

```bash
pip install greplog
```

## Initialize

```python
import greplog

greplog.init("payment-service", "production")  # Or greplog.init() to read from os.environ
```

`init(service_name, env)` accepts values explicitly, or reads `GREPLOG_SERVICE_NAME`, `GREPLOG_ENV`, and `GREPLOG_URL` from the environment when called with no arguments.

## Log

```python
greplog.info("Processing order", {"order_id": 9876})
greplog.error("Database connection lost", {"retry_count": 3})
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`, `fatal`.

## Batching

Records are buffered and flushed to `POST /api/log` in batches (on ~1000 records, an interval flush, or at interpreter exit) so the SDK stays fast and quiet on the network.

## Graceful shutdown

```python
greplog.flush()
```

Call before your process exits (or use `atexit`) to flush remaining buffered logs.