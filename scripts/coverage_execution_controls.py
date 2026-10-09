"""Small source and transport facts used by future execution-control receipts."""

from __future__ import annotations

import hashlib
import math
from pathlib import Path

import httpx


def module_source_sha256(module_file: str) -> str:
    """Hash the exact current producer module bytes; callers bind this in a sealed receipt."""
    path = Path(module_file)
    if path.is_symlink() or not path.is_file():
        raise ValueError("execution-control-source-unavailable")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def request_timeout_seconds(request: httpx.Request) -> float | None:
    """Return the applied HTTPX timeout extension only when every field is finite and positive."""
    extension = request.extensions.get("timeout")
    if not isinstance(extension, dict) or not extension:
        return None
    values = tuple(extension.values())
    if any(
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(float(value))
        or float(value) <= 0
        for value in values
    ):
        return None
    return max(float(value) for value in values)


MODULE_SOURCE_SHA256 = module_source_sha256(__file__)
