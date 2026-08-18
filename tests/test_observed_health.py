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

import asyncio
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


class _HangingEngine(EngineAdapter):
    """Adapter that never returns — used to force service-synthesized timeouts."""

    name = "hangeng"
    display_name = "Hanging Engine"
    categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        await asyncio.sleep(60.0)
        raise AssertionError("hanging engine unexpectedly completed")


class _LatencylessEngine(EngineAdapter):
    """Adapter that never reports a latency (relies on the ``0.0`` default)."""

    name = "latencyless"
    display_name = "Latencyless Engine"
    categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        # Deliberately no ``latency_ms``: the dataclass default ``0.0`` means
        # "not measured" and must never be recorded as a 0ms observation.
        return AdapterResponse(
            results=[
                SearchResult(
                    url="https://example.com/latencyless/0",
                    title="latencyless result 0",
                    content="content",
                    engine=self.name,
                )
            ],
            status=EngineStatus.OK,
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

    async def test_unmeasured_latency_records_none_not_zero(self) -> None:
        """An adapter that never reports latency records None, never 0.0.

        ``AdapterResponse.latency_ms`` defaults to ``0.0``, which means "not
        measured". That default must not be recorded as ``0.0`` — it would be
        indistinguishable from a real 0ms measurement on /health (issue 190
        review).
        """
        engine = _LatencylessEngine()
        await _service(engine).search(_request())
        assert engine.last_observed_status == "ok"
        assert engine.last_observed_latency_ms is None
        record = build_engine_health(engine)
        assert record["last_observed_latency_ms"] is None


# ---------------------------------------------------------------------------
# Unknown / stale / circuit / auth signals
# ---------------------------------------------------------------------------


class TestHealthSignals:
    def test_never_observed_is_unknown_not_ok(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        record = build_engine_health(engine)
        assert record["status"] == "unknown"
        assert record["status_at"] is None
        assert record["stale"] is False
        assert record["status"] != "ok"

    def test_health_surfaces_observed_latency_and_result_count(self) -> None:
        """build_engine_health exposes the last measured latency/result count.

        Null before any observation and never populated from a synthetic
        outcome — the fields surface only real adapter-reported values.
        """
        engine = _FakeEngine(EngineStatus.OK)

        never = build_engine_health(engine)
        assert never["last_observed_latency_ms"] is None
        assert never["last_observed_result_count"] is None

        engine.record_observation(EngineStatus.OK, latency_ms=11.5, result_count=4)
        observed = build_engine_health(engine)
        assert observed["last_observed_latency_ms"] == 11.5
        assert observed["last_observed_result_count"] == 4
        assert observed["status"] == "ok"

    def test_stale_observation_is_visibly_stale(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        now = time.time()
        engine.last_observed_status = "ok"
        engine.last_observed_at = now - 400.0

        record = build_engine_health(engine, now=now, stale_after=300.0)

        assert record["status"] == "ok"
        assert record["stale"] is True
        assert record["status_at"] is not None
        parsed = _dt.datetime.fromisoformat(record["status_at"])
        assert abs(parsed.timestamp() - engine.last_observed_at) < 1.0

    def test_stale_and_unknown_cannot_be_presented_as_fresh_healthy(self) -> None:
        fresh = build_engine_health(None, now=time.time(), stale_after=300.0)
        assert fresh["status"] == "unknown"
        assert fresh["stale"] is False

        engine = _FakeEngine(EngineStatus.OK)
        engine.last_observed_status = "ok"
        engine.last_observed_at = time.time() - 10_000.0
        stale = build_engine_health(engine, now=time.time(), stale_after=300.0)
        assert stale["status"] == "ok"
        assert stale["stale"] is True

    def test_circuit_state_is_distinct_from_observed_health(self) -> None:
        engine = _FakeEngine(EngineStatus.OK)
        engine.circuit_open_until = time.time() + 60.0
        engine.consecutive_errors = 5

        record = build_engine_health(engine)

        # Circuit is open even though no observation exists yet.
        assert record["circuit_open"] is True
        assert record["circuit_consecutive_errors"] == 5
        assert record["status"] == "unknown"

    def test_degraded_path_reports_auth_fields_as_null(self) -> None:
        """The degraded fallback (capability unavailable) never fabricates auth state.

        When the capability is unavailable the builder must emit explicit
        null for ``auth_class`` / ``auth_configured`` — never a fabricated
        ``unknown`` / ``false`` that would contradict a key-requiring engine
        still serving from its startup config (issue 190 review).
        """
        engine = _FakeEngine(EngineStatus.OK)
        # capability=None models the memoized catalog failing; the adapter is
        # still the running engine, so ``configured`` stays honest.
        record = build_engine_health(engine)
        assert record["configured"] is True
        assert record["auth_class"] is None
        assert record["auth_configured"] is None

    def test_auth_readiness_is_distinct_from_health(self) -> None:
        config = load_config()
        config.engines["brave"] = EngineEntry()  # required key, none configured
        catalog = CapabilityCatalog(config=config, adapters={})
        cap = catalog.get("brave")
        assert cap is not None

        record = build_engine_health(None, cap)

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
# Synthetic (service-fabricated) timeout latency is never observed
# ---------------------------------------------------------------------------


class TestDeadlineSyntheticLatency:
    async def test_gather_deadline_marks_synthetic_timeout(self) -> None:
        """A deadline-cut engine is marked synthetic so its latency is not observed."""
        service = _service(_FakeEngine(EngineStatus.OK))

        async def _never_completes() -> AdapterResponse:
            await asyncio.sleep(60.0)
            raise AssertionError("slow task unexpectedly completed")

        task = asyncio.create_task(_never_completes())
        results = await service._gather_with_deadline(  # type: ignore[attr-defined]
            [task], ["fakeeng"], deadline_s=0.01, started_engines={"fakeeng"}
        )

        assert results[0].status == EngineStatus.TIMEOUT
        assert results[0].synthetic is True
        assert task.done()

    async def test_dispatch_timeout_marks_synthetic(self) -> None:
        """A per-engine timeout the adapter never returned from is synthetic."""
        engine = _HangingEngine()
        service = SearchService(AppContext(active_engines={"hangeng": engine}))
        result = await service._dispatch_engine("hangeng", engine, "q", {}, timeout_s=0.01)

        assert result.status == EngineStatus.TIMEOUT
        assert result.synthetic is True

    async def test_synthetic_timeout_does_not_record_fabricated_latency(self) -> None:
        """The service stores None latency for synthetic outcomes (issue 190).

        A deadline timeout carries a fabricated latency bound (the deadline),
        never a measured latency, so ``last_observed_latency_ms`` must stay
        null while the (real) observed result count of zero is retained.
        """
        engine = _FakeEngine(EngineStatus.OK)
        service = _service(engine)

        async def _fake_gather(
            tasks: list[asyncio.Task[AdapterResponse]],
            engine_names: list[str],
            deadline_s: float = 10.0,
            started_engines: set[str] | None = None,
        ) -> list[AdapterResponse]:
            del deadline_s, started_engines
            # Let the dispatch tasks actually start (so ``started_engines`` is
            # populated), then fabricate a deadline-timeout outcome — the
            # scenario where the service synthesized the response.
            await asyncio.gather(*tasks, return_exceptions=True)
            return [
                AdapterResponse(
                    results=[],
                    status=EngineStatus.TIMEOUT,
                    error_message="timed out after 0.01s",
                    latency_ms=10.0,
                    synthetic=True,
                )
                for _ in engine_names
            ]

        original_gather = service._gather_with_deadline
        service._gather_with_deadline = _fake_gather  # type: ignore[method-assign]
        try:
            await service.search(_request())
        finally:
            service._gather_with_deadline = original_gather  # type: ignore[method-assign]

        assert engine.last_observed_status == "timeout"
        assert engine.last_observed_latency_ms is None
        assert engine.last_observed_result_count == 0
        # The observed-health record agrees: latency stays null.
        record = build_engine_health(engine)
        assert record["last_observed_latency_ms"] is None
        assert record["last_observed_result_count"] == 0


# ---------------------------------------------------------------------------
# HTTP /health memoizes the startup config/catalog snapshot
# ---------------------------------------------------------------------------


class TestHealthConfigSnapshot:
    def test_runtime_env_change_does_not_alter_health_after_startup(self, monkeypatch: Any) -> None:
        """The /health probe must not re-read config or rebuild the catalog.

        k8s and Docker poll /health continuously; re-reading config.yaml and
        re-scanning env vars per probe is disk/registry work, and reporting
        the *current* file/env state contradicts the startup state the running
        adapters were built from. The snapshot is captured once: a runtime env
        change after startup must not change the health output.
        """
        import slopsearx.server as server_mod
        from slopsearx.server import app

        # Reset the memo so this test controls the startup snapshot, and make
        # sure the env does not carry an unrelated brave key.
        server_mod._health_config_cache = None
        server_mod._health_catalog_cache = None
        server_mod._health_catalog_engines = None
        monkeypatch.delenv("ENGINE_BRAVE_API_KEY", raising=False)

        original = dict(server_mod._active_engines)
        try:
            with TestClient(app) as client:
                before = client.get("/health").json()["engines"]
                assert before["brave"]["auth_configured"] is False

                # A runtime env change after startup must not leak into the
                # health record: the probe uses the startup config snapshot.
                monkeypatch.setenv("ENGINE_BRAVE_API_KEY", "changed-after-startup")
                after = client.get("/health").json()["engines"]

                assert after["brave"] == before["brave"]
        finally:
            server_mod._active_engines = original
            server_mod._health_config_cache = None
            server_mod._health_catalog_cache = None
            server_mod._health_catalog_engines = None

    def test_health_survives_catalog_failure(self, monkeypatch: Any) -> None:
        """A config/catalog failure degrades to liveness, never a 500.

        A malformed/unreadable config file must not take the probe down even
        though the server is alive (issue 190 review): the handler falls back
        to a minimal record built from the running adapter alone.
        """
        import slopsearx.server as server_mod
        from slopsearx.server import app

        server_mod._health_config_cache = None
        server_mod._health_catalog_cache = None
        server_mod._health_catalog_engines = None
        original = dict(server_mod._active_engines)

        def _boom_catalog() -> None:
            raise RuntimeError("config unreadable")

        monkeypatch.setattr(server_mod, "_health_catalog", _boom_catalog)
        try:
            with TestClient(app) as client:
                server_mod._active_engines = {"wikipedia": _FakeEngine(EngineStatus.OK)}
                response = client.get("/health")
                assert response.status_code == 200
                data = response.json()
                assert data["status"] == "ok"
                record = data["engines"]["wikipedia"]
                assert record["configured"] is True
                assert record["status"] == "unknown"
                # The capability is unavailable, so auth readiness is an
                # explicit null — never a fabricated unknown/false (issue 190
                # review).
                assert record["auth_class"] is None
                assert record["auth_configured"] is None
        finally:
            server_mod._active_engines = original
            server_mod._health_config_cache = None
            server_mod._health_catalog_cache = None
            server_mod._health_catalog_engines = None

    def test_health_survives_capability_lookup_failure(self, monkeypatch: Any) -> None:
        """A single engine's capability lookup failure never 500s the probe."""
        import slopsearx.server as server_mod
        from slopsearx.server import app

        server_mod._health_config_cache = None
        server_mod._health_catalog_cache = None
        server_mod._health_catalog_engines = None
        original = dict(server_mod._active_engines)

        def _boom_get(self: Any, name: str) -> None:
            del self, name
            raise RuntimeError("capability lookup failed")

        monkeypatch.setattr(CapabilityCatalog, "get", _boom_get)
        try:
            with TestClient(app) as client:
                server_mod._active_engines = {"wikipedia": _FakeEngine(EngineStatus.OK)}
                response = client.get("/health")
                assert response.status_code == 200
                record = response.json()["engines"]["wikipedia"]
                # Degrades to a record derived from the running adapter alone.
                assert record["configured"] is True
        finally:
            server_mod._active_engines = original
            server_mod._health_config_cache = None
            server_mod._health_catalog_cache = None
            server_mod._health_catalog_engines = None


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
                    engine,
                    CapabilityCatalog(config=load_config(), adapters={"wikipedia": engine}).get("wikipedia"),
                )
                assert after == expected
        finally:
            server_mod._active_engines = original
            server_mod._router = original_router

    def test_health_surfaces_null_for_unmeasured_latency(self) -> None:
        """/health surfaces null, never 0.0, for an engine that never reports latency."""
        import slopsearx.server as server_mod
        from slopsearx.server import app

        original = dict(server_mod._active_engines)
        original_router = server_mod._router
        engine = _LatencylessEngine()
        engine.name = "wikipedia"
        try:
            with TestClient(app) as client:
                server_mod._active_engines = {"wikipedia": engine}
                server_mod._router = None

                client.get("/search", params={"q": "test", "engines": "wikipedia"})

                after = client.get("/health").json()["engines"]["wikipedia"]
                assert after["status"] == "ok"
                assert after["last_observed_latency_ms"] is None
                assert after["last_observed_result_count"] == 1
        finally:
            server_mod._active_engines = original
            server_mod._router = original_router
