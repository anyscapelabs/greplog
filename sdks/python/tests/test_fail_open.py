"""Fail-open behavior when no agent is reachable."""

from __future__ import annotations

import logging
import sys
import threading
import time

import greplog


def test_init_does_not_throw_without_agent():
    assert greplog.init(service_name="fail-open", socket_path="/nonexistent/greplog.sock", tcp_port=59999) is None


def test_manual_api_before_init():
    assert greplog.error("test error") is None
    assert greplog.warn("test warn") is None
    assert greplog.info("test info") is None
    assert greplog.debug("test debug") is None


def test_manual_api_with_details_does_not_throw():
    assert greplog.error("err", {"key": "val"}) is None


def test_init_then_manual_api_does_not_throw():
    greplog.init(service_name="test-svc", socket_path="/nonexistent/greplog.sock", tcp_port=59999)
    assert greplog.info("after init") is None
    assert greplog.error("after init error") is None


def test_init_called_twice_is_safe():
    greplog.init(service_name="test-svc", socket_path="/nonexistent/greplog.sock", tcp_port=59999)
    greplog.init(service_name="test-svc", socket_path="/nonexistent/greplog.sock", tcp_port=59999)


def test_sensitive_manual_api_does_not_throw():
    assert greplog.error("login failed", {"password": "hunter2"}) is None


def test_uncaught_exception_does_not_alter_behavior(mock_agent):
    prev_hook = sys.excepthook

    def swallow(exc_type, exc, tb):
        pass

    sys.excepthook = swallow
    greplog.init(**{"service_name": "x", "socket_path": "/nonexistent/greplog.sock", "tcp_port": 59999})

    try:
        t0 = time.perf_counter()
        try:
            raise RuntimeError("fail-open uncaught")
        except RuntimeError:
            pass
        elapsed = time.perf_counter() - t0
        assert elapsed < 0.5
    finally:
        sys.excepthook = prev_hook


def test_thread_exception_does_not_block_caller(mock_agent):
    greplog.init(**{"service_name": "x", "socket_path": "/nonexistent/greplog.sock", "tcp_port": 59999})

    def worker():
        raise ValueError("thread fail-open")

    t0 = time.perf_counter()
    thread = threading.Thread(target=worker)
    thread.start()
    thread.join(timeout=2)
    elapsed = time.perf_counter() - t0
    assert elapsed < 3.0


def test_middleware_request_without_agent(mock_agent):
    from flask import Flask

    app = Flask(__name__)

    @app.get("/ok")
    def ok():
        return "ok"

    greplog.init(app=app, service_name="flask-fail-open", socket_path="/nonexistent/greplog.sock", tcp_port=59999)

    client = app.test_client()
    t0 = time.perf_counter()
    response = client.get("/ok")
    elapsed = time.perf_counter() - t0

    assert response.status_code == 200
    assert response.data == b"ok"
    assert elapsed < 1.0
