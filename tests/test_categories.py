"""Tests for category support — adapter declaration, filtering, /config endpoint."""

from __future__ import annotations

from fastapi.testclient import TestClient

import engines  # noqa: F401
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)
from slopsearx.server import app


class TestAdapterCategories:
    """Category class attribute and merge logic."""

    def test_default_categories(self) -> None:
        """EngineAdapter defaults to ['general']."""

        class _TestEngine(EngineAdapter):
            name = "testcat"

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        engine = _TestEngine()
        assert engine.categories == ["general"]

    def test_subclass_overrides_categories(self) -> None:
        """Subclass can set categories class attribute."""

        class _TestEngine(EngineAdapter):
            name = "testcat2"
            categories = ["news", "science"]

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        engine = _TestEngine()
        assert engine.categories == ["news", "science"]

    def test_config_categories_override_self_declared(self) -> None:
        """Config categories replace self-declared categories."""

        class _TestEngine(EngineAdapter):
            name = "testcat3"
            categories = ["general", "images"]

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        engine = _TestEngine({"categories": ["news", "finance"]})
        assert engine.categories == ["news", "finance"]


class TestConfigEndpoint:
    """GET /config endpoint."""

    def test_returns_searxng_shape_and_additive_catalog(self) -> None:
        """Config exposes SearXNG fields and retains SlopSearX discovery data."""
        import slopsearx.server as server_mod

        original = dict(server_mod._active_engines)

        @register_engine
        class _ConfigTestEngine(EngineAdapter):
            name = "configtest"
            categories = ["general", "news", "science"]

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.OK)

        server_mod._active_engines = {"configtest": _ConfigTestEngine()}

        try:
            with TestClient(app) as client:
                response = client.get("/config")
                assert response.status_code == 200
                data = response.json()
                assert data["categories"] == ["general", "news", "science"]
                assert data["category_engines"] == {
                    "general": ["configtest"],
                    "news": ["configtest"],
                    "science": ["configtest"],
                }
                assert {
                    "autocomplete",
                    "default_locale",
                    "default_theme",
                    "engines",
                    "instance_name",
                    "locales",
                    "plugins",
                    "safe_search",
                    "version",
                } <= data.keys()
                assert data["instance_name"] == "SlopSearX"
                assert data["version"]
                assert len(data["engines"]) == 1
                engine = data["engines"][0]
                assert engine["categories"] == ["general", "news", "science"]
                assert engine["enabled"] is True
                assert engine["name"] == "configtest"
                assert engine["shortcut"] == "configtest"
                assert engine["display_name"] == "configtest"
                assert engine["type"] == "api"
                assert set(engine) >= {
                    "sensitive",
                    "supported_filters",
                    "enforced_filters",
                    "supported_result_types",
                    "supported_media_types",
                    "failure_classes",
                    "cost_class",
                    "auth",
                    "scope_hints",
                    "caveats",
                }
                assert "api_key" not in engine
                assert "secret" not in str(engine).lower()
        finally:
            server_mod._active_engines = original
            from slopsearx.adapter import _ENGINE_REGISTRY

            _ENGINE_REGISTRY.pop("configtest", None)


class TestCategoryFiltering:
    """Category filtering in /search."""

    def test_category_filter_excludes_non_matching(self) -> None:
        """Engines not in requested category are excluded — all return error."""
        import slopsearx.server as server_mod

        original = dict(server_mod._active_engines)

        @register_engine
        class _CatEngineA(EngineAdapter):
            name = "cata"
            categories = ["general", "news"]

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message="test")

        @register_engine
        class _CatEngineB(EngineAdapter):
            name = "catb"
            categories = ["science"]

            async def search(self, query, params=None):
                return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message="test")

        server_mod._active_engines = {"cata": _CatEngineA(), "catb": _CatEngineB()}

        try:
            with TestClient(app) as client:
                # Science category — only catb (cata filtered out)
                resp = client.get("/search", params={"q": "test", "categories": "science", "format": "json"})
                # 503 because catb returns ERROR (all engines unresponsive)
                assert resp.status_code == 503

                # News category — only cata (catb filtered out)
                resp = client.get("/search", params={"q": "test", "categories": "news", "format": "json"})
                assert resp.status_code == 503
        finally:
            server_mod._active_engines = original
            from slopsearx.adapter import _ENGINE_REGISTRY

            _ENGINE_REGISTRY.pop("cata", None)
            _ENGINE_REGISTRY.pop("catb", None)

    def test_engines_param_overrides_categories(self) -> None:
        """Explicit engines parameter wins over category filter."""
        import slopsearx.server as server_mod

        original = dict(server_mod._active_engines)

        @register_engine
        class _CatEngine(EngineAdapter):
            name = "catonly"
            categories = ["science"]

            async def search(self, query, params=None):
                return AdapterResponse(
                    results=[SearchResult(url="https://a.com", title="A", content="", engine="catonly")],
                    status=EngineStatus.OK,
                )

        server_mod._active_engines = {"catonly": _CatEngine()}

        try:
            with TestClient(app) as client:
                # Query with categories=nosuch — but engines=catonly overrides
                resp = client.get(
                    "/search",
                    params={"q": "test", "categories": "nosuch", "engines": "catonly", "format": "json"},
                )
                assert resp.status_code == 200
        finally:
            server_mod._active_engines = original
            from slopsearx.adapter import _ENGINE_REGISTRY

            _ENGINE_REGISTRY.pop("catonly", None)
