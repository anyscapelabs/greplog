"""Manual API tests."""

from __future__ import annotations

import greplog
from conftest import wait_for_logs


def test_exports_all_level_functions():
    assert callable(greplog.error)
    assert callable(greplog.warn)
    assert callable(greplog.info)
    assert callable(greplog.debug)


def test_exports_init():
    assert callable(greplog.init)


def test_accepts_optional_details():
    greplog.info("no details")
    greplog.info("with details", {"key": "val"})
    greplog.info("empty details", {})


def test_manual_api_before_and_after_init(mock_agent, init_kwargs):
    greplog.error("before-init", {"phase": "before"})
    greplog.init(**init_kwargs())
    greplog.error("after-init", {"phase": "after"})

    logs = wait_for_logs(mock_agent, lambda items: any(log.message == "after-init" for log in items))
    messages = {log.message for log in logs}
    assert "after-init" in messages


def test_manual_api_independent_of_init(mock_agent, init_kwargs):
    greplog.error("no-init-call")
    greplog.init(**init_kwargs())
    greplog.error("with-init-call")

    logs = wait_for_logs(mock_agent, lambda items: any(log.message == "with-init-call" for log in items))
    assert any(log.message == "with-init-call" for log in logs)
