"""Explicit entity relationships over snapshots; never dispatch or verify sources."""

from __future__ import annotations

import hashlib
import json
import re
import unicodedata
from typing import Any
from urllib.parse import unquote, urlparse

from slopsearx.adapter import SearchResult
from slopsearx.payload import payload_for_persistence
from slopsearx.snapshot import SearchSnapshot

ENTITY_CONTRACT = "slopsearx.entity_groups"
ENTITY_VERSION = 1
ENTITY_LATEST_VERSION = 2
ENTITY_VERSIONS = (ENTITY_VERSION, ENTITY_LATEST_VERSION)


def _text(value: Any) -> str | None:
    if not isinstance(value, str) or not value or len(value) > 512:
        return None
    if any(char.isspace() or ord(char) < 32 or ord(char) == 127 for char in value):
        return None
    return value


def _identity_v1(result: SearchResult) -> tuple[str | None, dict[str, Any], str, dict[str, Any]]:
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


def _entity_groups_v1(snapshot: SearchSnapshot) -> list[dict[str, Any]]:
    """Group before paging, preserving original indices and unresolved singletons.

    Identity encodes namespace and release coordinates, not URL or search rank.
    Differing shared payload facts are reported without selecting a winner.
    """
    groups: list[dict[str, Any]] = []
    by_identity: dict[str, dict[str, Any]] = {}
    facts: dict[str, dict[str, str]] = {}
    conflicts: dict[str, set[str]] = {}
    for index, result in enumerate(snapshot.results):
        namespace, identifier, reason, data = _identity_v1(result)
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


def _payload_context(result: SearchResult) -> tuple[dict[str, Any], set[str], str] | None:
    payload = payload_for_persistence(result.payload)
    if payload is None or type(payload.get("schema_version")) is not int or payload["schema_version"] != 1:
        return None
    data, provenance = payload.get("data"), payload.get("provenance")
    if not isinstance(data, dict) or not isinstance(provenance, dict):
        return None
    source = provenance.get("engine")
    if not isinstance(source, str) or source not in (result.engines or {result.engine}):
        return None
    reported, inferred = provenance.get("adapter_fields"), provenance.get("inferred_fields")
    if not isinstance(reported, list) or not isinstance(inferred, list):
        return None
    explicit = {field for field in reported if isinstance(field, str) and field not in inferred}
    return data, explicit, source


def _doi(value: Any) -> str | None:
    raw = _text(value)
    if raw is None:
        return None
    lowered = raw.casefold()
    for prefix in ("https://doi.org/", "http://doi.org/", "doi:"):
        if lowered.startswith(prefix):
            raw = unquote(raw[len(prefix) :])
            break
    normalized = unicodedata.normalize("NFC", raw).casefold()
    if len(normalized) > 255 or not re.fullmatch(r"10\.[0-9]{4,9}/\S+", normalized):
        return None
    if any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        return None
    return normalized


def _numeric_identifier(value: Any, *, prefix: str = "", maximum_digits: int = 12) -> str | None:
    raw = _text(value)
    if raw is None:
        return None
    if prefix and raw.upper().startswith(prefix):
        raw = raw[len(prefix) :]
    if not re.fullmatch(rf"[0-9]{{1,{maximum_digits}}}", raw) or int(raw) == 0:
        return None
    return f"{prefix}{raw}" if prefix else raw


def _openalex(value: Any) -> str | None:
    raw = _text(value)
    if raw is None:
        return None
    parsed = urlparse(raw)
    if parsed.scheme or parsed.netloc:
        if parsed.scheme.lower() != "https" or parsed.netloc.lower() != "openalex.org":
            return None
        if parsed.query or parsed.fragment:
            return None
        raw = parsed.path.strip("/")
    raw = raw.upper()
    return raw if re.fullmatch(r"W[1-9][0-9]{3,14}", raw) else None


def _repository(value: Any) -> str | None:
    raw = _text(value)
    if raw is None:
        return None
    parsed = urlparse(raw)
    if parsed.scheme or parsed.netloc:
        if parsed.scheme.lower() != "https" or parsed.netloc.lower() not in {"github.com", "www.github.com"}:
            return None
        if parsed.query or parsed.fragment:
            return None
        raw = parsed.path.strip("/")
    if raw.endswith(".git"):
        raw = raw[:-4]
    parts = raw.split("/")
    if len(parts) != 2:
        return None
    owner, repository = parts
    if not re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?", owner):
        return None
    if not re.fullmatch(r"[A-Za-z0-9._-]{1,100}", repository):
        return None
    return f"{owner.casefold()}/{repository.casefold()}"


def _advisory(value: Any) -> tuple[str, str] | None:
    raw = _text(value)
    if raw is None:
        return None
    normalized = raw.upper()
    if re.fullmatch(r"GHSA-[23456789CFGHJMPQRVWX]{4}-[23456789CFGHJMPQRVWX]{4}-[23456789CFGHJMPQRVWX]{4}", normalized):
        return "ghsa", normalized
    if re.fullmatch(r"(?:PYSEC|RUSTSEC|GO)-[0-9]{4}-[0-9]{1,8}", normalized):
        return "osv_advisory", normalized
    return None


def _identity_id(namespace: str, identifier: dict[str, Any], version: int) -> str:
    encoded = json.dumps([version, namespace, identifier], sort_keys=True, separators=(",", ":"))
    return f"entity-v{version}-" + hashlib.sha256(encoded.encode()).hexdigest()


def _identities_v2(result: SearchResult) -> tuple[list[tuple[str, dict[str, Any]]], str, dict[str, Any]]:
    context = _payload_context(result)
    if context is None:
        return [], "missing_or_unsupported_payload", {}
    data, explicit, source = context
    identities: list[tuple[str, dict[str, Any]]] = []

    namespace, identifier, reason, _legacy_data = _identity_v1(result)
    if namespace is not None:
        identities.append((namespace, identifier))

    def add(field: str, namespace_name: str, normalize: Any, canonical_field: str | None = None) -> None:
        if field not in data:
            return
        normalized = normalize(data.get(field)) if field in explicit else None
        if normalized is not None:
            identities.append((namespace_name, {canonical_field or field: normalized}))

    domain_type = (result.payload or {}).get("domain"), (result.payload or {}).get("type")
    if domain_type == ("science", "publication"):
        add("doi", "doi", _doi)
        if source == "pubmed" and "publication_id" in explicit and "pmid" not in data:
            normalized = _numeric_identifier(data.get("publication_id"), maximum_digits=9)
            if normalized is not None:
                identities.append(("pmid", {"pmid": normalized}))
        add("pmid", "pmid", lambda value: _numeric_identifier(value, maximum_digits=9))
        add("pmcid", "pmcid", lambda value: _numeric_identifier(value, prefix="PMC", maximum_digits=9))
        add("openalex_id", "openalex", _openalex)
    if domain_type == ("code", "repository"):
        add("repository", "github_repository", _repository)
        add("repository_url", "github_repository", _repository, "repository")
    if domain_type == ("packages", "package"):
        add("repository_url", "github_repository", _repository, "repository")
        add("repository", "github_repository", _repository)
    if domain_type in {("security", "vulnerability"), ("security", "advisory")}:
        for field in ("ghsa_id", "advisory_id"):
            if field in explicit:
                advisory = _advisory(data.get(field))
                if advisory is not None:
                    identities.append((advisory[0], {"advisory_id": advisory[1]}))

    unique: list[tuple[str, dict[str, Any]]] = []
    seen: set[str] = set()
    for item in identities:
        key = json.dumps(item, sort_keys=True, separators=(",", ":"))
        if key not in seen:
            seen.add(key)
            unique.append(item)
    values_by_namespace: dict[str, set[str]] = {}
    for namespace_name, identifier_value in unique:
        values_by_namespace.setdefault(namespace_name, set()).add(
            json.dumps(identifier_value, sort_keys=True, separators=(",", ":"))
        )
    conflicting_namespaces = {
        namespace_name for namespace_name, values in values_by_namespace.items() if len(values) > 1
    }
    unique = [item for item in unique if item[0] not in conflicting_namespaces]
    if unique:
        return unique, "explicit_adapter_identifier", data
    if conflicting_namespaces:
        return [], "conflicting_source_identifiers", data
    if any(
        field in data
        for field in ("doi", "pmid", "pmcid", "openalex_id", "repository", "repository_url", "ghsa_id", "advisory_id")
    ):
        return [], "invalid_inferred_or_conflicting_identifier", data
    return [], reason if reason != "explicit_adapter_identifier" else "unsupported_identifier", data


def entity_projection(
    snapshot: SearchSnapshot, version: int = ENTITY_VERSION
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Return versioned entity groups and explicit cross-namespace edges."""
    if version == ENTITY_VERSION:
        return _entity_groups_v1(snapshot), []
    if version != ENTITY_LATEST_VERSION:
        raise ValueError(f"entity version must be one of {ENTITY_VERSIONS}")

    groups: list[dict[str, Any]] = []
    by_id: dict[str, dict[str, Any]] = {}
    facts: dict[str, dict[str, str]] = {}
    conflicts: dict[str, set[str]] = {}
    relationships: dict[tuple[str, str, str], dict[str, Any]] = {}
    identity_fields = {
        "cve_id",
        "name",
        "version",
        "ecosystem",
        "doi",
        "pmid",
        "pmcid",
        "openalex_id",
        "repository",
        "repository_url",
        "ghsa_id",
        "advisory_id",
    }
    for index, result in enumerate(snapshot.results):
        identities, reason, data = _identities_v2(result)
        result_id = f"{snapshot.snapshot_id}:{index}"
        if not identities:
            groups.append(
                {
                    "entity_id": None,
                    "namespace": None,
                    "identifier": None,
                    "reason": reason,
                    "result_ids": [result_id],
                    "conflicting_fields": [],
                }
            )
            continue
        ids: list[tuple[str, str]] = []
        for namespace, identifier in identities:
            entity_id = _identity_id(namespace, identifier, version)
            ids.append((namespace, entity_id))
            group = by_id.get(entity_id)
            if group is None:
                group = {
                    "entity_id": entity_id,
                    "namespace": namespace,
                    "identifier": identifier,
                    "reason": "explicit_adapter_identifier",
                    "result_ids": [],
                    "conflicting_fields": [],
                }
                groups.append(group)
                by_id[entity_id] = group
                facts[entity_id] = {}
                conflicts[entity_id] = set()
            group["result_ids"].append(result_id)
            for field, value in data.items():
                if field in identity_fields:
                    continue
                encoded = json.dumps(value, sort_keys=True, separators=(",", ":"))
                previous = facts[entity_id].setdefault(field, encoded)
                if previous != encoded:
                    conflicts[entity_id].add(field)
        package_ids = [entity_id for namespace, entity_id in ids if namespace in {"npm", "pypi"}]
        repo_ids = [entity_id for namespace, entity_id in ids if namespace == "github_repository"]
        cve_ids = [entity_id for namespace, entity_id in ids if namespace == "cve"]
        advisory_ids = [entity_id for namespace, entity_id in ids if namespace in {"ghsa", "osv_advisory"}]
        scholarly_ids = [entity_id for namespace, entity_id in ids if namespace in {"doi", "pmid", "pmcid", "openalex"}]
        edge_sets = [
            (package_ids, repo_ids, "candidate_repository"),
            (cve_ids, advisory_ids, "source_reported_advisory"),
        ]
        if len(scholarly_ids) > 1:
            edge_sets.append((scholarly_ids[:1], scholarly_ids[1:], "source_reported_alias"))
        for from_ids, to_ids, relation in edge_sets:
            for from_id in from_ids:
                for to_id in to_ids:
                    key = from_id, relation, to_id
                    edge = relationships.setdefault(
                        key,
                        {"from_entity_id": from_id, "relation": relation, "to_entity_id": to_id, "result_ids": []},
                    )
                    if result_id not in edge["result_ids"]:
                        edge["result_ids"].append(result_id)
    for entity_id, group in by_id.items():
        group["conflicting_fields"] = sorted(conflicts[entity_id])
    return groups, [relationships[key] for key in sorted(relationships)]


def entity_groups(snapshot: SearchSnapshot, version: int = ENTITY_VERSION) -> list[dict[str, Any]]:
    """Compatibility wrapper returning only groups for the selected version."""
    return entity_projection(snapshot, version)[0]
