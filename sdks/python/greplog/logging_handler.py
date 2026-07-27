"""Root logger handler for auto-capturing stdlib logging output."""

from __future__ import annotations

import logging
from typing import Optional

from greplog.internal import push_event, state, timestamp_ns

_greplog_handler: Optional["GreplogHandler"] = None
_handler_registered = False


def reset_handler_flags() -> None:
    global _handler_registered, _greplog_handler
    _handler_registered = False
    _greplog_handler = None


class GreplogHandler(logging.Handler):
    def emit(self, record: logging.LogRecord) -> None:
        try:
            if record.levelno < self.level:
                return

            level_map = {
                logging.CRITICAL: "fatal",
                logging.ERROR: "error",
                logging.WARNING: "warn",
                logging.INFO: "info",
                logging.DEBUG: "debug",
            }
            level = level_map.get(record.levelno, "info")
            service_name = state.config.service_name if state.config else "unknown-service"

            attrs = {}
            if record.name:
                attrs["logger"] = record.name
            if record.pathname:
                attrs["file"] = record.pathname
            if record.lineno:
                attrs["line"] = str(record.lineno)

            message = record.getMessage()
            push_event(
                {
                    "service_name": service_name,
                    "message": message,
                    "level": level,
                    "timestamp_ns": timestamp_ns(),
                    "logger_name": record.name or "logging",
                    "file": record.pathname,
                    "line": record.lineno,
                    "attributes": attrs,
                }
            )
        except Exception:
            pass


def register_logging_handler(capture_log_level: str = "WARNING") -> None:
    global _handler_registered, _greplog_handler

    if _handler_registered:
        return
    _handler_registered = True

    try:
        level = getattr(logging, capture_log_level.upper(), logging.WARNING)
        _greplog_handler = GreplogHandler(level=level)
        logging.getLogger().addHandler(_greplog_handler)
    except Exception:
        pass


def get_greplog_handler() -> Optional[GreplogHandler]:
    return _greplog_handler
