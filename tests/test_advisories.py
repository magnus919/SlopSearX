"""Factual advisories: scope relevance, policy redaction and operator ownership."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

import pytest

import engines  # noqa: F401
from slopsearx.advisories import search_advisories
from slopsearx.capabilities import CapabilityCatalog
from slopsearx.config import load_config
from slopsearx.router import QueryRouter
from slopsearx.service import AppContext, ScopeDecision, SearchRequest


def catalog_for(**changes: dict[str, Any]) -> CapabilityCatalog:
    catalog = CapabilityCatalog(config=load_config())
    catalog._by_name = {name: replace(catalog.get(name), **values) for name, values in changes.items()}
    return catalog


@pytest.mark.parametrize("kind", ["engines", "categories", "topic", "fallback", "media"])
@pytest.mark.parametrize("disabled", [False, True])
def test_relevant_unavailable_sources_are_operator_owned(kind: str, disabled: bool) -> None:
    catalog = catalog_for(brave={"enabled": not disabled, "auth_class": "required", "auth_configured": False})
    ctx = AppContext(active_engines={}, catalog=catalog, tier1_engines={"brave"})
    request = SearchRequest(query="needle")
    if kind == "engines":
        request.engines = ["brave"]
    elif kind == "categories":
        request.categories = ["general"]
    elif kind == "topic":
        ctx.router = QueryRouter({"topics": [{"keywords": ["needle"], "engines": ["brave"]}]})
    elif kind == "media":
        request.media_type = "images"
        catalog._by_name["brave"] = replace(catalog.get("brave"), supported_media_types=["images"])
    notes = search_advisories(request, ScopeDecision(), ctx)
    source = next(n for n in notes if n.get("engine") == "brave")
    assert source["reason"] == ("engine_disabled" if disabled else "credentials_missing")
    assert source["action"]["actor"] == "operator"
    assert source["expected_quality_gain"] == "unmeasured"
    assert "needle" not in str(notes)


@pytest.mark.parametrize("hidden", ["capability", "context", "policy"])
def test_sensitive_sources_never_advertised(hidden: str) -> None:
    catalog = catalog_for(brave={"enabled": False, "sensitive": hidden == "capability"})
    ctx = AppContext(active_engines={}, catalog=catalog, sensitive_engines={"brave"} if hidden == "context" else set())
    notes = search_advisories(
        SearchRequest(query="needle", engines=["brave"]),
        ScopeDecision(),
        ctx,
        sensitive_engines={"brave"} if hidden == "policy" else set(),
    )
    assert notes == []


def test_no_unrelated_or_credentialed_source_advisory() -> None:
    catalog = catalog_for(
        brave={"enabled": False},
        wikipedia={"enabled": True, "auth_class": "none"},
    )
    ctx = AppContext(active_engines={}, catalog=catalog)
    assert search_advisories(SearchRequest(query="x", engines=["wikipedia"]), ScopeDecision(), ctx) == []
    catalog._by_name["brave"] = replace(catalog.get("brave"), enabled=True, auth_class="required", auth_configured=True)
    assert search_advisories(SearchRequest(query="x", engines=["brave"]), ScopeDecision(), ctx) == []


def test_jev_only_on_automatic_scopes_and_available_state() -> None:
    ctx = AppContext(active_engines={}, tier1_engines=set())
    request = SearchRequest(query="x")
    notes = search_advisories(request, ScopeDecision(), ctx)
    assert len(notes) == 1
    assert notes[0]["code"] == "optional_routing_unavailable"
    request.engines = ["wikipedia"]
    assert search_advisories(request, ScopeDecision(), ctx) == []
    request.engines = None
    assert search_advisories(request, ScopeDecision(jev_added_engines=["arxiv"]), ctx) == []


def test_bounded_sorted_and_derived_from_current_catalog() -> None:
    catalog = catalog_for(brave={"enabled": False}, wikipedia={"enabled": False}, arxiv={"enabled": False})
    ctx = AppContext(active_engines={}, catalog=catalog, tier1_engines={"wikipedia", "brave", "arxiv"})
    request = SearchRequest(query="x")
    cached_scope = ScopeDecision()
    first = search_advisories(request, cached_scope, ctx)
    assert [n.get("engine") for n in first] == [None, "arxiv", "brave"]
    assert len(first) == 3
    catalog._by_name["arxiv"] = replace(catalog.get("arxiv"), enabled=True, auth_class="none")
    second = search_advisories(request, cached_scope, ctx)
    assert [n.get("engine") for n in second] == [None, "brave", "wikipedia"]
    assert not hasattr(cached_scope, "advisories")


def test_configured_jev_does_not_emit_missing_routing_note() -> None:
    from slopsearx.jev import JevSpecialistRouter

    ctx = AppContext(active_engines={}, jev_router=JevSpecialistRouter("fixture-no-call"))
    assert search_advisories(SearchRequest(query="x"), ScopeDecision(), ctx) == []


@pytest.mark.parametrize("count,expected", [(0, False), (5, False), (6, True), (40, True), (100, True)])
def test_reranking_threshold_and_attributed_evidence(count: int, expected: bool) -> None:
    ctx = AppContext(active_engines={})
    notes = search_advisories(SearchRequest(query="x", engines=["wikipedia"]), ScopeDecision(), ctx, result_count=count)
    assert bool(notes) is expected
    if expected:
        assert notes[0]["quality_evidence"] == "operator_reported_production"
        assert notes[0]["expected_quality_gain"] == "unmeasured"
        assert notes[0]["action"]["actor"] == "operator"


def test_reranking_suppressed_for_sensitive_scope_and_configured_provider() -> None:
    from slopsearx.rerank import JevReranker

    ctx = AppContext(active_engines={}, sensitive_engines={"hibp"})
    assert search_advisories(SearchRequest(query="x", engines=["hibp"]), ScopeDecision(), ctx, result_count=40) == []
    ctx.rerank_provider = JevReranker("fixture-no-call")
    assert (
        search_advisories(SearchRequest(query="x", engines=["wikipedia"]), ScopeDecision(), ctx, result_count=40) == []
    )


def test_portal_advisories_collapsed_and_escaped() -> None:
    from slopsearx.formatter import format_html, format_yaml_markdown

    notes = [{"message": "<script>operator</script>", "code": "test"}]
    html = format_html([], "x", meta={"advisories": notes})
    assert '<details class="notice"><summary>Source availability</summary>' in html
    assert "&lt;script&gt;operator&lt;/script&gt;" in html
    assert "<script>operator</script>" not in html
    assert "advisories:" in format_yaml_markdown([], "x", meta={"advisories": notes})


def test_mcp_uses_full_count_before_presentation_slice() -> None:
    import asyncio

    from slopsearx.mcp import tools
    from slopsearx.mcp.state import set_state
    from tests.test_mcp_tools import _build_state

    state = _build_state(["wikipedia"])
    state.ctx.active_engines["wikipedia"]._count = 6
    set_state(state)
    try:
        body = asyncio.run(tools.slopsearx_search("x", engines=["wikipedia"], max_results=1))
        assert len(body["results"]) == 1
        note = body["meta"]["advisories"][0]
        assert note["capability"] == "jev_reranking"
        assert note["quality_evidence"] == "operator_reported_production"
    finally:
        set_state(None)


def test_cached_mcp_response_uses_current_availability() -> None:
    import asyncio

    from slopsearx.mcp import tools
    from slopsearx.mcp.state import set_state
    from tests.test_mcp_tools import _build_state

    state = _build_state(["wikipedia"])
    state.catalog = catalog_for(
        wikipedia={"enabled": True, "auth_class": "none", "categories": ["general"]},
        brave={"enabled": True, "auth_class": "required", "auth_configured": False, "categories": ["general"]},
    )
    state.ctx.catalog = state.catalog
    set_state(state)
    try:
        first = asyncio.run(tools.slopsearx_search("cached", categories=["general"]))
        assert first["meta"]["advisories"][0]["reason"] == "credentials_missing"
        state.catalog._by_name["brave"] = replace(state.catalog.get("brave"), auth_configured=True)
        second = asyncio.run(tools.slopsearx_search("cached", categories=["general"]))
        assert second["meta"]["cached"] is True
        assert "advisories" not in second["meta"]
        assert state.ctx.active_engines["wikipedia"].calls == 1
    finally:
        set_state(None)


@pytest.mark.parametrize("explicit", [False, True])
def test_media_intersects_requested_source_scope(explicit: bool) -> None:
    catalog = catalog_for(brave={"enabled": False, "categories": ["general"], "supported_media_types": []})
    ctx = AppContext(active_engines={}, catalog=catalog)
    request = SearchRequest(
        query="x",
        media_type="images",
        engines=["brave"] if explicit else None,
        categories=None if explicit else ["general"],
    )
    assert search_advisories(request, ScopeDecision(), ctx) == []


def test_http_uses_operator_sensitive_policy(monkeypatch: pytest.MonkeyPatch) -> None:
    from fastapi.testclient import TestClient

    from slopsearx import server
    from slopsearx.capabilities import MCPPolicy
    from tests.test_mcp_tools import _MockEngine

    catalog = catalog_for(
        brave={"enabled": False, "categories": ["general"], "sensitive": False},
        wikipedia={"enabled": True, "categories": ["general"], "auth_class": "none"},
    )
    client = TestClient(server.app)
    monkeypatch.setattr(server, "_active_engines", {"wikipedia": _MockEngine("wikipedia", count=1)})
    monkeypatch.setattr(server, "_portal_policy", MCPPolicy(sensitive_engines={"brave"}))
    monkeypatch.setattr(server, "_health_catalog", lambda: catalog)
    monkeypatch.setattr(server, "_cache", None)
    monkeypatch.setattr(server, "_rerank_provider", None)
    monkeypatch.setattr(server, "_jev_router", None)
    monkeypatch.setattr(server, "_router", None)
    monkeypatch.setattr(server, "_rate_limiter", None)
    monkeypatch.setattr(server, "_client_rate_window", None)
    body = client.get("/search", params={"q": "x", "categories": "general", "format": "json"}).json()
    client.close()
    assert body.get("meta", {}).get("advisories", []) == []
