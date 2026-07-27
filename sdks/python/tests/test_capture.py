"""End-to-end capture tests against a mock agent."""

from __future__ import annotations

import logging
import sys
import threading
import time

import greplog
from conftest import wait_for_logs


def test_main_thread_uncaught_exception_capture(mock_agent, init_kwargs):
    greplog.init(**init_kwargs(service_name="uncaught-test"))

    try:
        raise RuntimeError("main thread uncaught boom")
    except RuntimeError:
        exc_type, exc_value, exc_tb = sys.exc_info()
        sys.excepthook(exc_type, exc_value, exc_tb)

    logs = wait_for_logs(mock_agent, lambda logs: any(log.message == "main thread uncaught boom" for log in logs))
    log = next(item for item in logs if item.message == "main thread uncaught boom")

    assert log.level == "fatal"
    assert log.exception_type == "RuntimeError"
    assert len(log.stack_trace) > 0
    assert log.event_id


def test_thread_uncaught_exception_capture_via_threading_excepthook(mock_agent, init_kwargs):
    """Test #3 — thread exceptions must be captured, not silently dropped."""
    greplog.init(**init_kwargs(service_name="thread-excepthook-test"))

    def worker() -> None:
        raise ValueError("thread excepthook boom")

    thread = threading.Thread(target=worker, name="greplog-thread-test")
    thread.start()
    thread.join(timeout=2)

    logs = wait_for_logs(
        mock_agent,
        lambda items: any(log.message == "thread excepthook boom" for log in items),
        timeout=5.0,
    )
    log = next(item for item in logs if item.message == "thread excepthook boom")

    assert log.level == "fatal"
    assert log.exception_type == "ValueError"
    assert log.logger_name == "threading.excepthook"
    assert len(log.stack_trace) > 0
    assert log.event_id


def test_root_logger_captures_third_party_warning(mock_agent, init_kwargs):
    greplog.init(**init_kwargs(service_name="logging-test"))

    logging.getLogger("some.library").warning("dependency warning from stdlib")

    logs = wait_for_logs(mock_agent, lambda items: any("dependency warning" in log.message for log in items))
    log = next(item for item in logs if "dependency warning" in item.message)

    assert log.level == "warn"
    assert log.logger_name == "some.library"


def test_log_level_respected_info_excluded_warning_included(mock_agent, init_kwargs):
    greplog.init(**init_kwargs(service_name="log-level-test"), capture_log_level="WARNING")

    logging.getLogger("some.library").info("info should be excluded")
    logging.getLogger("some.library").warning("warning should be included")

    time.sleep(0.5)
    logs = mock_agent.all_logs()
    messages = [log.message for log in logs]

    assert "info should be excluded" not in messages
    assert "warning should be included" in messages


def test_logging_handler_does_not_break_existing_host_logging(capsys, mock_agent, init_kwargs):
    root = logging.getLogger()
    stream = logging.StreamHandler()
    stream.setLevel(logging.INFO)
    root.addHandler(stream)
    root.setLevel(logging.INFO)

    try:
        greplog.init(**init_kwargs(service_name="host-logging-test"))

        logging.getLogger("host.app").warning("host-visible warning")

        captured = capsys.readouterr()
        assert "host-visible warning" in captured.err or "host-visible warning" in captured.out

        logs = wait_for_logs(mock_agent, lambda items: any("host-visible warning" in log.message for log in items))
        assert any("host-visible warning" in log.message for log in logs)
    finally:
        root.removeHandler(stream)


def test_http_capture_flask(mock_agent, init_kwargs):
    from flask import Flask

    flask_app = Flask(__name__)

    @flask_app.get("/flask-route")
    def flask_route():
        return "ok"

    greplog.init(app=flask_app, **init_kwargs(service_name="flask-http-test"))
    flask_app.test_client().get("/flask-route")

    logs = wait_for_logs(
        mock_agent,
        lambda items: any(log.attributes.get("http.route") == "/flask-route" for log in items),
    )
    flask_log = next(log for log in logs if log.attributes.get("http.route") == "/flask-route")

    assert "GET /flask-route" in flask_log.message
    assert flask_log.event_id


def test_http_capture_fastapi(mock_agent, init_kwargs):
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    fastapi_app = FastAPI()

    @fastapi_app.get("/fastapi-route")
    def fastapi_route():
        return {"ok": True}

    greplog.init(app=fastapi_app, **init_kwargs(service_name="fastapi-http-test"))
    TestClient(fastapi_app).get("/fastapi-route")

    logs = wait_for_logs(
        mock_agent,
        lambda items: any(log.attributes.get("http.route") == "/fastapi-route" for log in items),
    )
    fastapi_log = next(log for log in logs if log.attributes.get("http.route") == "/fastapi-route")

    assert "GET /fastapi-route" in fastapi_log.message
    assert fastapi_log.event_id


def test_body_capture_off_by_default(mock_agent, init_kwargs):
    from flask import Flask

    app = Flask(__name__)

    @app.post("/submit")
    def submit():
        return "ok", 200

    greplog.init(app=app, **init_kwargs(service_name="body-test"))
    app.test_client().post("/submit", data="secret-body")

    logs = wait_for_logs(mock_agent, lambda items: any(log.attributes.get("http.method") == "POST" for log in items))
    log = next(item for item in logs if item.attributes.get("http.method") == "POST")
    assert "http.request.body" not in log.attributes


def test_init_idempotency_no_duplicate_exception_events(mock_agent, init_kwargs):
    greplog.init(**init_kwargs(service_name="idempotent-test"))
    greplog.init(**init_kwargs(service_name="idempotent-test"))

    try:
        raise RuntimeError("single idempotent exception")
    except RuntimeError:
        exc_type, exc_value, exc_tb = sys.exc_info()
        sys.excepthook(exc_type, exc_value, exc_tb)

    time.sleep(0.5)
    matches = [log for log in mock_agent.all_logs() if log.message == "single idempotent exception"]
    assert len(matches) == 1

    import greplog.logging_handler as logging_handler

    assert logging.getLogger().handlers.count(logging_handler.get_greplog_handler()) <= 1
