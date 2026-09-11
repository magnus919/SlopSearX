"""Pure identity and dossier helpers for the dependency workflow."""

from __future__ import annotations

import hashlib
import json
import re
from typing import Any
from urllib.parse import urlsplit

ECOSYSTEM_ENGINES = {"pypi": "pypi", "npm": "npm"}
ADVISORY_ENGINE = "nvd"
REPOSITORY_ENGINE = "github"
SECTION_STATES = (
    "available",
    "empty",
    "partial",
    "failed",
    "unavailable",
    "unresolved",
    "ambiguous",
    "expired",
)

_PYPI_NAME = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._-]{0,198}[A-Za-z0-9])?$")
_NPM_SEGMENT = re.compile(r"^[a-z0-9][a-z0-9._~-]{0,213}$")
_GITHUB_SEGMENT = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9_.-]{0,98}[A-Za-z0-9])?$")


def normalize_package(ecosystem: str, package: str) -> str:
    """Validate and normalize an ecosystem-qualified package name."""
    value = package.strip()
    if ecosystem == "pypi":
        if not _PYPI_NAME.fullmatch(value):
            raise ValueError("invalid PyPI package name")
        return re.sub(r"[-_.]+", "-", value).lower()
    if ecosystem == "npm":
        if value.startswith("@"):
            parts = value[1:].split("/")
            if len(parts) != 2 or not all(_NPM_SEGMENT.fullmatch(part) for part in parts):
                raise ValueError("invalid scoped npm package name")
            return "@" + "/".join(parts)
        if not _NPM_SEGMENT.fullmatch(value):
            raise ValueError("invalid npm package name")
        return value
    raise ValueError("unsupported ecosystem")


def normalize_repository(repository: str | None) -> str | None:
    """Return a canonical GitHub owner/repository identity without fetching."""
    if repository is None:
        return None
    value = repository.strip()
    if "://" in value:
        parsed = urlsplit(value)
        if (
            parsed.scheme != "https"
            or parsed.hostname not in {"github.com", "www.github.com"}
            or parsed.username
            or parsed.password
            or parsed.port
            or parsed.query
            or parsed.fragment
        ):
            raise ValueError("repository must be a public canonical GitHub URL")
        value = parsed.path.strip("/")
    parts = value.removesuffix(".git").split("/")
    if len(parts) != 2 or not all(_GITHUB_SEGMENT.fullmatch(part) for part in parts):
        raise ValueError("repository must be owner/repository")
    return f"{parts[0]}/{parts[1]}"


def workflow_identity(
    ecosystem: str,
    package: str,
    version: str | None,
    repository: str | None,
    deadline: str | None,
) -> tuple[dict[str, Any], str]:
    identity = {
        "ecosystem": ecosystem,
        "package": normalize_package(ecosystem, package),
        "requested_version": version.strip() if isinstance(version, str) and version.strip() else None,
        "repository": normalize_repository(repository),
        "requested_deadline": deadline,
    }
    digest = hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return identity, digest


def package_payload(result: Any, ecosystem: str) -> dict[str, Any] | None:
    payload = result.payload if isinstance(result.payload, dict) else None
    if not payload or payload.get("domain") != "packages" or payload.get("type") != "package":
        return None
    if payload.get("schema_version") != 1 or getattr(result, "engine", None) != ECOSYSTEM_ENGINES[ecosystem]:
        return None
    provenance = payload.get("provenance")
    if not isinstance(provenance, dict) or provenance.get("engine") != ECOSYSTEM_ENGINES[ecosystem]:
        return None
    adapter_fields = provenance.get("adapter_fields")
    inferred_fields = provenance.get("inferred_fields")
    if not isinstance(adapter_fields, list) or "name" not in adapter_fields:
        return None
    if isinstance(inferred_fields, list) and "name" in inferred_fields:
        return None
    data = payload.get("data")
    return data if isinstance(data, dict) else None


def github_root_candidate(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = urlsplit(value.strip())
        if parsed.scheme != "https" or parsed.hostname not in {"github.com", "www.github.com"}:
            return None
        return normalize_repository(value)
    except ValueError:
        return None


def resolve_package_results(ecosystem: str, requested_name: str, results: list[Any]) -> dict[str, Any]:
    """Resolve only exact attributed registry payload identities."""
    exact: list[dict[str, Any]] = []
    candidates: list[dict[str, Any]] = []
    repositories: set[str] = set()
    for index, result in enumerate(results):
        data = package_payload(result, ecosystem)
        if data is None:
            continue
        observed = data.get("name")
        if not isinstance(observed, str):
            continue
        try:
            normalized = normalize_package(ecosystem, str(observed))
        except ValueError:
            normalized = ""
        observed_version = data.get("version")
        version = observed_version if isinstance(observed_version, str) else None
        item = {"index": index, "name": observed, "normalized_name": normalized, "version": version}
        candidates.append(item)
        if normalized == requested_name:
            exact.append(item)
            for field in ("repository_url", "homepage"):
                candidate = github_root_candidate(data.get(field))
                if candidate:
                    repositories.add(candidate)
    distinct = {(item["normalized_name"], item.get("version")) for item in exact}
    if not exact:
        status = "unresolved"
    elif len(distinct) > 1:
        status = "ambiguous"
    else:
        status = "resolved"
    return {
        "status": status,
        "basis": "attributed_registry_payload" if exact else None,
        "matches": exact,
        "candidates": candidates,
        "repository_candidates": sorted(repositories),
    }
