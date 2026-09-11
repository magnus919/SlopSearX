"""Bounded saved-search definitions and source-observation comparisons.

A comparison describes a search window, never exhaustive upstream inventory.
No persistence, scheduling, or authorization is performed in this module.
"""

from __future__ import annotations

import hashlib
import json
import secrets
from dataclasses import dataclass
from typing import Any

from slopsearx.merger import _normalise_url
from slopsearx.service import SearchResponse

IDENTITY_VERSION = "url-source-window-v1"
MAX_FIELD_BYTES = 8192
MAX_REPORT_BYTES = 4 * 1024 * 1024


def stable_id(*parts: Any) -> str:
    """Domain callers supply the prefix; hash encoding is deterministic."""
    return hashlib.sha256(json.dumps(parts, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


@dataclass(frozen=True)
class SavedLimits:
    definitions: int = 20
    engines: int = 5
    rows: int = 100
    reports: int = 20
    retention_seconds: int = 7 * 86400
    minimum_interval: int = 60
    maximum_interval: int = 86400
    concurrency: int = 2
    dispatch_timeout: int = 30


@dataclass
class SavedDefinition:
    search_id: str
    tenant: str
    query: str
    engines: list[str]
    interval_seconds: int
    retention_seconds: int
    max_results: int
    max_reports: int
    created_at: float
    expires_at: float
    next_due: float
    revision: int = 1
    paused: bool = False
    policy_fingerprint: str = ""
    last_slot: int = -1
    latest_run_id: str | None = None
    baseline: dict[str, Any] | None = None
    lease_token: str | None = None
    lease_expires_at: float = 0
    deleted: bool = False

    def window(self) -> dict[str, Any]:
        return {
            "query": self.query,
            "engines": sorted(self.engines),
            "page": 1,
            "max_results": self.max_results,
            "revision": self.revision,
            "identity_version": IDENTITY_VERSION,
            "policy_fingerprint": self.policy_fingerprint,
        }


def generate_search_id() -> str:
    """Return an opaque, unguessable saved-search handle."""
    return f"saved-{secrets.token_urlsafe(18)}"


def definition_to_payload(definition: SavedDefinition) -> dict[str, Any]:
    """Serialize a definition without lossy string fallbacks."""
    return {
        "search_id": definition.search_id,
        "tenant": definition.tenant,
        "query": definition.query,
        "engines": list(definition.engines),
        "interval_seconds": definition.interval_seconds,
        "retention_seconds": definition.retention_seconds,
        "max_results": definition.max_results,
        "max_reports": definition.max_reports,
        "created_at": definition.created_at,
        "expires_at": definition.expires_at,
        "next_due": definition.next_due,
        "revision": definition.revision,
        "paused": definition.paused,
        "policy_fingerprint": definition.policy_fingerprint,
        "last_slot": definition.last_slot,
        "latest_run_id": definition.latest_run_id,
        "baseline": definition.baseline,
        "lease_token": definition.lease_token,
        "lease_expires_at": definition.lease_expires_at,
        "deleted": definition.deleted,
    }


def definition_from_payload(payload: dict[str, Any]) -> SavedDefinition:
    """Rehydrate a typed definition from shared state."""
    return SavedDefinition(
        search_id=str(payload["search_id"]),
        tenant=str(payload["tenant"]),
        query=str(payload["query"]),
        engines=[str(item) for item in payload.get("engines", [])],
        interval_seconds=int(payload["interval_seconds"]),
        retention_seconds=int(payload["retention_seconds"]),
        max_results=int(payload["max_results"]),
        max_reports=int(payload["max_reports"]),
        created_at=float(payload["created_at"]),
        expires_at=float(payload["expires_at"]),
        next_due=float(payload["next_due"]),
        revision=int(payload.get("revision", 1)),
        paused=bool(payload.get("paused", False)),
        policy_fingerprint=str(payload.get("policy_fingerprint", "")),
        last_slot=int(payload.get("last_slot", -1)),
        latest_run_id=str(payload["latest_run_id"]) if payload.get("latest_run_id") else None,
        baseline=payload.get("baseline") if isinstance(payload.get("baseline"), dict) else None,
        lease_token=str(payload["lease_token"]) if payload.get("lease_token") else None,
        lease_expires_at=float(payload.get("lease_expires_at", 0)),
        deleted=bool(payload.get("deleted", False)),
    )


def observe(definition: SavedDefinition, response: SearchResponse, observed_at: float) -> dict[str, Any]:
    """Capture source facts separately from ranking and transient metadata."""
    reasons: list[str] = []
    coverage = {outcome.engine: outcome.status for outcome in response.engine_outcomes}
    if response.cached:
        reasons.append("cached_observation")
    if response.partial or response.deadline_exceeded or response.all_unresponsive:
        reasons.append("incomplete_response")
    if set(response.scope.selected_engines) != set(definition.engines):
        reasons.append("scope_changed")
    if any(coverage.get(engine) != "ok" for engine in definition.engines):
        reasons.append("incomplete_engine_coverage")
    if len(response.results) > definition.max_results:
        reasons.append("observation_truncated")
    records: dict[str, dict[str, Any]] = {}
    for result in response.results[: definition.max_results]:
        fields = {"title": result.title, "content": result.content, "published_date": result.published_date}
        if not result.url or any(
            isinstance(value, str) and len(value.encode()) > MAX_FIELD_BYTES for value in [result.url, *fields.values()]
        ):
            reasons.append("unrepresentable_source_record")
            continue
        identity = stable_id(IDENTITY_VERSION, _normalise_url(result.url))
        record = {
            "url": result.url,
            "engine": result.engine,
            "engines": sorted(result.engines or {result.engine}),
            "fields": fields,
        }
        if identity in records and records[identity] != record:
            reasons.append("ambiguous_url_identity")
        records[identity] = record
    return {
        "window": definition.window(),
        "observed_at": observed_at,
        "query_id": response.query_id,
        "cached": response.cached,
        "coverage": coverage,
        "discovered_count": len(response.results),
        "records": records,
        "incomparable_reasons": sorted(set(reasons)),
    }


def compare_observations(
    definition: SavedDefinition,
    observation: dict[str, Any],
    *,
    run_id: str,
    scheduled_at: float,
    started_at: float,
    finished_at: float,
    missed_slots: int,
) -> tuple[dict[str, Any], dict[str, Any] | None]:
    """Return a report and optional replacement baseline; failures retain it."""
    baseline = definition.baseline
    reasons = list(observation["incomparable_reasons"])
    baseline_reason = "first_observation"
    if baseline is not None and baseline["expires_at"] <= finished_at:
        baseline = None
        baseline_reason = "baseline_expired"
    if baseline is not None and baseline["window"] != observation["window"]:
        baseline = None
        baseline_reason = "comparison_window_changed"
    current = observation["records"]
    prior = baseline["records"] if baseline else {}
    if baseline:
        for identity in current.keys() & prior.keys():
            if any(current[identity][key] != prior[identity][key] for key in ("engine", "engines")):
                reasons.append("source_provenance_changed")
                break
    events = []
    if not reasons and baseline is not None:
        for identity in sorted(current.keys() | prior.keys()):
            before, after = prior.get(identity), current.get(identity)
            if before is None:
                kind = "added"
            elif after is None:
                kind = "not_observed_in_latest_run"
            elif before["fields"] != after["fields"]:
                kind = "source_field_changed"
            else:
                continue
            events.append(
                {
                    "event_id": stable_id(run_id, identity, kind),
                    "kind": kind,
                    "identity": identity,
                    "before": before,
                    "after": after,
                }
            )
    report = {
        "run_id": run_id,
        "search_id": definition.search_id,
        "revision": definition.revision,
        "scheduled_at": scheduled_at,
        "started_at": started_at,
        "finished_at": finished_at,
        "missed_slots": missed_slots,
        "status": "incomparable" if reasons else ("compared" if baseline else "baseline_initialized"),
        "incomparable_reasons": sorted(set(reasons)),
        "baseline_reason": baseline_reason if baseline is None else None,
        "baseline_observed_at": baseline["observed_at"] if baseline else None,
        "observation": observation,
        "events": events,
        "expires_at": min(definition.expires_at, finished_at + definition.retention_seconds),
    }
    if len(json.dumps(report).encode()) > MAX_REPORT_BYTES:
        report["status"] = "incomparable"
        report["incomparable_reasons"] = sorted(set(reasons + ["report_size_exceeded"]))
        report["events"] = []
        report["observation"] = {**observation, "records": {}}
        return report, None
    replacement = {**observation, "expires_at": report["expires_at"]} if not reasons else None
    return report, replacement
