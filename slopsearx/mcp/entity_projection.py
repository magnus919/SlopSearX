"""Explicit entity relationships over snapshots; never dispatch or verify sources."""

from __future__ import annotations

import hashlib
import json
import re
from typing import Any

from slopsearx.adapter import SearchResult
from slopsearx.payload import payload_for_persistence
from slopsearx.snapshot import SearchSnapshot

ENTITY_CONTRACT = "slopsearx.entity_groups"
ENTITY_VERSION = 1


def _text(value: Any) -> str | None:
    if not isinstance(value, str) or not value or len(value) > 512:
        return None
    if any(char.isspace() or ord(char) < 32 or ord(char) == 127 for char in value):
        return None
    return value


def _identity(result: SearchResult) -> tuple[str | None, dict[str, Any], str, dict[str, Any]]:
    """Use only declared adapter fields from supported version-one payloads."""
    payload = payload_for_persistence(result.payload)
    if payload is None or type(payload.get("schema_version")) is not int or payload["schema_version"] != 1:
        return None, {}, "missing_or_unsupported_payload", {}
    data, provenance = payload.get("data"), payload.get("provenance")
    if not isinstance(data, dict) or not isinstance(provenance, dict):
        return None, {}, "invalid_payload", {}
    source = provenance.get("engine")
    if not isinstance(source, str) or source not in (result.engines or {result.engine}):
        return None, {}, "conflicting_provenance", {}
    reported, inferred = provenance.get("adapter_fields"), provenance.get("inferred_fields")
    if not isinstance(reported, list) or not isinstance(inferred, list):
        return None, {}, "invalid_provenance", {}

    def explicit(field: str) -> bool:
        return field in reported and field not in inferred

    domain_type = (payload.get("domain"), payload.get("type"))
    if domain_type == ("security", "vulnerability"):
        cve = _text(data.get("cve_id"))
        if not explicit("cve_id") or cve is None or not re.fullmatch(r"CVE-[0-9]{4}-[0-9]{4,}", cve.upper()):
            return None, {}, "invalid_or_unreported_identifier", {}
        return "cve", {"cve_id": cve.upper()}, "explicit_adapter_identifier", data
    if domain_type == ("packages", "package"):
        if source not in {"npm", "pypi"}:
            return None, {}, "unknown_ecosystem", {}
        if "ecosystem" in data and data["ecosystem"] != source:
            return None, {}, "conflicting_ecosystem", {}
        name = _text(data.get("name"))
        version = data.get("version")
        if not explicit("name") or name is None:
            return None, {}, "invalid_or_unreported_identifier", {}
        if "version" in data and (not explicit("version") or _text(version) is None):
            return None, {}, "invalid_or_unreported_version", {}
        pattern = r"[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?"
        if source == "npm":
            pattern = r"(?:@[A-Za-z0-9._-]+/)?[A-Za-z0-9][A-Za-z0-9._-]*"
        if not re.fullmatch(pattern, name):
            return None, {}, "invalid_or_unreported_identifier", {}
        if source == "pypi":
            name = re.sub(r"[-_.]+", "-", name).lower()
        return source, {"name": name, "version": version}, "explicit_adapter_identifier", data
    return None, {}, "unsupported_domain", {}


def entity_groups(snapshot: SearchSnapshot) -> list[dict[str, Any]]:
    """Group before paging, preserving original indices and unresolved singletons.

    Identity encodes namespace and release coordinates, not URL or search rank.
    Differing shared payload facts are reported without selecting a winner.
    """
    groups: list[dict[str, Any]] = []
    by_identity: dict[str, dict[str, Any]] = {}
    facts: dict[str, dict[str, str]] = {}
    conflicts: dict[str, set[str]] = {}
    for index, result in enumerate(snapshot.results):
        namespace, identifier, reason, data = _identity(result)
        entity_id = None
        if namespace is not None:
            encoded = json.dumps([ENTITY_VERSION, namespace, identifier], sort_keys=True, separators=(",", ":"))
            entity_id = "entity-v1-" + hashlib.sha256(encoded.encode()).hexdigest()
        group = by_identity.get(entity_id) if entity_id is not None else None
        if group is None:
            group = {
                "entity_id": entity_id,
                "namespace": namespace,
                "identifier": identifier or None,
                "reason": reason,
                "result_ids": [],
                "conflicting_fields": [],
            }
            groups.append(group)
            if entity_id is not None:
                by_identity[entity_id] = group
                facts[entity_id] = {}
                conflicts[entity_id] = set()
        group["result_ids"].append(f"{snapshot.snapshot_id}:{index}")
        if entity_id is not None:
            identity_fields = {"cve_id"} if namespace == "cve" else {"name", "version", "ecosystem"}
            for field, value in data.items():
                if field in identity_fields:
                    continue
                encoded_value = json.dumps(value, sort_keys=True, separators=(",", ":"))
                previous = facts[entity_id].setdefault(field, encoded_value)
                if previous != encoded_value:
                    conflicts[entity_id].add(field)
    for entity_id, group in by_identity.items():
        group["conflicting_fields"] = sorted(conflicts[entity_id])
    return groups
