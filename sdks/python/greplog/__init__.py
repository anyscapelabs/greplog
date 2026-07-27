"""Greplog Python SDK — automatic capture with fail-open guarantees."""

from __future__ import annotations

from typing import Any, Dict, Optional

from greplog.detect import detect_framework, write_config
from greplog.hooks import register_error_hooks
from greplog.internal import (
    GreplogConfig,
    build_manual_event,
    detect_service_name,
    generate_instance_id,
    state,
)
from greplog.logging_handler import register_logging_handler
from greplog.middleware import wrap_app
from greplog.transport import Transport


def error(message: str, details: Optional[Dict[str, Any]] = None) -> None:
    build_manual_event(message, "error", details)


def warn(message: str, details: Optional[Dict[str, Any]] = None) -> None:
    build_manual_event(message, "warn", details)


def info(message: str, details: Optional[Dict[str, Any]] = None) -> None:
    build_manual_event(message, "info", details)


def debug(message: str, details: Optional[Dict[str, Any]] = None) -> None:
    build_manual_event(message, "debug", details)


def init(
    *,
    capture_bodies: bool = False,
    capture_log_level: str = "WARNING",
    service_name: Optional[str] = None,
    socket_path: Optional[str] = None,
    tcp_port: Optional[int] = None,
    app: Any = None,
) -> None:
    try:
        if state.initialized:
            return
        state.initialized = True

        state.config = GreplogConfig(
            service_name=service_name or detect_service_name(),
            instance_id=generate_instance_id(),
            capture_bodies=capture_bodies,
            capture_log_level=capture_log_level,
        )

        state.transport = Transport(
            socket_path=socket_path,
            tcp_port=tcp_port or 4318,
        )

        register_error_hooks()
        register_logging_handler(capture_log_level=capture_log_level)

        if app is not None:
            wrap_app(app)

        try:
            detection = detect_framework()
            write_config(detection)
        except Exception:
            pass

        state.transport.connect()
    except Exception:
        state.initialized = False


def shutdown() -> None:
    try:
        if state.transport is not None:
            state.transport.destroy()
            state.transport = None
        state.initialized = False
    except Exception:
        pass


__all__ = [
    "init",
    "shutdown",
    "error",
    "warn",
    "info",
    "debug",
]
