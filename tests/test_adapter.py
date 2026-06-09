"""Tests for EngineAdapter, @register_engine, and data types."""

from __future__ import annotations

import pytest

import engines as _engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    discover_engines,
    list_engines,
    register_engine,
)


class TestSearchResult:
    """SearchResult dataclass contract."""

    def test_minimal_creation(self) -> None:
        result = SearchResult(url="https://example.com", title="Test", content="content", engine="test")
        assert result.url == "https://example.com"
        assert result.title == "Test"
        assert result.content == "content"
        assert result.engine == "test"
        assert result.engines == set()
        assert result.score == 0.0

    def test_engines_set_add(self) -> None:
        result = SearchResult(url="https://example.com", title="T", content="C", engine="a")
        result.engines.add("b")
        assert result.engines == {"b"}


class TestEngineStatus:
    """EngineStatus enum values."""

    def test_enum_values(self) -> None:
        assert EngineStatus.OK.value == "ok"
        assert EngineStatus.RATE_LIMITED.value == "rate_limited"
        assert EngineStatus.BLOCKED.value == "blocked"
        assert EngineStatus.ERROR.value == "error"
        assert EngineStatus.TIMEOUT.value == "timeout"

    def test_membership(self) -> None:
        assert EngineStatus.OK in EngineStatus


class TestAdapterResponse:
    """AdapterResponse dataclass contract."""

    def test_ok_response(self) -> None:
        resp = AdapterResponse(results=[], status=EngineStatus.OK)
        assert resp.status == EngineStatus.OK
        assert resp.results == []
        assert resp.error_message is None
        assert resp.latency_ms == 0.0

    def test_error_response(self) -> None:
        resp = AdapterResponse(
            results=[],
            status=EngineStatus.ERROR,
            error_message="something broke",
            latency_ms=150.0,
        )
        assert resp.status == EngineStatus.ERROR
        assert resp.error_message == "something broke"
        assert resp.latency_ms == 150.0


# ---------------------------------------------------------------------------
# Registration tests
# ---------------------------------------------------------------------------


class TestEngineRegistry:

    def test_register_valid_adapter(self) -> None:
        """A valid adapter should be registered and discoverable."""
        engines = list_engines()
        # Our 4 engines should be registered
        assert "brave" in engines
        assert "duckduckgo" in engines
        assert "google" in engines
        assert "wikipedia" in engines

    def test_register_requires_name(self) -> None:
        """An adapter without a name should fail."""
        with pytest.raises(AssertionError):

            @register_engine
            class _Nameless(EngineAdapter):
                name = ""

                async def search(self, query, params=None):
                    return AdapterResponse(results=[], status=EngineStatus.OK)

    def test_register_requires_adapter_subclass(self) -> None:
        """A non-adapter class should fail."""
        with pytest.raises(AssertionError):

            @register_engine
            class _NotAnAdapter:  # type: ignore[misc]
                name = "nope"

    def test_discover_enabled(self) -> None:
        """discover_engines() should instantiate all enabled engines."""
        configs = {
            "brave": {"enabled": True, "api_key": "test-key"},
            "wikipedia": {"enabled": True},
        }
        instances = discover_engines(configs)
        assert "brave" in instances
        assert "wikipedia" in instances

    def test_discover_disabled_engine(self) -> None:
        """An engine marked disabled should not be instantiated."""
        configs = {"brave": {"enabled": False}}
        instances = discover_engines(configs)
        assert "brave" not in instances

    def test_discover_empty_config(self) -> None:
        """When config is None, all engines should be instantiated with empty config."""
        instances = discover_engines()
        assert "brave" in instances
        assert "wikipedia" in instances


# ---------------------------------------------------------------------------
# EngineAdapter base class tests
# ---------------------------------------------------------------------------


class TestEngineAdapter:
    """EngineAdapter ABC contract tests."""

    def test_adapter_requires_search(self) -> None:
        """A class that doesn't implement search() can't be instantiated."""
        with pytest.raises(TypeError):
            EngineAdapter()  # type: ignore[abstract]

    def test_default_health_ok(self) -> None:
        """health() should return OK when search returns OK."""

        @register_engine
        class _HealthyEngine(EngineAdapter):
            name = "healthy-test"
            display_name = "Healthy Test"
            env_prefix = "ENGINE_HEALTHY"

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        import slopsearx.adapter as _adapter
        assert _HealthyEngine.name in _adapter._ENGINE_REGISTRY

    def test_adapter_config_passthrough(self) -> None:
        """Adapter constructor should store config."""

        @register_engine
        class _CfgEngine(EngineAdapter):
            name = "cfg-test"
            display_name = "Cfg Test"
            env_prefix = "ENGINE_CFG"

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        inst = _CfgEngine({"api_key": "secret"})
        assert inst.config["api_key"] == "secret"
