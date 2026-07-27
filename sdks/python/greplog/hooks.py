"""Exception hooks for main-thread and thread uncaught exceptions."""

from __future__ import annotations

import sys
import threading
import traceback
from types import TracebackType
from typing import Callable, Optional, Type

from greplog.internal import push_event, state, timestamp_ns

_ORIGINAL_SYS_EXCEPTHOOK = sys.excepthook
_ORIGINAL_THREAD_EXCEPTHOOK = getattr(threading, "excepthook", None)

_prev_excepthook: Optional[
    Callable[[Type[BaseException], BaseException, Optional[TracebackType]], None]
] = None
_prev_thread_excepthook: Optional[Callable[[threading.ExceptHookArgs], None]] = None
_hooks_registered = False


def reset_hook_flags() -> None:
    global _hooks_registered, _prev_excepthook, _prev_thread_excepthook

    try:
        sys.excepthook = _ORIGINAL_SYS_EXCEPTHOOK
    except Exception:
        pass

    try:
        if hasattr(threading, "excepthook") and _ORIGINAL_THREAD_EXCEPTHOOK is not None:
            threading.excepthook = _ORIGINAL_THREAD_EXCEPTHOOK
    except Exception:
        pass

    _hooks_registered = False
    _prev_excepthook = None
    _prev_thread_excepthook = None


def _format_stack(tb: Optional[TracebackType]) -> list[str]:
    if tb is None:
        return []
    try:
        return traceback.format_tb(tb)
    except Exception:
        return []


def _capture_exception(
    exc_type: Type[BaseException],
    exc_value: BaseException,
    exc_tb: Optional[TracebackType],
    *,
    logger_name: str,
    level: str,
) -> None:
    try:
        service_name = state.config.service_name if state.config else "unknown-service"
        message = str(exc_value) if exc_value is not None else exc_type.__name__
        push_event(
            {
                "service_name": service_name,
                "message": message,
                "level": level,
                "timestamp_ns": timestamp_ns(),
                "logger_name": logger_name,
                "stack_trace": _format_stack(exc_tb),
                "exception_type": getattr(exc_type, "__name__", "Exception"),
                "exception_message": message,
                "attributes": {},
            }
        )
    except Exception:
        pass


def _greplog_excepthook(
    exc_type: Type[BaseException],
    exc_value: BaseException,
    exc_tb: Optional[TracebackType],
) -> None:
    _capture_exception(exc_type, exc_value, exc_tb, logger_name="sys.excepthook", level="fatal")
    hook = _prev_excepthook or _ORIGINAL_SYS_EXCEPTHOOK
    if hook is not _greplog_excepthook:
        hook(exc_type, exc_value, exc_tb)


def _greplog_thread_excepthook(args: threading.ExceptHookArgs) -> None:
    _capture_exception(
        args.exc_type,
        args.exc_value,
        args.exc_traceback,
        logger_name="threading.excepthook",
        level="fatal",
    )
    hook = _prev_thread_excepthook or _ORIGINAL_THREAD_EXCEPTHOOK
    if hook is not None and hook is not _greplog_thread_excepthook:
        hook(args)


def register_error_hooks() -> None:
    global _hooks_registered, _prev_excepthook, _prev_thread_excepthook

    if _hooks_registered:
        return
    _hooks_registered = True

    try:
        current = sys.excepthook
        _prev_excepthook = current if current is not _greplog_excepthook else _ORIGINAL_SYS_EXCEPTHOOK
        sys.excepthook = _greplog_excepthook
    except Exception:
        pass

    try:
        if hasattr(threading, "excepthook"):
            current = threading.excepthook
            _prev_thread_excepthook = (
                current if current is not _greplog_thread_excepthook else _ORIGINAL_THREAD_EXCEPTHOOK
            )
            threading.excepthook = _greplog_thread_excepthook
    except Exception:
        pass
