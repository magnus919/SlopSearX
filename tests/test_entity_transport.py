"""Entity view exercised through authenticated production MCP SDK transport."""

import httpx

from slopsearx.adapter import AdapterResponse, EngineStatus, SearchResult
from slopsearx.mcp.harness import FakeEngine, FakeEngineSpec, make_fixture_http_app
from slopsearx.payload import build_payload
from tests.test_mcp_harness import _payload, _serve, _session


async def test_entity_view_preserves_cached_flat_results(monkeypatch):
    calls = []

    async def search(engine, query, params=None):
        calls.append(query)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://example.com/{index}",
                    title="lead",
                    content="snippet",
                    engine="nvd",
                    payload=build_payload("security", "vulnerability", {"cve_id": cve}, engine="nvd"),
                )
                for index, cve in enumerate(["CVE-2024-12345", "CVE-2024-99999", "CVE-2024-12345"])
            ],
            status=EngineStatus.OK,
        )

    monkeypatch.setattr(FakeEngine, "search", search)
    app = make_fixture_http_app([FakeEngineSpec(name="nvd")], token="test-token")
    async with _serve(app) as url:
        async with httpx.AsyncClient() as unauthenticated:
            assert (await unauthenticated.post(url, json={})).status_code == 401
        async with _session(url, "test-token") as (session, _client):
            await session.initialize()
            discovered = await session.list_tools()
            tool = next(tool for tool in discovered.tools if tool.name == "slopsearx_read_entities")
            assert tool.inputSchema["required"] == ["cursor"]
            query = {"query": "CVE-2024-12345", "engines": ["nvd"], "max_results": 1, "include": ["results"]}
            first = _payload(await session.call_tool("slopsearx_search", query))
            cursor = first["meta"]["cursor"]
            before = _payload(await session.call_tool("slopsearx_read_results", {"cursor": cursor}))
            grouped = _payload(await session.call_tool("slopsearx_read_entities", {"cursor": cursor, "max_results": 1}))
            assert grouped["meta"]["total_results"] == 3
            assert grouped["meta"]["total_entities"] == 2
            assert grouped["entities"][0]["result_ids"] == [f"{cursor}:0", f"{cursor}:2"]
            for result_id in grouped["entities"][0]["result_ids"]:
                record = _payload(await session.call_tool("slopsearx_read_result", {"result_id": result_id}))
                assert record["result_id"] == result_id
                assert record["payload"]["data"]["cve_id"] == "CVE-2024-12345"
            second = _payload(
                await session.call_tool("slopsearx_read_entities", {"cursor": cursor, "page": 2, "max_results": 1})
            )
            assert second["entities"][0]["identifier"]["cve_id"] == "CVE-2024-99999"
            assert not second["meta"]["has_more"]
            assert _payload(await session.call_tool("slopsearx_read_results", {"cursor": cursor})) == before
            repeated = _payload(await session.call_tool("slopsearx_search", query))
            assert len(calls) == 1
            for key in ("url", "title", "snippet", "score", "tier", "engines"):
                assert repeated["results"][0].get(key) == first["results"][0].get(key)
            after = _payload(await session.call_tool("slopsearx_read_entities", {"cursor": repeated["meta"]["cursor"]}))
            assert after["entities"][0]["entity_id"] == grouped["entities"][0]["entity_id"]
