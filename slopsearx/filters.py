"""Shared search-filter enforcement model (issue 187).

One vocabulary and one resolution path for every result-affecting filter
(``language``, ``time_range``, ``safesearch``, ``pagination``) across all
search surfaces: generic, targeted, specialist, and research.

Each filter resolves to a single machine-readable entry::

    {"requested": <value>, "status": <status>, "reason": <str>, "enforced_by": [<layer>:<engine>, ...]}

where ``status`` is exactly one of the closed :data:`ENFORCEMENT_STATUSES`
vocabulary and ``enforced_by`` carries layer-qualified tokens (``upstream:<name>``
or ``local:<name>``) that identify both the enforcing engines and the actual
enforcement layer.

Per adapter, a filter is classified into exactly one enforcement layer:

* ``upstream`` — the adapter passes the filter to the upstream source and the
  source applies it;
* ``local`` — the service locally post-filters the adapter's results using only
  result fields that are semantically valid for the filter (e.g.
  ``published_date`` for ``time_range``);
* ``None`` (unsupported) — neither layer applies.

A strict/mandatory request the selected scope cannot satisfy is rejected
fail-closed (``status == "rejected"``) **before** any engine dispatch.

This module is pure (no I/O, no Valkey) and has no dependency on the service
or MCP layers, so the same resolver is reused by the MCP tools and the
research runner.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any, Mapping, Sequence

ENFORCEMENT_STATUSES: tuple[str, ...] = ("enforced", "partially_enforced", "unsupported", "rejected")
ENFORCEMENT_LAYERS: tuple[str, ...] = ("upstream", "local")

# Age window (inclusive, in days) for the SearXNG time-range vocabulary.
_TIME_RANGE_DAYS: dict[str, int] = {
    "day": 1,
    "week": 7,
    "month": 30,
    "year": 365,
}


def enforcement_entry(
    requested: Any,
    status: str,
    reason: str,
    enforced_by: Sequence[str] | None = None,
) -> dict[str, Any]:
    """Build one enforcement entry in the pinned schema.

    Shape is exactly ``{requested, status, reason, enforced_by}``.
    ``enforced_by`` is canonicalized to a sorted, duplicate-free list.
    """
    return {
        "requested": requested,
        "status": status,
        "reason": reason,
        "enforced_by": sorted(set(enforced_by)) if enforced_by else [],
    }


def engine_filter_layer(adapter: Any, filter_name: str) -> str | None:
    """Classify one adapter's enforcement layer for a filter.

    Reads the audited ``enforced_filters`` declaration (a mapping of filter
    name → ``"upstream"`` | ``"local"``). Returns ``None`` when the adapter
    does not declare enforcement for ``filter_name``.
    """
    declared = getattr(adapter, "enforced_filters", None)
    if isinstance(declared, dict):
        layer = declared.get(filter_name)
        if isinstance(layer, str) and layer in ENFORCEMENT_LAYERS:
            return layer
    return None


def resolve_filter_enforcement(
    selected_engines: Sequence[str],
    filter_name: str,
    requested: Any,
    adapters: Mapping[str, Any],
) -> dict[str, Any]:
    """Resolve one filter's enforcement across the dispatched engine scope.

    Aggregate semantics:

    * every selected engine enforces (upstream or local) → ``enforced``;
    * a non-empty subset enforces → ``partially_enforced``;
    * none enforce → ``unsupported``.

    ``enforced_by`` carries ``"<layer>:<engine>"`` tokens so the report names
    the enforcing engines *and* the actual enforcement layer.
    """
    by_layer: dict[str, list[str]] = {"upstream": [], "local": []}
    for name in selected_engines:
        layer = engine_filter_layer(adapters.get(name), filter_name)
        if layer in by_layer:
            by_layer[layer].append(name)

    supporting = sorted(by_layer["upstream"] + by_layer["local"])
    enforced_by = [f"upstream:{name}" for name in sorted(by_layer["upstream"])] + [
        f"local:{name}" for name in sorted(by_layer["local"])
    ]

    if supporting and len(supporting) == len(selected_engines) and selected_engines:
        status = "enforced"
    elif supporting:
        status = "partially_enforced"
    else:
        status = "unsupported"

    return enforcement_entry(requested, status, _reason(filter_name, status, by_layer), enforced_by)


def _reason(filter_name: str, status: str, by_layer: dict[str, list[str]]) -> str:
    upstream = by_layer["upstream"]
    local = by_layer["local"]
    if status == "enforced":
        if upstream and not local:
            return f"{filter_name} is enforced upstream by every selected adapter"
        if local and not upstream:
            return f"{filter_name} is enforced locally by the service for every selected adapter"
        return f"{filter_name} is enforced across every selected adapter (upstream and local)"
    if status == "partially_enforced":
        parts: list[str] = []
        if upstream:
            parts.append("upstream by " + ", ".join(upstream))
        if local:
            parts.append("locally by " + ", ".join(local))
        return f"{filter_name} is enforced only for a subset of selected adapters ({'; '.join(parts)})"
    return f"no selected adapter enforces the {filter_name} filter"


# ---------------------------------------------------------------------------
# Local time-range post-filtering
# ---------------------------------------------------------------------------


def time_range_window(
    time_range: str,
    now: _dt.date | _dt.datetime | None = None,
) -> tuple[_dt.date, _dt.date] | None:
    """Return the inclusive ``(start, end)`` date window for a time range.

    Returns ``None`` for an unrecognized time-range value so callers never
    fabricate a window for a vocabulary they do not understand.
    """
    days = _TIME_RANGE_DAYS.get(time_range)
    if days is None:
        return None
    if now is None:
        end = _dt.date.today()
    elif isinstance(now, _dt.datetime):
        end = now.date()
    else:
        end = now
    return end - _dt.timedelta(days=days), end


def published_date_within(
    published_date: str | None,
    time_range: str,
    now: _dt.date | _dt.datetime | None = None,
) -> bool | None:
    """Whether a result's ``published_date`` falls inside the time window.

    Returns ``True``/``False`` when the date is present and parseable, and
    ``None`` when it cannot be determined (missing or malformed date) — so a
    caller can distinguish "stale" from "unknown" and never silently drops a
    result whose freshness cannot be established.
    """
    window = time_range_window(time_range, now)
    if window is None or not published_date:
        return None
    try:
        value = _dt.date.fromisoformat(str(published_date)[:10])
    except ValueError:
        return None
    start, end = window
    return start <= value <= end


def filter_results_by_time_range(
    results: Sequence[Any],
    time_range: str,
    now: _dt.date | _dt.datetime | None = None,
) -> list[Any]:
    """Locally post-filter results by ``published_date`` for ``time_range``.

    Keeps only results whose ``published_date`` is present, parseable, and
    inside the window. Results with a missing or unparseable date are excluded:
    declaring local ``time_range`` enforcement is a contract that the adapter
    reliably populates ``published_date``, so a result that cannot be proven
    fresh is dropped rather than silently bypassing the requested constraint.

    Never infers a date from other result fields — only the adapter-provided
    ``published_date`` is consulted. Unknown time-range values pass results
    through unchanged (the vocabulary is not fabricated).
    """
    if time_range not in _TIME_RANGE_DAYS:
        return list(results)
    kept: list[Any] = []
    for result in results:
        verdict = published_date_within(getattr(result, "published_date", None), time_range, now)
        if verdict is True:
            kept.append(result)
    return kept
