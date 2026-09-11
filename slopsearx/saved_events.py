"""Versioned, privacy-bounded saved-search outbox events."""

from __future__ import annotations

import json
import re
import secrets
from typing import Any

from slopsearx.artifacts import artifact_ref, composite_artifact_id
from slopsearx.saved_models import SavedDefinition

EVENT_CONTRACT = "slopsearx.saved_search_event"
EVENT_CONTRACT_VERSION = 1
EVENT_TYPES = frozenset(
    {
        "report_created",
        "run_incomparable",
        "run_failed",
        "definition_paused",
        "definition_expired",
    }
)
REASON_CODES = frozenset(
    {
        "none",
        "dispatch_failed",
        "observation_incomparable",
        "operator_paused",
        "lifetime_elapsed",
        "policy_redacted",
    }
)
CURSOR_RE = re.compile(r"^(0|[1-9][0-9]*)-(0|[1-9][0-9]*)$")
CONSUMER_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
MAX_EVENT_BYTES = 64 * 1024


def generate_event_id() -> str:
    """Return an opaque identifier suitable for at-least-once deduplication."""
    return f"event-{secrets.token_urlsafe(18)}"


def validate_cursor(cursor: str) -> str:
    """Validate and return one canonical Valkey stream cursor."""
    if not isinstance(cursor, str) or len(cursor) > 64 or CURSOR_RE.fullmatch(cursor) is None:
        raise ValueError("cursor must use the '<milliseconds>-<sequence>' form")
    return cursor


def validate_consumer_id(consumer_id: str) -> str:
    """Bound consumer identity so it is safe in tenant-scoped storage keys."""
    if not isinstance(consumer_id, str) or CONSUMER_RE.fullmatch(consumer_id) is None:
        raise ValueError("consumer_id must be 1-128 safe identifier characters")
    return consumer_id


def cursor_tuple(cursor: str) -> tuple[int, int]:
    milliseconds, sequence = validate_cursor(cursor).split("-", 1)
    return int(milliseconds), int(sequence)


def event_for_report(definition: SavedDefinition, report: dict[str, Any]) -> dict[str, Any]:
    """Summarize a committed report without retaining result bodies or secrets."""
    status = str(report.get("status", "incomparable"))
    reasons = [str(item) for item in report.get("incomparable_reasons", [])]
    operational_error = report.get("observation", {}).get("operational_error")
    if operational_error or "dispatch_failed" in reasons:
        event_type = "run_failed"
        reason_code = "dispatch_failed"
    elif status == "incomparable":
        event_type = "run_incomparable"
        reason_code = "observation_incomparable"
    else:
        event_type = "report_created"
        reason_code = "none"
    changes = {"added": 0, "source_field_changed": 0, "not_observed_in_latest_run": 0}
    for item in report.get("events", []):
        kind = str(item.get("kind", ""))
        if kind in changes:
            changes[kind] += 1
    observation = report.get("observation", {})
    coverage = observation.get("coverage", {})
    coverage_state = (
        "complete"
        if coverage and all(value == "ok" for value in coverage.values())
        else ("partial" if coverage else "unknown")
    )
    event = {
        "contract": EVENT_CONTRACT,
        "contract_version": EVENT_CONTRACT_VERSION,
        "event_id": generate_event_id(),
        "event_type": event_type,
        "occurred_at": float(report["finished_at"]),
        "saved_search_id": definition.search_id,
        "run_id": str(report["run_id"]),
        "saved_search": artifact_ref("saved_search", definition.search_id),
        "run": artifact_ref(
            "saved_report",
            composite_artifact_id(definition.search_id, str(report["run_id"])),
        ),
        "summary": {
            "status": status,
            "added_count": changes["added"],
            "changed_count": changes["source_field_changed"],
            "not_observed_count": changes["not_observed_in_latest_run"],
            "discovered_count": int(observation.get("discovered_count", 0)),
            "missed_slots": int(report.get("missed_slots", 0)),
        },
        "coverage_state": coverage_state,
        "reason_code": reason_code,
        "_policy_engines": sorted(definition.engines),
    }
    if len(json.dumps(event, separators=(",", ":")).encode()) > MAX_EVENT_BYTES:
        raise ValueError("saved-search event exceeds the storage bound")
    return event


def definition_event(definition: SavedDefinition, event_type: str, occurred_at: float) -> dict[str, Any]:
    """Build a state-transition event with the same public envelope."""
    if event_type not in {"definition_paused", "definition_expired"}:
        raise ValueError("unsupported definition event type")
    return {
        "contract": EVENT_CONTRACT,
        "contract_version": EVENT_CONTRACT_VERSION,
        "event_id": generate_event_id(),
        "event_type": event_type,
        "occurred_at": occurred_at,
        "saved_search_id": definition.search_id,
        "run_id": None,
        "saved_search": artifact_ref("saved_search", definition.search_id),
        "run": None,
        "summary": None,
        "coverage_state": "unknown",
        "reason_code": "operator_paused" if event_type == "definition_paused" else "lifetime_elapsed",
        "_policy_engines": sorted(definition.engines),
    }


def public_event(event: dict[str, Any], cursor: str, *, redacted: bool = False) -> dict[str, Any]:
    """Serialize an event, dropping internal policy input and protected detail."""
    output = {key: value for key, value in event.items() if not key.startswith("_")}
    output["cursor"] = cursor
    output["policy_state"] = "redacted" if redacted else "current"
    if redacted:
        output["summary"] = None
        output["coverage_state"] = "redacted"
        output["reason_code"] = "policy_redacted"
    return output
