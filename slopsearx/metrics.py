"""Prometheus text instrumentation (stdlib-only, no prometheus-client dependency).

Exposes per-engine counters, latency histogram, status gauges,
and cache hit/miss counters in Prometheus text format 0.0.4.
"""

from __future__ import annotations

import math
import time
from collections import defaultdict
from dataclasses import dataclass
from typing import Literal, Mapping


class _Metric:
    """Base metric with HELP and TYPE rendering."""

    def __init__(self, name: str, help_text: str, type_name: str) -> None:
        self.name = name
        self.help_text = help_text
        self.type_name = type_name

    def _header_lines(self) -> list[str]:
        return [
            f"# HELP {self.name} {self.help_text}",
            f"# TYPE {self.name} {self.type_name}",
        ]


class Counter(_Metric):
    """Monotonically increasing counter."""

    def __init__(self, name: str, help_text: str) -> None:
        super().__init__(name, help_text, "counter")
        self._values: dict[str, float] = defaultdict(float)

    def inc(self, labels: dict[str, str], amount: float = 1.0) -> None:
        key = _labels_key(labels)
        self._values[key] += amount

    def render(self) -> str:
        lines = self._header_lines()
        for key, val in sorted(self._values.items()):
            lines.append(f"{self.name}{{{key}}} {_format_val(val)}")
        return "\n".join(lines) + "\n"


class Gauge(_Metric):
    """Point-in-time value gauge."""

    def __init__(self, name: str, help_text: str) -> None:
        super().__init__(name, help_text, "gauge")
        self._values: dict[str, float] = {}

    def set(self, labels: dict[str, str], value: float) -> None:
        key = _labels_key(labels)
        self._values[key] = value

    def inc(self, labels: dict[str, str], amount: float = 1.0, *, minimum: float | None = None) -> None:
        key = _labels_key(labels)
        value = self._values.get(key, 0.0) + amount
        self._values[key] = max(minimum, value) if minimum is not None else value

    def render(self) -> str:
        lines = self._header_lines()
        for key, val in sorted(self._values.items()):
            lines.append(f"{self.name}{{{key}}} {_format_val(val)}")
        return "\n".join(lines) + "\n"


@dataclass
class _HistogramState:
    buckets: list[int]
    count: int = 0
    total: float = 0.0


class Histogram(_Metric):
    """Fixed cumulative buckets: memory and scrape work depend on series, not traffic."""

    def __init__(self, name: str, help_text: str, buckets: tuple[float, ...] | None = None) -> None:
        super().__init__(name, help_text, "histogram")
        self.buckets = (
            buckets if buckets is not None else (0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60)
        )
        if any(not math.isfinite(v) or v < 0 for v in self.buckets) or tuple(sorted(set(self.buckets))) != self.buckets:
            raise ValueError("Histogram buckets must be finite, nonnegative, and strictly increasing")
        self._values: dict[str, _HistogramState] = {}

    def observe(self, labels: dict[str, str], value: float) -> None:
        # Invalid adapter telemetry must not corrupt counters or break search.
        if not math.isfinite(value) or value < 0:
            return
        if "le" in labels:
            raise ValueError("le is reserved for histogram bucket boundaries")
        key = _labels_key(labels)
        if key not in self._values:
            self._values[key] = _HistogramState([0] * len(self.buckets))
        state = self._values[key]
        state.count += 1
        state.total += value
        for i, bound in enumerate(self.buckets):
            if value <= bound:
                state.buckets[i] += 1

    def render(self) -> str:
        lines = self._header_lines()
        for key, state in sorted(self._values.items()):
            prefix = f"{key}," if key else ""
            for bound, count in zip(self.buckets, state.buckets):
                lines.append(f'{self.name}_bucket{{{prefix}le="{_format_val(bound)}"}} {count}')
            lines.append(f'{self.name}_bucket{{{prefix}le="+Inf"}} {state.count}')
            lines.append(f"{self.name}_sum{{{key}}} {_format_val(state.total)}")
            lines.append(f"{self.name}_count{{{key}}} {state.count}")
        return "\n".join(lines) + "\n"


# --- Helpers ---


def _labels_key(labels: dict[str, str]) -> str:
    """Render label dict as key=value,val pairs."""
    parts = ",".join(f'{k}="{_escape_label(v)}"' for k, v in sorted(labels.items()))
    return parts


def _format_val(val: float) -> str:
    """Format metric value, using integer representation when whole."""
    if not math.isfinite(val):
        return "NaN" if math.isnan(val) else ("+Inf" if val > 0 else "-Inf")
    if val == int(val):
        return str(int(val))
    return f"{val:.6g}"


def _escape_label(value: str) -> str:
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"')


# --- Global metric instances ---

engine_queries = Counter(
    "slopsearx_engine_queries_total",
    "Total queries dispatched per engine",
)

engine_errors = Counter(
    "slopsearx_engine_errors_total",
    "Failed engine outcomes, excluding successful empty results",
)

engine_latency = Histogram(
    "slopsearx_engine_latency_seconds",
    "Query latency per engine in seconds",
)

engine_status = Gauge(
    "slopsearx_engine_status",
    "Engine status (0=ok, 1=degraded, 2=down)",
)

cache_hits = Counter(
    "slopsearx_cache_hit_total",
    "Cache hit/miss counters",
)

server_requests = Counter(
    "slopsearx_server_requests_total",
    "Total search requests handled",
)

# Product analytics: per-category and per-format request counts.
# Enables operators to understand *what* is being searched and in
# which format, without collecting any user-identifiable data.
server_requests_by_category = Counter(
    "slopsearx_server_requests_by_category_total",
    "Search requests per category",
)

server_requests_by_format = Counter(
    "slopsearx_server_requests_by_format_total",
    "Search requests per output format",
)

server_errors_total = Counter(
    "slopsearx_server_errors_total",
    "Server errors by type (timeout, circuit_open, rate_limited, internal)",
)

# MCP surface telemetry — per-tool counts, latency, and structured errors.
# Raw values are operator-facing (/metrics); the MCP layer never exposes
# them to agents directly.
mcp_tool_calls = Counter(
    "slopsearx_mcp_tool_calls_total",
    "MCP tool invocations per tool",
)

mcp_tool_errors = Counter(
    "slopsearx_mcp_tool_errors_total",
    "MCP tool errors per tool and error code",
)

mcp_tool_latency = Histogram(
    "slopsearx_mcp_tool_latency_seconds",
    "MCP tool latency per tool in seconds",
)

# Durable workflow telemetry.  Every label value is selected from the closed
# vocabularies below; none of these helpers accepts caller or stored text as a
# label.  This keeps cardinality independent of tenants, queries and artifacts.
WorkflowKind = Literal["research", "dependency_dossier", "staged_search", "saved_search", "retrieval_receipt"]
WORKFLOW_KINDS: tuple[WorkflowKind, ...] = (
    "research",
    "dependency_dossier",
    "staged_search",
    "saved_search",
    "retrieval_receipt",
)
WORKFLOW_MODES = frozenset({"durable_leased", "durable", "immediate"})
WORKFLOW_OUTCOMES = frozenset({"succeeded", "partial", "failed", "interrupted", "cancelled", "expired"})
WORKFLOW_REASONS = frozenset(
    {"policy", "budget", "capacity", "idempotency", "lease_lost", "execution", "deadline", "manual"}
)
WORKFLOW_ARTIFACTS = frozenset({"job", "operation", "definition", "report", "receipt"})

workflow_accepted = Counter("slopsearx_workflow_accepted_total", "Accepted durable workflow operations")
workflow_terminal = Counter("slopsearx_workflow_terminal_total", "Durable workflow terminal outcomes")
workflow_retries = Counter("slopsearx_workflow_retries_total", "Durable workflow retry attempts")
workflow_lease_recoveries = Counter(
    "slopsearx_workflow_lease_recoveries_total", "Expired workflow leases recovered by a worker"
)
workflow_rejections = Counter("slopsearx_workflow_rejections_total", "Rejected workflow operations by reason class")
workflow_expired = Counter("slopsearx_workflow_expired_total", "Observed durable workflow artifact expirations")
workflow_queued = Gauge("slopsearx_workflow_queued", "Queued workflow operations visible to this replica")
workflow_running = Gauge("slopsearx_workflow_running", "Running workflow operations visible to this replica")
workflow_active_leases = Gauge("slopsearx_workflow_active_leases", "Active workflow leases held by this replica")
workflow_oldest_claimable_age = Gauge(
    "slopsearx_workflow_oldest_claimable_age_seconds", "Age of the oldest claimable workflow operation"
)
workflow_store_items = Gauge("slopsearx_workflow_store_items", "Bounded workflow store or index cardinality")
workflow_queue_wait = Histogram("slopsearx_workflow_queue_wait_seconds", "Workflow queue wait before claim")
workflow_execution = Histogram("slopsearx_workflow_execution_seconds", "Workflow execution duration")
workflow_report_generation = Histogram(
    "slopsearx_workflow_report_generation_seconds", "Saved-search report generation duration"
)
workflow_admitted_results = Histogram(
    "slopsearx_workflow_admitted_results",
    "Results admitted to one durable workflow artifact",
    buckets=(0, 1, 5, 10, 25, 50, 100, 250, 500),
)
SAVED_EVENT_TYPES = frozenset(
    {"report_created", "run_incomparable", "run_failed", "definition_paused", "definition_expired"}
)
SAVED_EVENT_READ_OUTCOMES = frozenset({"delivered", "empty", "gap", "redacted", "rejected"})
SAVED_EVENT_ACK_OUTCOMES = frozenset({"advanced", "idempotent", "rejected"})
SAVED_EVENT_CAPACITY_RESOURCES = frozenset({"stream", "consumer"})
saved_event_publications = Counter(
    "slopsearx_saved_search_event_publications_total", "Saved-search outbox events published by event type"
)
saved_event_reads = Counter(
    "slopsearx_saved_search_event_reads_total", "Saved-search outbox read operations by closed outcome"
)
saved_event_acknowledgements = Counter(
    "slopsearx_saved_search_event_acknowledgements_total",
    "Saved-search outbox acknowledgement operations by closed outcome",
)
saved_event_retention_gaps = Counter(
    "slopsearx_saved_search_event_retention_gaps_total", "Saved-search outbox reads that observed a retention gap"
)
saved_event_capacity_rejections = Counter(
    "slopsearx_saved_search_event_capacity_rejections_total",
    "Saved-search outbox operations rejected by bounded resource",
)
saved_event_backlog_age = Gauge(
    "slopsearx_saved_search_event_backlog_age_seconds",
    "Age of the oldest event returned by the most recent local consumer read",
)
_workflow_oldest_started: dict[str, float] = {}


def _closed(value: str, allowed: tuple[str, ...] | frozenset[str], field: str) -> str:
    if value not in allowed:
        raise ValueError(f"unsupported workflow {field}: {value}")
    return value


def record_workflow_accepted(workflow: WorkflowKind, mode: str) -> None:
    workflow_accepted.inc(
        {"workflow": _closed(workflow, WORKFLOW_KINDS, "kind"), "mode": _closed(mode, WORKFLOW_MODES, "mode")}
    )


def record_workflow_terminal(workflow: WorkflowKind, outcome: str, duration_seconds: float | None = None) -> None:
    labels = {
        "workflow": _closed(workflow, WORKFLOW_KINDS, "kind"),
        "outcome": _closed(outcome, WORKFLOW_OUTCOMES, "outcome"),
    }
    workflow_terminal.inc(labels)
    if duration_seconds is not None:
        workflow_execution.observe(labels, duration_seconds)


def transition_workflow(workflow: WorkflowKind, previous: str | None, current: str) -> None:
    """Balance process-local lifecycle gauges at a durable state transition."""
    label = {"workflow": _closed(workflow, WORKFLOW_KINDS, "kind")}
    key = _labels_key(label)
    for state, gauge in (("queued", workflow_queued), ("running", workflow_running)):
        if previous == state and current != state:
            gauge.inc(label, -1, minimum=0)
        if current == state and previous != state:
            gauge.inc(label, 1)
    if current == "queued" and previous != "queued" and workflow_queued._values.get(key, 0) == 1:
        _workflow_oldest_started[workflow] = time.monotonic()
    if previous == "queued" and current != "queued" and workflow_queued._values.get(key, 0) == 0:
        _workflow_oldest_started.pop(workflow, None)
        workflow_oldest_claimable_age.set(label, 0)
    if previous == "running" and current != "running":
        workflow_active_leases.inc(label, -1, minimum=0)
    if current == "running" and previous != "running":
        workflow_active_leases.inc(label, 1)


def record_workflow_rejection(workflow: WorkflowKind, reason: str) -> None:
    workflow_rejections.inc(
        {"workflow": _closed(workflow, WORKFLOW_KINDS, "kind"), "reason": _closed(reason, WORKFLOW_REASONS, "reason")}
    )


def record_workflow_retry(workflow: WorkflowKind, reason: str = "manual") -> None:
    workflow_retries.inc(
        {"workflow": _closed(workflow, WORKFLOW_KINDS, "kind"), "reason": _closed(reason, WORKFLOW_REASONS, "reason")}
    )


def record_workflow_recovery(workflow: WorkflowKind, amount: int = 1) -> None:
    if amount > 0:
        workflow_lease_recoveries.inc({"workflow": _closed(workflow, WORKFLOW_KINDS, "kind")}, amount)


def record_workflow_expiry(workflow: WorkflowKind, artifact: str, amount: int = 1) -> None:
    if amount > 0:
        workflow_expired.inc(
            {
                "workflow": _closed(workflow, WORKFLOW_KINDS, "kind"),
                "artifact": _closed(artifact, WORKFLOW_ARTIFACTS, "artifact"),
            },
            amount,
        )


def reconcile_workflow(
    workflow: WorkflowKind,
    *,
    queued: int,
    running: int,
    active_leases: int,
    oldest_claimable_age_seconds: float,
) -> None:
    """Replace lifecycle gauges from a bounded authoritative observation."""
    label = {"workflow": _closed(workflow, WORKFLOW_KINDS, "kind")}
    workflow_queued.set(label, max(0, queued))
    workflow_running.set(label, max(0, running))
    workflow_active_leases.set(label, max(0, active_leases))
    workflow_oldest_claimable_age.set(label, max(0.0, oldest_claimable_age_seconds))
    if queued:
        _workflow_oldest_started[workflow] = time.monotonic() - max(0.0, oldest_claimable_age_seconds)
    else:
        _workflow_oldest_started.pop(workflow, None)


def _refresh_oldest_claimable_ages() -> None:
    now = time.monotonic()
    for workflow, started in _workflow_oldest_started.items():
        workflow_oldest_claimable_age.set({"workflow": workflow}, max(0.0, now - started))


def workflow_health_summary(*, availability: Mapping[WorkflowKind, bool]) -> dict[str, dict[str, str | bool]]:
    """Return store availability without exposing cross-tenant activity."""
    summary: dict[str, dict[str, str | bool]] = {}
    for workflow in WORKFLOW_KINDS:
        available = bool(availability.get(workflow, False))
        summary[workflow] = {"available": available, "status": "available" if available else "unavailable"}
    return summary


def record_saved_event_publication(event_type: str, amount: int = 1) -> None:
    if amount > 0:
        saved_event_publications.inc({"event_type": _closed(event_type, SAVED_EVENT_TYPES, "event type")}, amount)


def record_saved_event_read(outcome: str, *, oldest_age_seconds: float | None = None) -> None:
    saved_event_reads.inc({"outcome": _closed(outcome, SAVED_EVENT_READ_OUTCOMES, "event read outcome")})
    if outcome == "gap":
        saved_event_retention_gaps.inc({})
    if oldest_age_seconds is not None:
        saved_event_backlog_age.set({}, max(0.0, oldest_age_seconds))
    elif outcome in {"empty", "gap"}:
        saved_event_backlog_age.set({}, 0.0)


def record_saved_event_ack(outcome: str) -> None:
    saved_event_acknowledgements.inc({"outcome": _closed(outcome, SAVED_EVENT_ACK_OUTCOMES, "event ack outcome")})


def record_saved_event_capacity(resource: str) -> None:
    saved_event_capacity_rejections.inc(
        {"resource": _closed(resource, SAVED_EVENT_CAPACITY_RESOURCES, "event capacity resource")}
    )


# --- Render all metrics ---


def render_metrics() -> str:
    """Render all registered metrics in Prometheus text format 0.0.4."""
    _refresh_oldest_claimable_ages()
    parts = [
        engine_queries.render(),
        engine_errors.render(),
        engine_latency.render(),
        engine_status.render(),
        cache_hits.render(),
        server_requests.render(),
        server_requests_by_category.render(),
        server_requests_by_format.render(),
        server_errors_total.render(),
        mcp_tool_calls.render(),
        mcp_tool_errors.render(),
        mcp_tool_latency.render(),
        workflow_accepted.render(),
        workflow_terminal.render(),
        workflow_retries.render(),
        workflow_lease_recoveries.render(),
        workflow_rejections.render(),
        workflow_expired.render(),
        workflow_queued.render(),
        workflow_running.render(),
        workflow_active_leases.render(),
        workflow_oldest_claimable_age.render(),
        workflow_store_items.render(),
        workflow_queue_wait.render(),
        workflow_execution.render(),
        workflow_report_generation.render(),
        workflow_admitted_results.render(),
        saved_event_publications.render(),
        saved_event_reads.render(),
        saved_event_acknowledgements.render(),
        saved_event_retention_gaps.render(),
        saved_event_capacity_rejections.render(),
        saved_event_backlog_age.render(),
    ]
    return "".join(parts)
