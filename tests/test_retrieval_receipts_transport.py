"""Production FastMCP transport journey for retrieval receipts."""

from __future__ import annotations

from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app
from tests.test_mcp_harness import _payload, _serve, _session


async def test_search_receipt_read_manifest_over_authenticated_transport(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_RETRIEVAL_RECEIPTS", "1")
    monkeypatch.setenv("MCP_AUTH_TOKEN", "receipt-token")
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=1)])
    async with _serve(app) as url, _session(url, "receipt-token") as (client, _http):
        await client.initialize()
        tools = {tool.name for tool in (await client.list_tools()).tools}
        assert {
            "slopsearx_submit_retrieval_receipt",
            "slopsearx_read_retrieval_receipts",
            "slopsearx_export_research_manifest",
        } <= tools
        search = _payload(await client.call_tool("slopsearx_search", {"query": "transport receipt"}))
        identifier = search["results"][0]["result_id"]
        submitted = _payload(
            await client.call_tool(
                "slopsearx_submit_retrieval_receipt",
                {
                    "result_id": identifier,
                    "retriever": "transport-reader",
                    "idempotency_key": "transport-1",
                    "status": "succeeded",
                    "captured_at": "2026-09-11T04:00:00Z",
                    "capture_ref": "capture://transport/1",
                },
            )
        )
        assert submitted["state"] == "created"
        read = _payload(await client.call_tool("slopsearx_read_retrieval_receipts", {"result_id": identifier}))
        assert read["total"] == 1
        manifest = _payload(await client.call_tool("slopsearx_export_research_manifest", {"result_ids": [identifier]}))
        assert manifest["items"][0]["receipts"][0]["receipt_id"] == submitted["receipt"]["receipt_id"]
