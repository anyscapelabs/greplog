"""Redaction tests."""

from __future__ import annotations

import greplog
from conftest import wait_for_logs
from greplog.redact import RedactionMode, redact_attributes


def test_password_full_redaction():
    assert redact_attributes({"password": "my-secret-pass"})["password"] == "[REDACTED]"


def test_email_partial_redaction():
    assert redact_attributes({"email": "user@example.com"})["email"] == "us***om"


def test_case_insensitive_keys():
    result = redact_attributes({"Password": "secret123", "TOKEN": "xyz"})
    assert result["Password"] == "[REDACTED]"
    assert result["TOKEN"] == "[REDACTED]"


def test_redaction_applies_to_manual_and_http(mock_agent, init_kwargs):
    from flask import Flask

    app = Flask(__name__)

    @app.post("/login")
    def login():
        return {"ok": True}, 200

    greplog.init(app=app, **init_kwargs())
    greplog.error("manual login failed", {"password": "hunter2"})

    client = app.test_client()
    client.post("/login", json={"password": "hunter2"})

    logs = wait_for_logs(
        mock_agent,
        lambda items: any(log.message == "manual login failed" for log in items),
    )
    manual = next(log for log in logs if log.message == "manual login failed")
    assert manual.attributes["password"] == "[REDACTED]"

    http_logs = [log for log in logs if log.logger_name == "greplog.http"]
    assert http_logs
    # HTTP attrs should not leak raw password values in attributes
    for log in http_logs:
        for val in log.attributes.values():
            assert val != "hunter2"
