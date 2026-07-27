"""Shared SDK state and event queue."""

from __future__ import annotations

import os
import time
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from greplog.redact import redact_attributes
from greplog.serialize import encode_ingest_batch
from greplog.ulid import generate_ulid

if TYPE_CHECKING:
    from greplog.transport import Transport


@dataclass
class GreplogConfig:
    service_name: str
    instance_id: str
    capture_bodies: bool = False
    capture_log_level: str = "WARNING"


@dataclass
class SDKState:
    initialized: bool = False
    config: Optional[GreplogConfig] = None
    transport: Optional["Transport"] = None
    event_queue: List[Dict[str, Any]] = field(default_factory=list)
    batch_seq: int = 0


state = SDKState()


def detect_service_name() -> str:
    import re

    if os.path.exists("pyproject.toml"):
        try:
            with open("pyproject.toml", "r", encoding="utf-8") as fh:
                content = fh.read()
            match = re.search(r'name\s*=\s*"([^"]+)"', content)
            if match:
                return match.group(1)
        except Exception:
            pass

    if os.path.exists("setup.py"):
        try:
            with open("setup.py", "r", encoding="utf-8") as fh:
                content = fh.read()
            for line in content.splitlines():
                if "name=" in line.replace(" ", ""):
                    _, _, rest = line.partition("=")
                    cleaned = rest.strip().strip("'\"")
                    if cleaned:
                        return cleaned
        except Exception:
            pass

    return "unknown-service"


def generate_instance_id() -> str:
    return generate_ulid()


def timestamp_ns() -> int:
    return time.time_ns()


def flush_event_queue() -> None:
    try:
        if state.transport is None or not state.event_queue:
            return

        events = state.event_queue[:100]
        del state.event_queue[:100]

        service_name = state.config.service_name if state.config else detect_service_name()
        instance_id = state.config.instance_id if state.config else generate_instance_id()

        for event in events:
            attrs = event.get("attributes") or {}
            event["attributes"] = redact_attributes({k: str(v) for k, v in attrs.items()})
            if not event.get("event_id"):
                event["event_id"] = generate_ulid()

        state.batch_seq += 1
        payload = encode_ingest_batch(
            service_name=service_name,
            instance_id=instance_id,
            batch_seq=state.batch_seq,
            logs=events,
        )
        state.transport.send(payload)
    except Exception:
        pass


def push_event(event: Dict[str, Any]) -> None:
    try:
        if not event.get("event_id"):
            event["event_id"] = generate_ulid()
        state.event_queue.append(event)
        if len(state.event_queue) > 1000:
            state.event_queue.pop(0)
        flush_event_queue()
    except Exception:
        pass


def details_to_attributes(details: Optional[Dict[str, Any]]) -> Dict[str, str]:
    if not details:
        return {}
    return {str(k): str(v) for k, v in details.items()}


def build_manual_event(message: str, level: str, details: Optional[Dict[str, Any]] = None) -> None:
    try:
        service_name = state.config.service_name if state.config else detect_service_name()
        push_event(
            {
                "service_name": service_name,
                "message": message,
                "level": level,
                "timestamp_ns": timestamp_ns(),
                "logger_name": "greplog",
                "attributes": details_to_attributes(details),
            }
        )
    except Exception:
        pass
