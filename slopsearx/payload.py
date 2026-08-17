"""Versioned, JSON-safe domain payloads for normalized search results.

A *payload* is an optional structured field attached to a
:class:`~slopsearx.adapter.SearchResult`. It preserves source-specific
structured meaning that does not fit the common search-result envelope, while
the envelope itself stays stable and generic. Results without a payload remain
valid; consumers that do not understand a payload can ignore it and still use
the common fields.

The payload envelope is deliberately small and self-describing so a consumer
can branch on the domain/type without knowing the adapter:

    {
      "domain": "security",
      "type": "vulnerability",
      "schema_version": 1,
      "data": { ...domain-typed fields... },
      "provenance": {
        "engine": "nvd",
        "adapter_fields": [...],     # fields reported by the source adapter
        "normalized_fields": [...],  # fields mapped from the common envelope
        "inferred_fields": [...],    # fields derived by the pipeline
      },
    }

Provenance distinguishes source-reported fields from normalized/inferred ones.
Payload ``data`` never invents fields the adapter did not return: ``None``
values are dropped at build time, so an absent source field stays absent
rather than materializing as a fabricated ``null``.
"""

from __future__ import annotations

import json
import os
from typing import Any, cast

# ---------------------------------------------------------------------------
# Contract constants
# ---------------------------------------------------------------------------

# Version of the payload *envelope* schema (not the per-domain data schema).
PAYLOAD_SCHEMA_VERSION = 1

# Compact-disclosure inline threshold (bytes). Compact surfaces — the MCP
# result cards and the public, unauthenticated HTTP /search output — inline a
# payload only when its serialized form is this small. Larger payloads are
# omitted from compact output and stay available through the full-record read
# path (MCP ``slopsearx_read_result``).
PAYLOAD_INLINE_BYTES = 512

# Persistence cap (bytes). The canonical cache/snapshot form stores a payload
# only when its serialized size is at most this bound. It is deliberately
# distinct from the 512-byte compact-disclosure cap above: compact surfaces
# hide payloads to keep triage cards small, while this bound keeps the shared
# Valkey cache and snapshots from growing without limit when an adapter
# reports a very large structured payload. Overridable via the
# ``PAYLOAD_MAX_PERSIST_BYTES`` environment variable.
PAYLOAD_MAX_PERSIST_BYTES = 16_384

# Stable domain-family identifiers. Families group payload types that share a
# common shape so consumers can branch on the family without knowing every
# adapter.
DOMAIN_SECURITY = "security"
DOMAIN_SCIENCE = "science"
DOMAIN_PACKAGES = "packages"
DOMAIN_JOBS = "jobs"
DOMAIN_MEDIA = "media"
DOMAIN_FINANCIAL = "financial"
DOMAIN_BIOMEDICAL = "biomedical"


# ---------------------------------------------------------------------------
# Serialization helpers
# ---------------------------------------------------------------------------


# Marker emitted when a payload contains a reference cycle. Cycles cannot be
# represented in JSON, so the canonicalizer replaces the back-reference with a
# deterministic string instead of recursing forever (RecursionError).
_CIRCULAR_REF_MARKER = "<circular reference>"

# Recursion bound for ``_json_safe``. Payloads nested deeper than this are
# structurally pathological and would otherwise recurse until RecursionError;
# the container at the bound is replaced with a deterministic marker instead.
_JSON_SAFE_MAX_DEPTH = 100
_DEPTH_LIMIT_MARKER = "<max depth exceeded>"


def _json_safe(value: Any, _seen: set[int] | None = None, _depth: int = 0) -> Any:
    """Deep-convert a value to JSON-safe primitives.

    Dict keys are stringified; lists/tuples become lists (order preserved);
    sets/frozensets become deterministically sorted lists; ``None`` and scalar
    primitives are returned unchanged. Anything else (e.g. a stray datetime)
    is stringified as a deterministic last resort. Reference cycles are
    replaced with ``<circular reference>`` and containers nested beyond
    ``_JSON_SAFE_MAX_DEPTH`` with ``<max depth exceeded>`` rather than
    recursing forever.
    """
    if _seen is None:
        _seen = set()

    if _depth > _JSON_SAFE_MAX_DEPTH:
        return _DEPTH_LIMIT_MARKER

    if isinstance(value, dict):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return {str(key): _json_safe(item, _seen, _depth + 1) for key, item in value.items()}
        finally:
            _seen.discard(marker)

    if isinstance(value, (list, tuple)):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return [_json_safe(item, _seen, _depth + 1) for item in value]
        finally:
            _seen.discard(marker)

    if isinstance(value, (set, frozenset)):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return [_json_safe(item, _seen, _depth + 1) for item in sorted(value, key=repr)]
        finally:
            _seen.discard(marker)

    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    return str(value)


def payload_serialized_size(payload: dict[str, Any] | None) -> int | None:
    """Approximate serialized byte size of a payload envelope.

    Returns ``None`` when the payload cannot be JSON-serialized so callers can
    conservatively omit it — an unserializable payload is never treated as the
    smallest possible payload and inlined by mistake.
    """
    if not isinstance(payload, dict):
        return None
    try:
        return len(json.dumps(payload, separators=(",", ":"), allow_nan=False).encode("utf-8"))
    except (TypeError, ValueError, RecursionError):
        return None


def payload_max_persist_bytes() -> int:
    """Return the effective payload persistence bound in bytes.

    Honors the ``PAYLOAD_MAX_PERSIST_BYTES`` environment variable, falling
    back to :data:`PAYLOAD_MAX_PERSIST_BYTES` when unset or invalid.
    """
    raw = os.environ.get("PAYLOAD_MAX_PERSIST_BYTES")
    if raw is None:
        return PAYLOAD_MAX_PERSIST_BYTES
    try:
        value = int(raw.strip())
    except (TypeError, ValueError):
        return PAYLOAD_MAX_PERSIST_BYTES
    return value if value >= 0 else PAYLOAD_MAX_PERSIST_BYTES


def payload_for_persistence(
    payload: dict[str, Any] | None,
    max_bytes: int | None = None,
) -> dict[str, Any] | None:
    """Return the canonical form of a payload to persist, or ``None``.

    The persisted (cache/snapshot) form is bounded independently of the
    512-byte compact-disclosure cap. The payload is canonicalized exactly once
    through :func:`payload_to_dict` (sets/tuples → lists, JSON-safe
    primitives, cycle/depth markers); then, if the canonical form's serialized
    size exceeds ``max_bytes`` (default: :func:`payload_max_persist_bytes`),
    it is omitted so the shared store never absorbs unbounded payloads. An
    unserializable payload is omitted as well.
    """
    canonical = payload_to_dict(payload)
    if canonical is None:
        return None
    size = payload_serialized_size(canonical)
    if size is None:
        return None
    limit = payload_max_persist_bytes() if max_bytes is None else max_bytes
    if size > limit:
        return None
    return canonical


def build_payload(
    domain: str,
    payload_type: str,
    data: dict[str, Any],
    *,
    engine: str | None = None,
    normalized_fields: tuple[str, ...] = (),
    inferred_fields: tuple[str, ...] = (),
    schema_version: int = PAYLOAD_SCHEMA_VERSION,
) -> dict[str, Any]:
    """Build a versioned, JSON-safe payload envelope.

    ``None`` values in ``data`` are dropped so absent source fields stay
    absent. ``adapter_fields`` is derived from the retained ``data`` keys:
    everything inside ``data`` is source-reported by construction.
    """
    compact = {key: value for key, value in data.items() if value is not None}
    return {
        "domain": domain,
        "type": payload_type,
        "schema_version": int(schema_version),
        "data": _json_safe(compact),
        "provenance": {
            "engine": engine,
            "adapter_fields": list(compact.keys()),
            "normalized_fields": list(normalized_fields),
            "inferred_fields": list(inferred_fields),
        },
    }


def payload_to_dict(payload: dict[str, Any] | None) -> dict[str, Any] | None:
    """Return a JSON-safe copy of a payload envelope, or ``None``."""
    if not isinstance(payload, dict):
        return None
    return cast("dict[str, Any]", _json_safe(payload))


def payload_from_dict(value: Any) -> dict[str, Any] | None:
    """Rehydrate a payload envelope from a serialized value, or ``None``.

    Accepts a dict; anything else (missing, legacy stringified, malformed)
    yields ``None`` so a broken payload never crashes the read path.
    """
    if not isinstance(value, dict):
        return None
    return cast("dict[str, Any]", _json_safe(value))


def is_valid_payload(payload: dict[str, Any] | None) -> bool:
    """Whether ``payload`` satisfies the minimal self-describing contract."""
    if not isinstance(payload, dict):
        return False
    domain = payload.get("domain")
    payload_type = payload.get("type")
    schema_version = payload.get("schema_version")
    if not isinstance(domain, str) or not domain:
        return False
    if not isinstance(payload_type, str) or not payload_type:
        return False
    if not isinstance(schema_version, int):
        return False
    if not isinstance(payload.get("data"), dict):
        return False
    return True
