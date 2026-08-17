"""Deterministic tests for observed engine health (issue 190).

Locks in the contract that engine health is derived from classified search
outcomes (plus circuit-breaker/auth state), never fabricated from
registration or configuration:

- a dispatched outcome records a redacted observed-health summary;
- never-observed engines stay ``unknown``, never ``ok``;
- stale observations are visibly stale, never fresh;
- circuit state and authentication readiness are distinct signals;
- HTTP /health and the MCP status surface share one vocabulary.
"""

from __future__ import annotations

import datetime as _dt
import time
from typing import Any

from fastapi.testclient import TestClient

import engines  # noqa: F401 — triggers @register_engine to populate the registry
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
)
from slopsearx.capabilities import (
    CapabilityCatalog,
    build_engine_health,
)
from slopsearx.config import EngineEntry, load_config
from slopsearx.service import AppContext, SearchRequest, SearchService


class _FakeEngine(EngineAdapter):
    """Concrete adapter returning a fixed, classified outcome."""

    name = "fakeeng"
    display_name = "Fake Engine"
    categories = ["general"]

    def __init__(self, status: EngineStatus = EngineStatus.OK, count: int = 1) -> None:
        super().__init__()
        self._status = status
        self._count = count
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(
                results=[],
                status=self._status,
                error_message="simulated failure",
                latency_ms=2.0,
            )
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://example.com/{self.name}/{i}",
                    title=f"{self.name} result {i}",
                    content="content",
                    engine=self.name,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=3.0,
        )


def _service(engine: EngineAdapter) -> SearchService:
    return SearchService(
        AppContext(
            active_engines={engine.name: engine},
            tier1_engines={engine.name},
        )
    )


def _request() -> SearchRequest:
    return SearchRequest(query="deterministic fixture query")


# ---------------------------------------------------------------------------
# Redacted observation recording
# ---------------------------------------------------------------------------


class TestObservationRecording:
    def test_record_observation_stores_only_redacted_summary(self) -> None:
        engine = _FakeEngine(EngineStatus.TIMEOUT)
        engine.record_observation(EngineStatus.TIMEOUT, latency_ms=123.4, result_count=0)

        assert engine.last_observed_status == "timeout"
        assert engine.last_observed_latency_ms == 123.4
        assert engine.last_observed_result_count == 0
        assert engine.last_observed_at is not None
        # No query or result content is ever retained on the adapter.
        for attr in vars(engine):
            assert "deterministic" not in str(getattr(engine, attr))

    def test_record_observation_tracks_success(self) -> None:
        engine = _FakeEngine(EngineStatus.OK, count=3)
        engine.record_observation(EngineStatus.OK, latency_ms=10.0, result_count=3)
        assert engine.last_observed_status == "ok"
        assert engine.last_observed_result_count == 3

    async def test_service_records_success_outcome(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        await _service(engine).search(_request())
        assert engine.last_observed_status == "ok"
        assert engine.last_observed_at is not None
        assert engine.last_observed_result_count == 1

    async def test_service_records_timeout_outcome(self) -> None:
        engine = _FakeEngine(EngineStatus.TIMEOUT)
        await _service(engine).search(_request())
        assert engine.last_observed_status == "timeout"

    async def test_service_records_rate_limited_outcome(self) -> None:
        engine = _FakeEngine(EngineStatus.RATE_LIMITED)
        await _service(engine).search(_request())
        assert engine.last_observed_status == "rate_limited"


# ---------------------------------------------------------------------------
# Unknown / stale / circuit / auth signals
# ---------------------------------------------------------------------------


class TestHealthSignals:
    def test_never_observed_is_unknown_not_ok(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        record = build_engine_health("wikipedia", engine)
        assert record["status"] == "unknown"
        assert record["status_at"] is None
        assert record["stale"] is False
        assert record["status"] != "ok"

    def test_stale_observation_is_visibly_stale(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        now = time.time()
        engine.last_observed_status = "ok"
        engine.last_observed_at = now - 400.0

        record = build_engine_health("wikipedia", engine, now=now, stale_after=300.0)

        assert record["status"] == "ok"
        assert record["stale"] is True
        assert record["status_at"] is not None
        parsed = _dt.datetime.fromisoformat(record["status_at"])
        assert abs(parsed.timestamp() - engine.last_observed_at) < 1.0

    def test_stale_and_unknown_cannot_be_presented_as_fresh_healthy(self) -> None:
        fresh = build_engine_health("wikipedia", None, now=time.time(), stale_after=300.0)
        assert fresh["status"] == "unknown"
        assert fresh["stale"] is False

        engine = _FakeEngine(EngineStatus.OK)
        engine.last_observed_status = "ok"
        engine.last_observed_at = time.time() - 10_000.0
        stale = build_engine_health("wikipedia", engine, now=time.time(), stale_after=300.0)
        assert stale["status"] == "ok"
        assert stale["stale"] is True

    def test_circuit_state_is_distinct_from_observed_health(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        engine.circuit_open_until = time.time() + 60.0
        engine.consecutive_errors = 5

        record = build_engine_health("wikipedia", engine)

        # Circuit is open even though no observation exists yet.
        assert record["circuit_open"] is True
        assert record["consecutive_errors"] == 5
        assert record["status"] == "unknown"

    def test_auth_readiness_is_distinct_from_health(self) -> None:
        config = load_config()
        config.engines["brave"] = EngineEntry()  # required key, none configured
        catalog = CapabilityCatalog(config=config, adapters={})
        cap = catalog.get("brave")
        assert cap is not None

        record = build_engine_health("brave", None, cap)

        assert record["auth_class"] == "required"
        assert record["auth_configured"] is False
        assert record["status"] == "unknown"
        assert record["configured"] is True


# ---------------------------------------------------------------------------
# Catalog reflects live observations
# ---------------------------------------------------------------------------


class TestCatalogObservedHealth:
    async def test_catalog_reflects_observed_outcome(self) -> None:
        engine = _FakeEngine(EngineStatus.TIMEOUT)
        engine.name = "wikipedia"
        catalog = CapabilityCatalog(config=load_config(), adapters={"wikipedia": engine})

        assert catalog.get("wikipedia").last_known_status == "unknown"  # type: ignore[union-attr]

        await _service(engine).search(_request())

        cap = catalog.get("wikipedia")
        assert cap is not None
        assert cap.last_known_status == "timeout"
        assert cap.last_known_status_at is not None
        assert cap.last_known_status_stale is False
        assert cap.circuit_consecutive_errors == 1

    def test_catalog_without_adapter_stays_unknown(self) -> None:
        catalog = CapabilityCatalog(config=load_config(), adapters={})
        assert catalog.get("wikipedia").last_known_status == "unknown"  # type: ignore[union-attr]
        assert catalog.get("wikipedia").last_known_status_at is None  # type: ignore[union-attr]
        assert catalog.get("wikipedia").last_known_status_stale is False  # type: ignore[union-attr]


# ---------------------------------------------------------------------------
# HTTP /health agrees with the shared builder
# ---------------------------------------------------------------------------


class TestHealthEndpointObserved:
    def test_health_reports_unknown_until_observed_then_ok(self) -> None:
        import slopsearx.server as server_mod
        from slopsearx.server import app

        original = dict(server_mod._active_engines)
        original_router = server_mod._router
        engine = _FakeEngine(EngineStatus.OK)
        engine.name = "wikipedia"
        try:
            with TestClient(app) as client:
                server_mod._active_engines = {"wikipedia": engine}
                server_mod._router = None

                before = client.get("/health").json()["engines"]["wikipedia"]
                assert before["status"] == "unknown"

                client.get("/search", params={"q": "test", "engines": "wikipedia"})

                after = client.get("/health").json()["engines"]["wikipedia"]
                assert after["status"] == "ok"
                assert after["status_at"] is not None

                # /health and the shared builder agree on the record fields.
                expected = build_engine_health(
                    "wikipedia",
                    engine,
                    CapabilityCatalog(config=load_config(), adapters={"wikipedia": engine}).get("wikipedia"),
                )
                assert after == expected
        finally:
            server_mod._active_engines = original
            server_mod._router = original_router
