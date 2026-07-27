"""ULID generation matching the Node SDK implementation."""

from __future__ import annotations

import random

_ENCODING = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
_ENCODING_LEN = len(_ENCODING)

_last_timestamp = 0
_last_random = 0


def generate_ulid() -> str:
    global _last_timestamp, _last_random

    now = int(__import__("time").time() * 1000)
    if now != _last_timestamp:
        _last_timestamp = now
        _last_random = 0
    _last_random += 1

    return _timestamp_to_ulid(now) + _random_to_ulid(_last_random)


def _timestamp_to_ulid(ts: int) -> str:
    chars: list[str] = []
    val = ts
    for _ in range(10):
        chars.append(_ENCODING[val % _ENCODING_LEN])
        val //= _ENCODING_LEN
    return "".join(reversed(chars))


def _random_to_ulid(seq: int) -> str:
    chars: list[str] = []
    for i in range(16):
        if i < 4:
            idx = random.randint(0, _ENCODING_LEN - 1)
        else:
            idx = ((seq >> ((i - 4) * 5)) + random.randint(0, 7)) % _ENCODING_LEN
        chars.append(_ENCODING[idx])
    return "".join(chars)
