"""SDK-side redaction matching Round 1 defaults and modes."""

from __future__ import annotations

from enum import Enum
from typing import Dict, Mapping, Optional


class RedactionMode(str, Enum):
    FULL = "Full"
    PARTIAL = "Partial"
    HASH = "Hash"


DEFAULT_REDACTED_KEYS: Dict[str, RedactionMode] = {
    "password": RedactionMode.FULL,
    "token": RedactionMode.FULL,
    "secret": RedactionMode.FULL,
    "email": RedactionMode.PARTIAL,
}


def _redact_string(val: str, mode: RedactionMode) -> str:
    if not val:
        return ""

    if mode is RedactionMode.FULL:
        return "[REDACTED]"

    if mode is RedactionMode.PARTIAL:
        if len(val) <= 4:
            return "[***]"
        return f"{val[:2]}***{val[-2:]}"

    # Hash mode — same 32-bit rolling hash as the Node SDK
    hash_val = 0
    for ch in val:
        hash_val = ((hash_val << 5) - hash_val + ord(ch)) & 0xFFFFFFFF
    return f"[HASH:{hash_val:08x}]"


def _key_matches_pattern(key: str, pattern: str) -> bool:
    return pattern.lower() in key.lower()


def redact_attributes(
    attrs: Mapping[str, str],
    custom_keys: Optional[Mapping[str, RedactionMode]] = None,
) -> Dict[str, str]:
    merged = {**DEFAULT_REDACTED_KEYS, **(custom_keys or {})}
    result: Dict[str, str] = {}

    for key, val in attrs.items():
        matched = False
        for pattern, mode in merged.items():
            if _key_matches_pattern(key, pattern):
                result[key] = _redact_string(val, mode)
                matched = True
                break
        if not matched:
            result[key] = val

    return result


def redact_headers(headers: Mapping[str, str]) -> Dict[str, str]:
    return redact_attributes(dict(headers))
