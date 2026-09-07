"""Prometheus text instrumentation (stdlib-only, no prometheus-client dependency).

Exposes per-engine counters, latency histogram, status gauges,
and cache hit/miss counters in Prometheus text format 0.0.4.
"""

from __future__ import annotations

import math
from collections import defaultdict
from dataclasses import dataclass


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


# --- Render all metrics ---


def render_metrics() -> str:
    """Render all registered metrics in Prometheus text format 0.0.4."""
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
    ]
    return "".join(parts)
