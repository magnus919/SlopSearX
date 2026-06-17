"""OpenMetrics instrumentation (stdlib-only, no prometheus-client dependency).

Exposes per-engine counters, latency histogram, status gauges,
and cache hit/miss counters in standard OpenMetrics text format.
"""

from __future__ import annotations

from collections import defaultdict


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


class Histogram(_Metric):
    """Client-side histogram with configurable quantiles.

    Stores all observed values, computes quantiles on render.
    Suitable for low-to-moderate cardinality (per-engine latency).
    """

    def __init__(self, name: str, help_text: str, quantiles: list[float] | None = None) -> None:
        super().__init__(name, help_text, "histogram")
        self.quantiles = quantiles or [0.5, 0.9, 0.99]
        self._values: dict[str, list[float]] = defaultdict(list)

    def observe(self, labels: dict[str, str], value: float) -> None:
        key = _labels_key(labels)
        self._values[key].append(value)

    def render(self) -> str:
        lines = self._header_lines()
        for key in sorted(self._values.keys()):
            vals = sorted(self._values[key])
            if not vals:
                continue
            for q in self.quantiles:
                qval = _quantile(vals, q)
                # Append quantile label
                qkey = key.rstrip("}") + f',quantile="{q}")'
                lines.append(f"{self.name}{{{qkey}}} {_format_val(qval)}")
            lines.append(f"{self.name}_sum{{{key}}} {_format_val(sum(vals))}")
            lines.append(f"{self.name}_count{{{key}}} {len(vals)}")
        return "\n".join(lines) + "\n"


# --- Helpers ---


def _labels_key(labels: dict[str, str]) -> str:
    """Render label dict as key=value,val pairs."""
    parts = ",".join(f'{k}="{v}"' for k, v in sorted(labels.items()))
    return parts


def _format_val(val: float) -> str:
    """Format metric value, using integer representation when whole."""
    if val == int(val):
        return str(int(val))
    return f"{val:.6g}"


def _quantile(sorted_vals: list[float], q: float) -> float:
    """Compute quantile from sorted values using linear interpolation."""
    if not sorted_vals:
        return 0.0
    n = len(sorted_vals)
    idx = q * (n - 1)
    lo = int(idx)
    hi = min(lo + 1, n - 1)
    frac = idx - lo
    return sorted_vals[lo] * (1 - frac) + sorted_vals[hi] * frac


# --- Global metric instances ---

engine_queries = Counter(
    "slopsearx_engine_queries_total",
    "Total queries dispatched per engine",
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


# --- Render all metrics ---


def render_metrics() -> str:
    """Render all registered metrics in OpenMetrics text format."""
    parts = [
        engine_queries.render(),
        engine_latency.render(),
        engine_status.render(),
        cache_hits.render(),
        server_requests.render(),
        server_requests_by_category.render(),
        server_requests_by_format.render(),
        server_errors_total.render(),
    ]
    return "".join(parts)
