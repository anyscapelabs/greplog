"""Protobuf serialization for IngestBatch wire frames."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from greplog.proto.greplog.v1 import events_pb2
from greplog.ulid import generate_ulid


def encode_length_prefixed_frame(payload: bytes) -> bytes:
    return len(payload).to_bytes(4, byteorder="little") + payload


def _build_log_event(data: Dict[str, Any]) -> events_pb2.LogEvent:
    event = events_pb2.LogEvent(
        service_name=data.get("service_name", ""),
        message=data.get("message", ""),
        level=data.get("level", ""),
        timestamp_ns=int(data.get("timestamp_ns", 0)),
        event_id=data.get("event_id") or generate_ulid(),
    )

    if data.get("logger_name"):
        event.logger_name = data["logger_name"]
    if data.get("file"):
        event.file = data["file"]
    if data.get("line") is not None:
        event.line = int(data["line"])
    if data.get("correlation_id"):
        event.correlation_id = data["correlation_id"]
    if data.get("attributes"):
        for key, val in data["attributes"].items():
            event.attributes[key] = str(val)
    if data.get("stack_trace"):
        event.stack_trace.extend(data["stack_trace"])
    if data.get("exception_type"):
        event.exception_type = data["exception_type"]
    if data.get("exception_message"):
        event.exception_message = data["exception_message"]

    return event


def encode_ingest_batch(
    service_name: str,
    instance_id: str,
    batch_seq: int,
    logs: List[Dict[str, Any]],
    spans: Optional[List[Any]] = None,
    metrics: Optional[List[Any]] = None,
) -> bytes:
    batch = events_pb2.IngestBatch(
        service_name=service_name,
        instance_id=instance_id,
        batch_seq=batch_seq,
    )
    batch.logs.extend(_build_log_event(log) for log in logs)
    # spans and metrics intentionally empty for now
    _ = spans, metrics
    return batch.SerializeToString()
