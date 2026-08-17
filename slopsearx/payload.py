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

# The provenance kinds carried in ``provenance``.
PROVENANCE_KINDS: tuple[str, ...] = ("adapter", "normalized", "inferred")

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

DOMAIN_FAMILIES: tuple[str, ...] = (
    DOMAIN_SECURITY,
    DOMAIN_SCIENCE,
    DOMAIN_PACKAGES,
    DOMAIN_JOBS,
    DOMAIN_MEDIA,
    DOMAIN_FINANCIAL,
    DOMAIN_BIOMEDICAL,
)

# Well-known payload types per family (the initial typed schemas).
PAYLOAD_TYPES: dict[str, tuple[str, ...]] = {
    DOMAIN_SECURITY: ("vulnerability",),
    DOMAIN_SCIENCE: ("publication",),
    DOMAIN_PACKAGES: ("package",),
    DOMAIN_JOBS: ("job",),
    DOMAIN_MEDIA: ("media_item",),
    DOMAIN_FINANCIAL: ("economic_series",),
    DOMAIN_BIOMEDICAL: ("drug_label",),
}

# Initial typed field schemas. Each documents the *possible* fields of a
# payload type; adapters include only the fields their source actually
# returned, so an absent field is absent — never fabricated.
PAYLOAD_FIELDS: dict[str, dict[str, tuple[str, ...]]] = {
    DOMAIN_SECURITY: {
        "vulnerability": (
            "cve_id",
            "description",
            "cvss",
            "cwe_ids",
            "references",
        ),
    },
    DOMAIN_SCIENCE: {
        "publication": (
            "publication_id",
            "authors",
            "journal",
            "abstract",
        ),
    },
    DOMAIN_PACKAGES: {
        "package": (
            "name",
            "version",
            "summary",
            "license",
            "homepage",
        ),
    },
    DOMAIN_JOBS: {
        "job": (
            "company",
            "title",
            "location",
            "salary",
            "job_id",
        ),
    },
    DOMAIN_MEDIA: {
        "media_item": (
            "media_type",
            "title",
            "release_date",
            "overview",
            "vote_average",
        ),
    },
    DOMAIN_FINANCIAL: {
        "economic_series": (
            "series_id",
            "title",
            "units",
            "frequency",
            "seasonal_adjustment",
            "observation_start",
            "notes",
        ),
    },
    DOMAIN_BIOMEDICAL: {
        "drug_label": (
            "brand_name",
            "generic_name",
            "manufacturer",
            "substance",
            "purpose",
            "indications",
        ),
    },
}


# ---------------------------------------------------------------------------
# Serialization helpers
# ---------------------------------------------------------------------------


# Marker emitted when a payload contains a reference cycle. Cycles cannot be
# represented in JSON, so the canonicalizer replaces the back-reference with a
# deterministic string instead of recursing forever (RecursionError).
_CIRCULAR_REF_MARKER = "<circular reference>"


def _json_safe(value: Any, _seen: set[int] | None = None) -> Any:
    """Deep-convert a value to JSON-safe primitives.

    Dict keys are stringified; lists/tuples become lists (order preserved);
    sets/frozensets become deterministically sorted lists; ``None`` and scalar
    primitives are returned unchanged. Anything else (e.g. a stray datetime)
    is stringified as a deterministic last resort. Reference cycles are
    replaced with ``<circular reference>`` rather than recursing forever.
    """
    if _seen is None:
        _seen = set()

    if isinstance(value, dict):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return {str(key): _json_safe(item, _seen) for key, item in value.items()}
        finally:
            _seen.discard(marker)

    if isinstance(value, (list, tuple)):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return [_json_safe(item, _seen) for item in value]
        finally:
            _seen.discard(marker)

    if isinstance(value, (set, frozenset)):
        marker = id(value)
        if marker in _seen:
            return _CIRCULAR_REF_MARKER
        _seen.add(marker)
        try:
            return [_json_safe(item, _seen) for item in sorted(value, key=repr)]
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
        return len(json.dumps(payload, separators=(",", ":"), default=str).encode("utf-8"))
    except (TypeError, ValueError):
        return None


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
