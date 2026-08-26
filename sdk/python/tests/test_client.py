import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

import greplog


class FakeServer:
    """Captures POST bodies and answers with a configurable status."""

    def __init__(self, status=200):
        self.requests = []
        self.status = status
        self.lock = threading.Lock()
        outer = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(length)
                with outer.lock:
                    outer.requests.append(json.loads(body))
                self.send_response(outer.status)
                self.end_headers()

            def log_message(self, *args):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_port
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def url(self):
        return f"http://127.0.0.1:{self.port}"

    def stop(self):
        self.server.shutdown()


@pytest.fixture
def server():
    fake = FakeServer()
    yield fake
    fake.stop()


def make_client(server, **overrides):
    options = {
        "service": "test-svc",
        "endpoint": server.url(),
        "batch_size": 3,
        "flush_interval": 60,
    }
    options.update(overrides)
    return greplog.GreplogClient(**options)


def test_batch_flushes_at_batch_size(server):
    client = make_client(server)
    for index in range(3):
        client.track("INFO", f"event {index}", {"order_id": index})
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline and not server.requests:
        time.sleep(0.01)

    assert len(server.requests) == 1
    batch = server.requests[0]
    assert [row["message"] for row in batch] == ["event 0", "event 1", "event 2"]
    first = batch[0]
    assert first["level"] == "INFO"
    assert first["service"] == "test-svc"
    assert isinstance(first["timestamp_us"], int)
    assert json.loads(first["raw_body"]) == {"order_id": 0}
    client.shutdown()


def test_trace_id_travels_in_its_own_field(server):
    client = make_client(server, batch_size=1)
    client.track("ERROR", "boom", {"trace_id": "job_9"})
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline and not server.requests:
        time.sleep(0.01)

    row = server.requests[0][0]
    assert row["trace_id"] == "job_9"
    # trace_id lives in its own field; with nothing else in meta there is no body.
    assert row["raw_body"] is None
    client.shutdown()


def test_periodic_flush_below_batch_size(server):
    client = make_client(server, flush_interval=0.1)
    client.track("WARN", "lonely")
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline and not server.requests:
        time.sleep(0.01)

    assert server.requests and server.requests[0][0]["message"] == "lonely"
    client.shutdown()


def test_retryable_status_requeues_and_recovers(server):
    client = make_client(server, batch_size=1, flush_interval=60)
    server.status = 500
    client.track("ERROR", "first try")
    time.sleep(0.2)

    server.status = 200
    client.track("INFO", "second try")
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline and len(server.requests) < 2:
        time.sleep(0.01)

    messages = [batch[0]["message"] for batch in server.requests]
    assert messages.count("first try") >= 1
    assert "second try" in messages
    client.shutdown()


def test_queue_cap_drops_oldest_and_counts(server):
    client = make_client(server, max_queue_size=5, batch_size=100, flush_interval=60)
    for index in range(8):
        client.track("INFO", f"m{index}")

    assert client.dropped_count == 3
    client.flush()
    assert [row["message"] for row in server.requests[0]] == ["m3", "m4", "m5", "m6", "m7"]
    client.shutdown()


def test_shutdown_flushes_remainder(server):
    client = make_client(server, flush_interval=60)
    client.track("CRITICAL", "last words")
    client.shutdown()

    assert server.requests[0][0]["message"] == "last words"


def test_init_reads_env(monkeypatch, server):
    monkeypatch.setenv("GREPLOG_URL", server.url())
    monkeypatch.setenv("GREPLOG_SERVICE_NAME", "env-svc")
    monkeypatch.setenv("GREPLOG_ENV", "staging")

    greplog._client = None
    greplog.init(batch_size=1)
    greplog.info("via env")
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline and not server.requests:
        time.sleep(0.01)

    row = server.requests[0][0]
    assert row["service"] == "env-svc"
    greplog.shutdown()


def test_service_name_validation_matches_the_server():
    for name in ("auth-api", "payment_worker.2", "A-1_2.b", "x" * 64):
        assert greplog.is_valid_service_name(name)
    for name in ("", "../evil", "..\\windows", "a/b", "has space", "süß", "x" * 65):
        assert not greplog.is_valid_service_name(name)


def test_constructor_rejects_invalid_service_names_before_spawning_anything():
    # A rejected constructor must leave nothing behind: no flusher thread, no
    # atexit hook that would run against a half-built client.
    threads_before = threading.active_count()
    with pytest.raises(ValueError, match=r'invalid service name.*\.\./evil'):
        greplog.GreplogClient(service="../evil")
    assert threading.active_count() == threads_before


def test_init_raises_for_bad_env_config(monkeypatch):
    monkeypatch.setenv("GREPLOG_SERVICE_NAME", "../from-env")
    monkeypatch.setattr(greplog, "_client", None)
    with pytest.raises(ValueError, match="invalid service name"):
        greplog.init()
    assert greplog._client is None
