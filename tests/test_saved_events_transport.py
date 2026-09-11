"""Saved-search event read/ack journey through the production FastMCP registry."""

from __future__ import annotations

import asyncio
import json

from slopsearx.mcp.harness import FakeEngineSpec, InMemoryStore, make_fixture_http_app
from tests.test_mcp_harness import _serve, _session


def _payload(result):
    return json.loads(result.content[0].text)


async def test_saved_event_transport_read_restart_and_ack(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_SAVED_SEARCHES", "1")
    monkeypatch.setenv("MCP_GRANT_SAVED_SEARCH_EVENTS", "1")
    store = InMemoryStore()
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=2)], store=store, token="event-token")
    async with _serve(app) as url, _session(url, "event-token") as (client, _http):
        await client.initialize()
        listed = {item.name for item in (await client.list_tools()).tools}
        assert {"slopsearx_read_saved_search_events", "slopsearx_ack_saved_search_events"} <= listed
        created_result = await client.call_tool(
            "slopsearx_create_saved_search",
            {
                "query": "release changes",
                "engines": ["wikipedia"],
                "interval_seconds": 60,
                "start_immediately": True,
            },
        )
        assert not created_result.isError
        async with asyncio.timeout(5):
            while True:
                read = _payload(
                    await client.call_tool(
                        "slopsearx_read_saved_search_events",
                        {"consumer_id": "transport-agent", "limit": 1},
                    )
                )
                if read.get("events"):
                    break
                await asyncio.sleep(0.02)
        assert read["events"][0]["saved_search_id"] == _payload(created_result)["search_id"]
        assert read["acknowledged_cursor"] == "0-0"

        ack = _payload(
            await client.call_tool(
                "slopsearx_ack_saved_search_events",
                {"consumer_id": "transport-agent", "cursor": read["next_cursor"]},
            )
        )
        assert ack["acknowledged_cursor"] == read["next_cursor"]
        restarted = _payload(
            await client.call_tool("slopsearx_read_saved_search_events", {"consumer_id": "transport-agent"})
        )
        assert restarted["acknowledged_cursor"] == read["next_cursor"]
        assert restarted["events"] == []


async def test_saved_event_transport_requires_separate_grant(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_SAVED_SEARCHES", "1")
    monkeypatch.delenv("MCP_GRANT_SAVED_SEARCH_EVENTS", raising=False)
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia")])
    async with _serve(app) as url, _session(url) as (client, _http):
        await client.initialize()
        value = _payload(
            await client.call_tool("slopsearx_read_saved_search_events", {"consumer_id": "transport-agent"})
        )
        assert value["error"] == {
            "code": "tool_disabled",
            "message": "saved-search events require the event grant (MCP_GRANT_SAVED_SEARCH_EVENTS=1)",
        }
