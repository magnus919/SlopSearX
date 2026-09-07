"""Tests for OpenMetrics instrumentation."""

from __future__ import annotations

import pytest
from prometheus_client.parser import text_string_to_metric_families

from slopsearx import metrics as m


class TestCounter:
    """Counter metric type."""

    def test_increment(self) -> None:
        c = m.Counter("test_total", "Test counter")
        c.inc({"label": "a"})
        c.inc({"label": "a"}, 2)
        rendered = c.render()
        assert 'test_total{label="a"} 3' in rendered

    def test_multiple_labels(self) -> None:
        c = m.Counter("test_total", "Test")
        c.inc({"engine": "brave"})
        c.inc({"engine": "wikipedia"})
        c.inc({"engine": "brave"})
        rendered = c.render()
        assert 'test_total{engine="brave"} 2' in rendered
        assert 'test_total{engine="wikipedia"} 1' in rendered

    def test_header(self) -> None:
        c = m.Counter("test_total", "Test counter")
        rendered = c.render()
        assert "# HELP test_total Test counter" in rendered
        assert "# TYPE test_total counter" in rendered


class TestGauge:
    """Gauge metric type."""

    def test_set_and_render(self) -> None:
        g = m.Gauge("test_status", "Test status")
        g.set({"engine": "brave"}, 0)
        g.set({"engine": "google"}, 1)
        rendered = g.render()
        assert 'test_status{engine="brave"} 0' in rendered
        assert 'test_status{engine="google"} 1' in rendered

    def test_overwrite(self) -> None:
        g = m.Gauge("test_status", "Test")
        g.set({"engine": "brave"}, 0)
        g.set({"engine": "brave"}, 2)
        rendered = g.render()
        assert 'test_status{engine="brave"} 2' in rendered


class TestHistogram:
    """Histogram metric type."""

    def test_cumulative_buckets_and_parser(self) -> None:
        h = m.Histogram("test_latency", "Test latency", buckets=(0.1, 0.5, 5.0))
        for value in [0, 0.1, 0.2, 0.5, 6]:
            h.observe({"engine": "brave"}, value)
        (family,) = text_string_to_metric_families(h.render())
        buckets = {sample.labels["le"]: sample.value for sample in family.samples if sample.name.endswith("_bucket")}
        assert buckets == {"0.1": 2, "0.5": 4, "5": 4, "+Inf": 5}
        assert next(sample.value for sample in family.samples if sample.name.endswith("_sum")) == pytest.approx(6.8)
        assert next(sample.value for sample in family.samples if sample.name.endswith("_count")) == 5
        assert family.type == "histogram"

    def test_bounded_retention(self) -> None:
        h = m.Histogram("test_latency", "Test latency")
        for _ in range(100_000):
            h.observe({}, 0.25)
        state = h._values[""]
        assert len(state.buckets) == len(h.buckets)
        assert state.count == 100_000
        assert state.total == 25_000
        assert len(list(text_string_to_metric_families(h.render()))[0].samples) == len(h.buckets) + 3

    def test_nonfinite_and_negative_latency_ignored(self) -> None:
        h = m.Histogram("test_latency", "Test latency")
        for value in [float("nan"), float("inf"), -1]:
            h.observe({}, value)
        assert not h._values

    @pytest.mark.parametrize("buckets", [(1.0, 1.0), (2.0, 1.0), (-1.0,), (float("inf"),)])
    def test_invalid_buckets(self, buckets: tuple[float, ...]) -> None:
        with pytest.raises(ValueError):
            m.Histogram("test", "Test", buckets=buckets)

    def test_empty_no_output(self) -> None:
        h = m.Histogram("test", "Test")
        rendered = h.render()
        assert "# HELP" in rendered
        assert "# TYPE" in rendered


class TestRenderMetrics:
    """Full render_metrics() output."""

    def test_render_produces_valid_openmetrics(self) -> None:
        # Record some data to ensure non-empty output
        m.engine_queries.inc({"engine": "brave"})
        m.engine_latency.observe({"engine": "brave"}, 0.34)
        m.engine_status.set({"engine": "brave"}, 0)
        m.cache_hits.inc({"type": "hit"})
        m.server_requests.inc({})

        rendered = m.render_metrics()

        assert "slopsearx_engine_queries_total" in rendered
        assert "slopsearx_engine_latency_seconds" in rendered
        assert "slopsearx_engine_status" in rendered
        assert "slopsearx_cache_hit_total" in rendered
        assert "slopsearx_server_requests_total" in rendered
        assert "# HELP" in rendered
        assert "# TYPE" in rendered
        assert list(text_string_to_metric_families(rendered))

    def test_render_includes_help_and_type(self) -> None:
        rendered = m.render_metrics()
        assert rendered.count("# HELP") >= 5
        assert rendered.count("# TYPE") >= 5


class TestMetricsEndpoint:
    """GET /metrics endpoint on server."""

    def test_metrics_endpoint_returns_200(self) -> None:
        from fastapi.testclient import TestClient

        import slopsearx.server as server_mod
        from slopsearx.server import app

        original = dict(server_mod._active_engines)
        server_mod._active_engines = {}

        try:
            with TestClient(app) as client:
                response = client.get("/metrics")
                assert response.status_code == 200
                assert "text/plain" in response.headers["content-type"]
                assert "slopsearx" in response.text
                assert list(text_string_to_metric_families(response.text))
        finally:
            server_mod._active_engines = original


def test_label_escaping_roundtrips() -> None:
    label = 'quotes" slash\\ and\nnewline'
    for metric in (m.Counter("escape_total", "Test"), m.Gauge("escape", "Test"), m.Histogram("escape_latency", "Test")):
        if isinstance(metric, m.Counter):
            metric.inc({"label": label})
        elif isinstance(metric, m.Gauge):
            metric.set({"label": label}, 1)
        else:
            metric.observe({"label": label}, 1)
        (family,) = text_string_to_metric_families(metric.render())
        assert all(sample.labels["label"] == label for sample in family.samples)


def test_engine_error_counter_counts_failures_not_empty(monkeypatch) -> None:
    from slopsearx.adapter import AdapterResponse, EngineStatus
    from slopsearx.service import SearchService

    counter = m.Counter("failures_total", "Failures")
    monkeypatch.setattr(m, "engine_errors", counter)
    for status in EngineStatus:
        SearchService._record_engine_metrics(None, "example", AdapterResponse(results=[], status=status))
    (family,) = text_string_to_metric_families(counter.render())
    assert family.samples[0].value == len(EngineStatus) - 1


def test_user_input_does_not_create_unbounded_metric_series(monkeypatch) -> None:
    from fastapi.testclient import TestClient

    import slopsearx.server as server

    formats = m.Counter("formats_total", "Formats")
    categories = m.Counter("categories_total", "Categories")
    monkeypatch.setattr(m, "server_requests_by_format", formats)
    monkeypatch.setattr(m, "server_requests_by_category", categories)
    with TestClient(server.app) as client:
        for i in range(50):
            client.get("/search", params={"q": "", "format": f"unique{i}", "categories": f"unique{i}"})
    assert formats._values == {'format="other"': 50}
    assert categories._values == {'category="other"': 50}
