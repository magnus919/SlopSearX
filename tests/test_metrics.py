"""Tests for OpenMetrics instrumentation."""

from __future__ import annotations

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

    def test_observe_and_quantiles(self) -> None:
        h = m.Histogram("test_latency", "Test latency", quantiles=[0.5, 0.99])
        for v in [0.1, 0.2, 0.3, 0.4, 0.5]:
            h.observe({"engine": "brave"}, v)
        rendered = h.render()
        assert 'quantile="0.5"' in rendered
        assert 'quantile="0.99"' in rendered
        assert "test_latency_sum" in rendered
        assert "test_latency_count" in rendered

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
        finally:
            server_mod._active_engines = original
