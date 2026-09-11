"""Supported composition transitions over the production FastMCP transport."""

from __future__ import annotations

import asyncio

from slopsearx.config import EngineEntry
from slopsearx.mcp import harness as h
from slopsearx.mcp.harness import FakeEngineSpec, InMemoryStore, make_fixture_http_app
from tests.test_mcp_harness import _payload, _serve, _session


async def test_supported_composition_matrix_over_authenticated_transport(monkeypatch) -> None:
    for grant in (
        "MCP_GRANT_RESEARCH",
        "MCP_GRANT_RETRIEVAL_RECEIPTS",
        "MCP_GRANT_SAVED_SEARCHES",
        "MCP_GRANT_STAGED_SEARCH",
        "MCP_GRANT_DEPENDENCY_DOSSIER",
        "MCP_GRANT_SECURITY",
    ):
        monkeypatch.setenv(grant, "1")
    specs = [
        FakeEngineSpec(name="wikipedia", count=2),
        FakeEngineSpec(name="pypi", count=1, categories=["packages"]),
        FakeEngineSpec(name="github", count=1, categories=["it"]),
        FakeEngineSpec(name="nvd", count=1, categories=["security"]),
    ]
    config = h.fixture_config()
    config.engines.update(
        {
            "pypi": EngineEntry(api_key=""),
            "github": EngineEntry(api_key="fixture-key"),
            "nvd": EngineEntry(api_key=""),
        }
    )
    store = InMemoryStore()
    app = make_fixture_http_app(specs, store=store, token="composition-token", config=config)
    async with _serve(app) as url, _session(url, "composition-token") as (client, _http):
        await client.initialize()

        async def call(name: str, **arguments):
            response = await client.call_tool(name, arguments)
            assert not response.isError, response
            value = _payload(response)
            assert "error" not in value, value
            return value

        search = await call("slopsearx_search", query="source evidence", engines=["wikipedia"])
        snapshot = search["meta"]["artifact"]
        result = search["results"][0]["artifact"]
        research = await call(
            "slopsearx_start_research",
            question="continue evidence",
            initial_plan=[{"query": "explicit research plan", "engines": ["wikipedia"]}],
            max_queries=1,
            max_attempts=1,
            source=snapshot,
        )
        async with asyncio.timeout(5):
            while research["state"] in {"queued", "running"}:
                await asyncio.sleep(0.02)
                research = await call("slopsearx_get_job", job_id=research["job_id"])
        attempt = research["queries"][0]["attempts"][0]["artifact"]
        for source in (research["artifact"], attempt):
            manifest = await call("slopsearx_export_research_manifest", source=source)
            assert manifest["source"]["artifact"] == source

        for index, source in enumerate((snapshot, result)):
            dossier = await call(
                "slopsearx_start_dependency_dossier",
                ecosystem="pypi",
                package="requests",
                idempotency_key=f"transport-dossier-{index}",
                source=source,
            )
            assert dossier["source"]["artifact"] == source

        package_search = await call("slopsearx_search", query="entity seed", engines=["pypi"])
        cursor = package_search["meta"]["cursor"]
        stored = store._data[f"mcp:snapshot:default:{cursor}"]
        stored["results"][0]["payload"] = {
            "schema_version": 1,
            "domain": "packages",
            "type": "package",
            "data": {"name": "requests", "version": "2.32.5", "ecosystem": "pypi"},
            "provenance": {
                "engine": "pypi",
                "adapter_fields": ["name", "version", "ecosystem"],
                "inferred_fields": [],
            },
        }
        entities = await call("slopsearx_read_entities", cursor=cursor)
        entity = entities["entities"][0]["artifact"]
        entity_dossier = await call(
            "slopsearx_start_dependency_dossier",
            ecosystem="pypi",
            package="requests",
            idempotency_key="transport-dossier-entity",
            source=entity,
        )
        assert entity_dossier["source"]["artifact"] == entity

        saved = await call(
            "slopsearx_create_saved_search",
            query="saved source query",
            engines=["wikipedia"],
            interval_seconds=60,
            start_immediately=True,
        )
        async with asyncio.timeout(5):
            reports = await call("slopsearx_read_saved_search_reports", search_id=saved["search_id"])
            while not reports["reports"]:
                await asyncio.sleep(0.02)
                reports = await call("slopsearx_read_saved_search_reports", search_id=saved["search_id"])
        saved_report = reports["reports"][0]["artifact"]
        saved_research = await call(
            "slopsearx_start_research",
            question="continue saved observation",
            initial_plan=[{"query": "new objective", "engines": ["wikipedia"]}],
            max_queries=1,
            max_attempts=1,
            source=saved_report,
        )
        assert saved_research["source"]["artifact"] == saved_report
        staged = await call(
            "slopsearx_search_staged",
            objectives={"deadline_ms": 5000, "max_engine_calls": 1},
            initial_scope={"engines": ["wikipedia"]},
            idempotency_key="transport-saved-staged",
            source=saved_report,
        )
        async with asyncio.timeout(5):
            while staged["state"] in {"queued", "running"}:
                await asyncio.sleep(0.02)
                staged = await call("slopsearx_get_staged_search", operation_id=staged["operation_id"])
        staged_manifest = await call("slopsearx_export_research_manifest", source=staged["artifact"])
        staged_research = await call(
            "slopsearx_start_research",
            question="continue staged evidence",
            initial_plan=[{"query": "staged follow-up", "engines": ["wikipedia"]}],
            max_queries=1,
            max_attempts=1,
            source=staged["artifact"],
        )
        assert staged_manifest["source"]["artifact"] == staged["artifact"]
        assert staged_research["source"]["artifact"] == staged["artifact"]
