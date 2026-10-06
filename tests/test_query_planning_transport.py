"""Production MCP transport: preview, execute, link and replay a bounded plan."""

import asyncio

from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app
from tests.test_mcp_harness import _payload, _serve, _session


async def test_query_planning_mcp_journey(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_RESEARCH", "1")
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=2)], token="planning-test")
    async with _serve(app) as url, _session(url, "planning-test") as (client, _):
        await client.initialize()

        async def call(name, **args):
            response = await client.call_tool(name, args)
            assert not response.isError, response
            value = _payload(response)
            assert "error" not in value, value
            return value

        discovered = {tool.name for tool in (await client.list_tools()).tools}
        assert {
            "slopsearx_plan_research",
            "slopsearx_plan_query_variants",
            "slopsearx_plan_research_followup",
        } <= discovered
        preview = await call(
            "slopsearx_plan_research",
            question="compare maintenance and alternatives",
            engines=["wikipedia"],
            subquestions=[{"id": "a", "question": "maintenance"}, {"id": "b", "question": "alternatives"}],
            max_queries=4,
            max_attempts=4,
            max_engine_attempts=4,
        )
        assert preview["dispatched"] is False
        job = await call(preview["execution"]["tool"], **preview["execution"]["arguments"])
        async with asyncio.timeout(10):
            while job["state"] in {"queued", "running"}:
                await asyncio.sleep(0.02)
                job = await call("slopsearx_get_job", job_id=job["job_id"])
        assert [q["planning_method"] for q in job["queries"]] == ["original", "decomposition", "decomposition"]
        attempt = job["queries"][1]["attempts"][0]
        followup = await call(
            "slopsearx_plan_research_followup",
            job_id=job["job_id"],
            query="new maintenance lead",
            parent_attempt_id=attempt["attempt_id"],
            evidence_result_ids=attempt["admitted_result_ids"][:1],
            subquestion_id="a",
            rationale="Investigate this reading lead",
            engines=["wikipedia"],
        )
        args = {**followup["execution"]["arguments"], "continuation_key": "evidence-step"}
        result = await call(followup["execution"]["tool"], **args)
        assert result["queries"][-1]["evidence_result_ids"] == args["evidence_result_ids"]
        replay = await call(followup["execution"]["tool"], **args)
        assert replay["budgets"]["used"] == result["budgets"]["used"]
        variants = await call(
            "slopsearx_plan_query_variants",
            question="CVE-2026-12345",
            variants=["CVE-2026-12345 advisory"],
            engines=["wikipedia"],
            max_queries=2,
        )
        variant_job = await call(variants["execution"]["tool"], **variants["execution"]["arguments"])
        assert variant_job["queries"][1]["planning_method"] == "terminology_expansion"
        prompts = {prompt.name for prompt in (await client.list_prompts()).prompts}
        assert "plan_research_with_evidence" in prompts
