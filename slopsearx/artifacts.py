"""Versioned identities and relationships for bounded SlopSearX artifacts."""

from __future__ import annotations

import hashlib
import json
from typing import Any

ARTIFACT_CONTRACT = "slopsearx.artifact_ref"
ARTIFACT_VERSION = 1
LINEAGE_CONTRACT = "slopsearx.artifact_lineage"
LINEAGE_VERSION = 1

ARTIFACT_KINDS = frozenset(
    {
        "snapshot",
        "result",
        "entity_group",
        "research_job",
        "research_attempt",
        "staged_search",
        "saved_search",
        "saved_report",
        "retrieval_receipt",
        "research_manifest",
        "dependency_dossier",
    }
)
LINEAGE_RELATIONS = frozenset({"contains", "derived_from", "selected", "observed", "retrieval_of"})


def artifact_ref(kind: str, artifact_id: str) -> dict[str, Any]:
    """Return a validated, JSON-safe artifact reference."""
    if kind not in ARTIFACT_KINDS:
        raise ValueError("unsupported artifact kind")
    if not isinstance(artifact_id, str) or not artifact_id or len(artifact_id) > 1024:
        raise ValueError("artifact id must contain 1 to 1024 characters")
    if any(ord(char) < 32 or ord(char) == 127 for char in artifact_id):
        raise ValueError("artifact id contains control characters")
    return {"contract": ARTIFACT_CONTRACT, "version": ARTIFACT_VERSION, "kind": kind, "id": artifact_id}


def parse_artifact_ref(value: Any) -> dict[str, Any]:
    """Validate an untrusted artifact reference without accepting extra fields."""
    if not isinstance(value, dict) or set(value) != {"contract", "version", "kind", "id"}:
        raise ValueError("artifact must contain exactly contract, version, kind, and id")
    if value.get("contract") != ARTIFACT_CONTRACT or value.get("version") != ARTIFACT_VERSION:
        raise ValueError("unsupported artifact reference contract or version")
    kind, artifact_id = value.get("kind"), value.get("id")
    if not isinstance(kind, str) or not isinstance(artifact_id, str):
        raise ValueError("artifact kind and id must be strings")
    return artifact_ref(kind, artifact_id)


def composite_artifact_id(*parts: str) -> str:
    """Encode already-opaque component IDs into one unambiguous artifact ID."""
    if not parts or any(not isinstance(part, str) or not part for part in parts):
        raise ValueError("composite artifact parts must be non-empty strings")
    encoded = json.dumps(parts, ensure_ascii=True, separators=(",", ":")).encode()
    return "composite-v1-" + encoded.hex()


def parse_composite_artifact_id(value: str, count: int) -> tuple[str, ...]:
    """Decode an ID created by :func:`composite_artifact_id`."""
    prefix = "composite-v1-"
    if not isinstance(value, str) or not value.startswith(prefix):
        raise ValueError("invalid composite artifact id")
    try:
        decoded = json.loads(bytes.fromhex(value[len(prefix) :]).decode())
    except (ValueError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("invalid composite artifact id") from exc
    if not isinstance(decoded, list) or len(decoded) != count:
        raise ValueError("invalid composite artifact id")
    if any(not isinstance(part, str) or not part for part in decoded):
        raise ValueError("invalid composite artifact id")
    return tuple(decoded)


def lineage_edge(source: dict[str, Any], relation: str, target: dict[str, Any]) -> dict[str, Any]:
    """Return one directed relationship using the closed relation vocabulary."""
    if relation not in LINEAGE_RELATIONS:
        raise ValueError("unsupported lineage relation")
    return {"from": parse_artifact_ref(source), "relation": relation, "to": parse_artifact_ref(target)}


def manifest_artifact_id(payload: dict[str, Any]) -> str:
    """Return a deterministic identity for one exact generated manifest."""
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    return "manifest-v1-" + hashlib.sha256(encoded).hexdigest()
