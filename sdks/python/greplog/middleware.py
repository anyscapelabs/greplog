"""WSGI/ASGI middleware for automatic HTTP request capture."""

from __future__ import annotations

import inspect
import time
from typing import Any, Callable, Dict, Iterable, Mapping, MutableMapping, Optional, Tuple

from greplog.internal import push_event, state, timestamp_ns
from greplog.redact import redact_attributes, redact_headers
from greplog.ulid import generate_ulid

_apps_wrapped: set[int] = set()


def reset_middleware_flags() -> None:
    _apps_wrapped.clear()


def _status_code_from_status(status: str) -> int:
    try:
        return int(status.split(" ", 1)[0])
    except Exception:
        return 0


def _capture_http_event(
    *,
    method: str,
    route: str,
    status_code: int,
    latency_ms: int,
    headers: Optional[Mapping[str, str]] = None,
    start_time_ns: Optional[int] = None,
) -> None:
    try:
        service_name = state.config.service_name if state.config else "unknown-service"
        capture_bodies = state.config.capture_bodies if state.config else False

        attrs: Dict[str, str] = {
            "http.method": method,
            "http.route": route,
            "http.status_code": str(status_code),
            "http.latency_ms": str(latency_ms),
        }
        if capture_bodies:
            attrs["http.request.body"] = "(body captured)"

        if headers:
            attrs.update(redact_headers({k: str(v) for k, v in headers.items()}))

        level = "error" if status_code >= 500 else "warn" if status_code >= 400 else "info"

        push_event(
            {
                "service_name": service_name,
                "message": f"{method} {route} → {status_code} ({latency_ms}ms)",
                "level": level,
                "timestamp_ns": start_time_ns or timestamp_ns(),
                "logger_name": "greplog.http",
                "event_id": generate_ulid(),
                "attributes": redact_attributes(attrs),
            }
        )
    except Exception:
        pass


class WSGIMiddleware:
    def __init__(self, app: Callable) -> None:
        self.app = app

    def __call__(
        self,
        environ: MutableMapping[str, Any],
        start_response: Callable[[str, list], Any],
    ) -> Iterable[bytes]:
        start = time.time()
        start_ns = timestamp_ns()
        status_holder: Dict[str, Any] = {"code": 0}

        def capturing_start_response(status: str, response_headers: list, exc_info=None):
            status_holder["code"] = _status_code_from_status(status)
            return start_response(status, response_headers, exc_info)

        try:
            return self.app(environ, capturing_start_response)
        finally:
            latency_ms = int((time.time() - start) * 1000)
            method = str(environ.get("REQUEST_METHOD", "GET"))
            route = str(environ.get("PATH_INFO", "/"))
            headers: Dict[str, str] = {}
            for key, val in environ.items():
                if key.startswith("HTTP_"):
                    header_name = key[5:].replace("_", "-").title()
                    headers[header_name] = str(val)
            _capture_http_event(
                method=method,
                route=route,
                status_code=int(status_holder["code"] or 0),
                latency_ms=latency_ms,
                headers=headers,
                start_time_ns=start_ns,
            )


class ASGIMiddleware:
    def __init__(self, app: Callable) -> None:
        self.app = app

    async def __call__(self, scope: dict, receive: Callable, send: Callable) -> None:
        if scope.get("type") != "http":
            await self.app(scope, receive, send)
            return

        start = time.time()
        start_ns = timestamp_ns()
        status_holder = {"code": 0}

        async def capturing_send(message: dict) -> None:
            if message.get("type") == "http.response.start":
                status_holder["code"] = int(message.get("status", 0))
            await send(message)

        try:
            await self.app(scope, receive, capturing_send)
        finally:
            latency_ms = int((time.time() - start) * 1000)
            method = str(scope.get("method", "GET"))
            route = str(scope.get("path", "/"))
            headers = {k.decode(): v.decode() for k, v in scope.get("headers", [])}
            _capture_http_event(
                method=method,
                route=route,
                status_code=int(status_holder["code"] or 0),
                latency_ms=latency_ms,
                headers=headers,
                start_time_ns=start_ns,
            )


def _is_asgi_app(app: Any) -> bool:
    if inspect.iscoroutinefunction(app):
        return True
    call = getattr(app, "__call__", None)
    return call is not None and inspect.iscoroutinefunction(call)


def wrap_app(app: Any) -> Any:
    app_id = id(app)
    if app_id in _apps_wrapped:
        return app

    try:
        if hasattr(app, "wsgi_app"):
            if not isinstance(app.wsgi_app, WSGIMiddleware):
                app.wsgi_app = WSGIMiddleware(app.wsgi_app)
            _apps_wrapped.add(app_id)
            return app

        if _is_asgi_app(app):
            if hasattr(app, "add_middleware"):
                try:
                    app.add_middleware(ASGIMiddleware)
                    _apps_wrapped.add(app_id)
                    return app
                except Exception:
                    pass

            original_call = app.__call__
            middleware = ASGIMiddleware(original_call)

            async def patched_call(scope: dict, receive: Callable, send: Callable) -> None:
                await middleware(scope, receive, send)

            app.__call__ = patched_call
            _apps_wrapped.add(app_id)
            return app
    except Exception:
        pass

    return app
