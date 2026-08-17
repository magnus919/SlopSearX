"""Transport-level tests for the MCP fixture harness.

These drive the **real** MCP server over streamable HTTP (uvicorn on an
ephemeral loopback port + the Python ``mcp`` SDK client) with injected fake
engines and an in-memory cache/snapshot/job store. They prove the harness
supports:

- first-visit reachability (tools/list -> capabilities -> scope -> search),
- authenticated and unauthenticated transport flows, and
- deterministic search envelopes (bounded ``max_results``, per-request
  ``include``/``engine_outcomes``, and the structured filter-enforcement
  report) with no live network and no Valkey.

Every assertion is deterministic: fake engines never touch the network and
the in-memory store replaces Valkey.
"""

from __future__ import annotations

import asyncio
import json
from contextlib import asynccontextmanager
from typing import Any, AsyncIterator

import httpx
import pytest
import uvicorn
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client

from slopsearx.adapter import EngineStatus
from slopsearx.mcp import harness as h
from slopsearx.mcp.harness import (
    FakeEngine,
    FakeEngineSpec,
    InMemoryStore,
    build_fake_engines,
    build_fixture_context,
    make_fixture_http_app,
)
from slopsearx.service import AppContext

_FIXTURE_SPECS = [FakeEngineSpec(name="wikipedia", count=3), FakeEngineSpec(name="brave", count=2)]


@asynccontextmanager
async def _serve(app: Any, token: str = "") -> AsyncIterator[str]:
    """Run an ASGI app on an ephemeral loopback port and yield its /mcp URL."""
    del token
    config = uvicorn.Config(app, host="127.0.0.1", port=0, log_level="error")
    server = uvicorn.Server(config)
    task = asyncio.create_task(server.serve())
    try:
        while not server.started:
            await asyncio.sleep(0.02)
        port = server.servers[0].sockets[0].getsockname()[1]
        yield f"http://127.0.0.1:{port}/mcp"
    finally:
        server.should_exit = True
        await task


@asynccontextmanager
async def _session(url: str, token: str = "") -> AsyncIterator[tuple[ClientSession, httpx.AsyncClient]]:
    """Open a streamable-HTTP MCP client session against ``url``."""
    headers = {"Authorization": f"Bearer {token}"} if token else {}
    async with httpx.AsyncClient(headers=headers) as client:
        async with streamable_http_client(url, http_client=client) as (read, write, _get_session_id):
            async with ClientSession(read, write) as session:
                yield session, client


def _payload(result: Any) -> dict[str, Any]:
    """Return the JSON payload of an MCP tool-call result."""
    return json.loads(result.content[0].text)


# ---------------------------------------------------------------------------
# Unit-level: the in-memory store and fake engines
# ---------------------------------------------------------------------------


class TestInMemoryStore:
    async def test_get_set_round_trip(self) -> None:
        store = InMemoryStore()
        assert store.is_connected is True
        await store.set("k", {"a": 1}, ttl=120)
        assert await store.get("k") == {"a": 1}
        assert store.set_ttls == [120]

    async def test_missing_key_returns_none(self) -> None:
        store = InMemoryStore()
        assert await store.get("nope") is None

    async def test_close_is_noop(self) -> None:
        await InMemoryStore().close()


class TestFakeEngine:
    async def test_deterministic_ok_results(self) -> None:
        engine = FakeEngine(FakeEngineSpec(name="wikipedia", count=3))
        resp = await engine.search("q")
        assert len(resp.results) == 3
        assert resp.status.value == "ok"
        assert resp.results[0].engines == {"wikipedia"}
        assert resp.results[0].url == "https://wikipedia0.example"
        # Second call is identical (deterministic).
        again = await engine.search("q")
        assert [r.url for r in again.results] == [r.url for r in resp.results]

    async def test_failure_status_no_network(self) -> None:
        engine = FakeEngine(FakeEngineSpec(name="brave", status=EngineStatus.RATE_LIMITED))
        resp = await engine.search("q")
        assert resp.results == []
        assert resp.status == EngineStatus.RATE_LIMITED

    async def test_invocation_count_tracks(self) -> None:
        engine = FakeEngine(FakeEngineSpec(name="brave"))
        await engine.search("q")
        await engine.search("q")
        assert engine.calls == 2


class TestBuildFixtureContext:
    def test_injects_fake_engines_and_store(self) -> None:
        ctx = build_fixture_context(_FIXTURE_SPECS)
        assert isinstance(ctx, AppContext)
        assert set(ctx.active_engines) == {"wikipedia", "brave"}
        assert ctx.cache is not None
        assert isinstance(ctx.cache, InMemoryStore)
        assert ctx.cache.is_connected is True

    def test_default_engines_and_sensitive_from_policy(self) -> None:
        ctx = build_fixture_context()
        assert set(ctx.active_engines) == set(h.DEFAULT_FAKE_ENGINES)
        # Sensitive engines default to the effective policy set so the
        # shared policy gate behaves like the production server.
        assert "hibp" in ctx.sensitive_engines

    def test_build_fake_engines_keys_by_name(self) -> None:
        engines = build_fake_engines(_FIXTURE_SPECS)
        assert set(engines) == {"wikipedia", "brave"}
        assert isinstance(engines["wikipedia"], FakeEngine)


# ---------------------------------------------------------------------------
# Transport-level: real MCP server over streamable HTTP
# ---------------------------------------------------------------------------


class TestFirstVisitReachability:
    async def test_discovery_to_search_chain(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                tools = await session.list_tools()
                names = {t.name for t in tools.tools}
                # The four tools an agent needs to reach a result.
                for required in (
                    "slopsearx_list_capabilities",
                    "slopsearx_explain_search_scope",
                    "slopsearx_search",
                    "slopsearx_read_result",
                ):
                    assert required in names

                caps = await session.call_tool("slopsearx_list_capabilities", {})
                caps_data = _payload(caps)
                assert "engines" in caps_data
                assert isinstance(caps_data["count"], int)

                scope = await session.call_tool("slopsearx_explain_search_scope", {"query": "hello", "intent": "auto"})
                scope_data = _payload(scope)
                assert "selected_engines" in scope_data

                search = await session.call_tool("slopsearx_search", {"query": "hello world"})
                data = _payload(search)
                assert "results" in data
                assert "meta" in data

    async def test_server_reports_full_tool_surface(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                tools = await session.list_tools()
                # The harness serves the same 13-tool surface as production.
                assert len(tools.tools) == 15


class TestDeterministicSearchEnvelope:
    async def test_search_returns_deterministic_results_and_cursor(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                res = await session.call_tool("slopsearx_search", {"query": "hello world"})
                data = _payload(res)
                assert "query" in data
                # The fixture deployment has no Brave key configured, so
                # automatic routing excludes the keyless brave fake with a
                # machine-readable reason (issue 192) while explicit
                # targeted searches can still reach it.
                assert data["scope"]["selected_engines"] == ["wikipedia"]
                excluded = {e["engine"]: e for e in data["scope"]["excluded_engines"]}
                assert excluded["brave"]["stage"] == "auth"
                assert "credentials" in excluded["brave"]["reason"]
                assert data["scope"]["routing"]["fallback"] is False
                assert data["meta"]["cursor"]
                assert len(data["results"]) == 3  # 3 deterministic wikipedia results

    async def test_max_results_is_bounded_per_request(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                one = await session.call_tool("slopsearx_search", {"query": "q", "max_results": 1})
                assert len(_payload(one)["results"]) == 1

    async def test_include_drives_engine_outcomes_per_request(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                with_status = await session.call_tool(
                    "slopsearx_search", {"query": "q", "include": ["results", "engine_status"]}
                )
                without_status = await session.call_tool("slopsearx_search", {"query": "q", "include": ["results"]})
                assert _payload(with_status)["engine_outcomes"]
                assert _payload(without_status)["engine_outcomes"] == []

    async def test_enforcement_report_is_structured(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                res = await session.call_tool(
                    "slopsearx_search", {"query": "q", "language": "de", "time_range": "week", "safesearch": "moderate"}
                )
                data = _payload(res)
                enforcement = data["enforcement"]
                assert set(enforcement) == {"language", "time_range", "safesearch"}
                for entry in enforcement.values():
                    assert set(entry) == {"requested", "status", "reason", "enforced_by"}
                    assert entry["status"] in ("enforced", "partially_enforced", "unsupported", "rejected")
                    assert entry["reason"]
                assert enforcement["language"]["status"] == "unsupported"
                assert enforcement["time_range"]["status"] == "unsupported"
                assert enforcement["safesearch"]["status"] == "unsupported"

    async def test_default_enforcement_shape(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS)
        async with _serve(app) as url:
            async with _session(url) as (session, _client):
                await session.initialize()
                res = await session.call_tool("slopsearx_search", {"query": "q"})
                data = _payload(res)
                assert "enforcement" in data


class TestAuthenticatedTransport:
    async def test_unauthenticated_request_is_rejected(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS, token="s3cret")
        async with _serve(app) as url:
            async with httpx.AsyncClient() as client:
                resp = await client.post(url, json={"jsonrpc": "2.0", "method": "tools/list", "id": 1})
                assert resp.status_code == 401

    async def test_authenticated_session_succeeds(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS, token="s3cret")
        async with _serve(app) as url:
            async with _session(url, token="s3cret") as (session, _client):
                await session.initialize()
                res = await session.call_tool("slopsearx_search", {"query": "hello"})
                assert "results" in _payload(res)
                tools = await session.list_tools()
                assert len(tools.tools) == 15

    async def test_wrong_token_is_rejected(self) -> None:
        app = make_fixture_http_app(_FIXTURE_SPECS, token="s3cret")
        async with _serve(app) as url:
            async with httpx.AsyncClient() as client:
                resp = await client.post(
                    url,
                    headers={"Authorization": "Bearer wrong"},
                    json={"jsonrpc": "2.0", "method": "tools/list", "id": 1},
                )
                assert resp.status_code == 401


class TestCli:
    def test_cli_parses_help(self) -> None:
        with pytest.raises(SystemExit):
            h.main(["--help"])
