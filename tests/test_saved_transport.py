"""Saved-search lifecycle through production FastMCP transport/state injection."""

from __future__ import annotations

import asyncio
import json

from slopsearx.mcp.harness import FakeEngineSpec, InMemoryStore, make_fixture_http_app
from tests.test_mcp_harness import _serve, _session


def payload(result):
    return json.loads(result.content[0].text)


async def test_saved_search_transport_journey(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_SAVED_SEARCHES", "1")
    store = InMemoryStore()
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=2)], store=store, token="saved-token")
    async with _serve(app) as url, _session(url, "saved-token") as (client, _http):
        await client.initialize()

        async def call(name, **arguments):
            result = await client.call_tool(name, arguments)
            assert not result.isError, result
            value = payload(result)
            assert "error" not in value, value
            return value

        listed = await client.list_tools()
        assert {item.name for item in listed.tools} >= {
            "slopsearx_create_saved_search",
            "slopsearx_get_saved_search",
            "slopsearx_update_saved_search",
            "slopsearx_pause_saved_search",
            "slopsearx_delete_saved_search",
            "slopsearx_read_saved_search_reports",
        }
        created = await call(
            "slopsearx_create_saved_search",
            query="release changes",
            engines=["wikipedia"],
            interval_seconds=60,
            start_immediately=True,
            max_results=10,
            max_reports=3,
        )
        async with asyncio.timeout(5):
            reports = await call("slopsearx_read_saved_search_reports", search_id=created["search_id"])
            while not reports["reports"]:
                await asyncio.sleep(0.02)
                reports = await call("slopsearx_read_saved_search_reports", search_id=created["search_id"])
        assert reports["reports"][0]["status"] == "baseline_initialized"
        assert reports["reports"][0]["observation"]["cached"] is False
        current = await call("slopsearx_get_saved_search", search_id=created["search_id"])
        updated = await call(
            "slopsearx_update_saved_search",
            search_id=created["search_id"],
            expected_revision=current["revision"],
            query="new release changes",
        )
        paused = await call(
            "slopsearx_pause_saved_search",
            search_id=created["search_id"],
            expected_revision=updated["revision"],
            paused=True,
        )
        assert paused["paused"] is True
        resumed = await call(
            "slopsearx_pause_saved_search",
            search_id=created["search_id"],
            expected_revision=paused["revision"],
            paused=False,
        )
        deleted = await call(
            "slopsearx_delete_saved_search",
            search_id=created["search_id"],
            expected_revision=resumed["revision"],
        )
        assert deleted["state"] == "deleted"
        missing = payload(await client.call_tool("slopsearx_get_saved_search", {"search_id": created["search_id"]}))
        assert missing["error"]["code"] == "invalid_search_id"


async def test_saved_search_transport_rejects_boolean_integers(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_SAVED_SEARCHES", "1")
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia")])
    async with _serve(app) as url, _session(url) as (client, _http):
        await client.initialize()
        for field in ("interval_seconds", "retention_seconds", "max_results", "max_reports"):
            arguments = {"query": "q", "engines": ["wikipedia"], "interval_seconds": 60, field: True}
            result = await client.call_tool("slopsearx_create_saved_search", arguments)
            assert result.isError
