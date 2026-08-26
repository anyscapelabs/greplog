# greplog-sdk (Python)

Buffered, fire-and-forget logging to a Greplog ingest server. Stdlib only.

```bash
pip install greplog-sdk
```

## Quick start

```python
import greplog

greplog.init("payment-service", "production")  # or greplog.init() to read env vars

greplog.info("Processing order", {"order_id": 9876})
greplog.error("Database connection lost", {"retry_count": 3, "trace_id": "job_42"})
```

Records are queued in memory and flushed to `POST /api/log` when the batch
fills (100 records) or every 500 ms, whichever comes first. `shutdown()` —
also registered via `atexit` — flushes the remainder before process exit.

## Configuration

Arguments to `init()` win over environment variables:

| argument | env var | default |
|---|---|---|
| `service` | `GREPLOG_SERVICE_NAME` | `python-app` |
| `env` | `GREPLOG_ENV` | `development` |
| `endpoint` | `GREPLOG_URL` | `http://127.0.0.1:5050` |

Tuning: `batch_size`, `flush_interval` (seconds), `max_queue_size`
(oldest records are dropped past the cap; see `client.dropped_count`).

## Guarantees

- The SDK never raises into your application; failed flushes are logged via
  the `logging` module and retried once for `429`/`5xx`/network errors.
- Levels map straight onto the engine's severity set:
  `debug`, `info`, `warning`, `error`, `critical`.

## Development

```bash
python -m venv .venv && . .venv/bin/activate
pip install -e '.[dev]'
pytest
```
