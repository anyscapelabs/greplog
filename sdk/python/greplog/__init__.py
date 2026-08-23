"""Greplog Python SDK: buffered, fire-and-forget logging to a Greplog server."""

import atexit
import json
import logging
import os
import threading
import time
import urllib.error
import urllib.request
from typing import Any, Optional

logger = logging.getLogger("greplog")

LEVELS = ("DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL")


class GreplogClient:
    """Queues records in memory and flushes them to `POST /api/log`.

    Mirrors the Node SDK: batch-size and interval triggers, oldest-drop past
    the queue cap, one retry for 429/5xx/network failures (the batch was
    never durably accepted), and never raises into the caller's code path.
    """

    def __init__(
        self,
        service: str,
        env: str = "development",
        endpoint: str = "http://127.0.0.1:5050",
        batch_size: int = 100,
        flush_interval: float = 0.5,
        max_queue_size: int = 10_000,
    ) -> None:
        self.service = service
        self.env = env
        self._url = f"{endpoint.rstrip('/')}/api/log"
        self._batch_size = max(1, batch_size)
        self._flush_interval = max(0.05, flush_interval)
        self._max_queue_size = max(1, max_queue_size)
        self._queue: list[dict[str, Any]] = []
        self._lock = threading.Lock()
        self._dropped = 0
        self._closing = threading.Event()
        self._wake = threading.Event()
        self._thread = threading.Thread(target=self._run, name="greplog-flush", daemon=True)
        self._thread.start()
        atexit.register(self.shutdown)

    @property
    def dropped_count(self) -> int:
        return self._dropped

    def track(self, level: str, message: Any, meta: Optional[dict[str, Any]] = None) -> None:
        record = {
            "timestamp_us": time.time_ns() // 1_000,
            "trace_id": meta.get("trace_id") if isinstance(meta, dict) else None,
            "level": level,
            "service": self.service,
            "message": message if isinstance(message, str) else _stringify(message),
            "raw_body": _stringify(_payload_without_trace(meta)),
        }
        with self._lock:
            self._queue.append(record)
            if len(self._queue) > self._max_queue_size:
                self._queue.pop(0)
                self._dropped += 1
            should_flush = len(self._queue) >= self._batch_size
        if should_flush:
            self.flush()

    def flush(self) -> None:
        with self._lock:
            batch, self._queue = self._queue, []
        if batch:
            self._send(batch)

    def shutdown(self, timeout: float = 5.0) -> None:
        if self._closing.is_set():
            return
        self._closing.set()
        self._wake.set()
        self._thread.join(timeout=timeout)
        self.flush()

    def _run(self) -> None:
        while not self._closing.wait(self._flush_interval):
            self.flush()

    def _send(self, batch: list[dict[str, Any]]) -> None:
        for attempt in (1, 2):
            try:
                request = urllib.request.Request(
                    self._url,
                    data=json.dumps(batch).encode("utf-8"),
                    headers={"Content-Type": "application/json"},
                    method="POST",
                )
                with urllib.request.urlopen(request, timeout=5) as response:
                    if response.status < 400:
                        return
                    retryable = response.status == 429 or response.status >= 500
            except (urllib.error.URLError, OSError):
                retryable = True
            if not retryable or attempt == 2:
                break
        logger.warning("greplog dropped %d records after a failed flush", len(batch))


def _stringify(value: Any) -> Optional[str]:
    if value is None:
        return None
    if isinstance(value, str):
        return value
    try:
        return json.dumps(value, default=str)
    except (TypeError, ValueError):
        return str(value)


def _payload_without_trace(meta: Optional[dict[str, Any]]) -> Optional[dict[str, Any]]:
    if not isinstance(meta, dict):
        return None
    return {key: value for key, value in meta.items() if key != "trace_id"} or None


_client: Optional[GreplogClient] = None
_init_lock = threading.Lock()


def init(
    service: Optional[str] = None,
    env: Optional[str] = None,
    endpoint: Optional[str] = None,
    batch_size: int = 100,
    flush_interval: float = 0.5,
    max_queue_size: int = 10_000,
) -> GreplogClient:
    """Initializes the global client; arguments win over `GREPLOG_*` env vars."""
    global _client
    with _init_lock:
        if _client is not None:
            return _client
        resolved_service = service or os.environ.get("GREPLOG_SERVICE_NAME") or "python-app"
        _client = GreplogClient(
            service=resolved_service,
            env=env or os.environ.get("GREPLOG_ENV") or "development",
            endpoint=endpoint or os.environ.get("GREPLOG_URL") or "http://127.0.0.1:5050",
            batch_size=batch_size,
            flush_interval=flush_interval,
            max_queue_size=max_queue_size,
        )
        return _client


def _client_or_raise() -> GreplogClient:
    if _client is None:
        raise RuntimeError("greplog.init() must be called before logging")
    return _client


def debug(message: Any, meta: Optional[dict[str, Any]] = None) -> None:
    _client_or_raise().track("DEBUG", message, meta)


def info(message: Any, meta: Optional[dict[str, Any]] = None) -> None:
    _client_or_raise().track("INFO", message, meta)


def warning(message: Any, meta: Optional[dict[str, Any]] = None) -> None:
    _client_or_raise().track("WARN", message, meta)


def error(message: Any, meta: Optional[dict[str, Any]] = None) -> None:
    _client_or_raise().track("ERROR", message, meta)


def critical(message: Any, meta: Optional[dict[str, Any]] = None) -> None:
    _client_or_raise().track("CRITICAL", message, meta)


def flush() -> None:
    _client_or_raise().flush()


def shutdown(timeout: float = 5.0) -> None:
    if _client is not None:
        _client.shutdown(timeout)
