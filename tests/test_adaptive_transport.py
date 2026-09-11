"""Model-independent research journey over the production MCP transport."""

import asyncio

from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app
from tests.test_mcp_harness import _payload, _serve, _session


async def test_adaptive_research_journey(monkeypatch):
    monkeypatch.setenv("MCP_GRANT_RESEARCH", "1")
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=3)], token="research-test")
    async with _serve(app) as url, _session(url, "research-test") as (client, _):
        await client.initialize()

        async def call(name, **arguments):
            result = await client.call_tool(name, arguments)
            assert not result.isError, result
            value = _payload(result)
            assert "error" not in value, value
            return value

        for field in ("max_queries", "max_attempts", "max_engine_attempts", "max_results"):
            invalid = await client.call_tool("slopsearx_start_research", {"question": "invalid", field: True})
            assert invalid.isError, invalid

        job = await call(
            "slopsearx_start_research",
            question="Compare maintenance and alternatives",
            max_queries=3,
            max_attempts=3,
            subquestions=[
                {"id": "maintenance", "question": "Maintained?"},
                {"id": "alternatives", "question": "Alternatives?"},
            ],
            initial_plan=[
                {
                    "query": "maintenance",
                    "engines": ["wikipedia"],
                    "subquestion_id": "maintenance",
                    "rationale": "Find the baseline",
                }
            ],
        )
        async with asyncio.timeout(10):
            while job["state"] in {"queued", "running"}:
                await asyncio.sleep(0.02)
                job = await call("slopsearx_get_job", job_id=job["job_id"])
        assert job["stop_reason"] == "plan_executed"
        original = job["queries"][0]["attempts"][0]
        original_results = await call("slopsearx_read_results", cursor=original["cursor"])
        followup = dict(
            job_id=job["job_id"],
            query="alternatives",
            engines=["wikipedia"],
            subquestion_id="alternatives",
            parent_attempt_id=original["attempt_id"],
            rationale="Compare the baseline",
            continuation_key="step-2",
        )
        extended = await call("slopsearx_extend_research", **followup)
        assert extended["queries"][0]["attempts"][0] == original
        assert extended["queries"][1]["parent_attempt_id"] == original["attempt_id"]
        replay = await call("slopsearx_extend_research", **followup)
        assert replay["queries"] == extended["queries"]
        conflict = _payload(await client.call_tool("slopsearx_extend_research", {**followup, "query": "different"}))
        assert conflict["error"]["code"] == "idempotency_conflict"
        completed = await call(
            "slopsearx_update_research",
            job_id=job["job_id"],
            subquestion_states={"maintenance": "resolved"},
            complete=True,
        )
        assert completed["stop_reason"] == "caller_completed"
        assert completed["unresolved_subquestions"] == ["alternatives"]
        assert completed["queries"] == extended["queries"]
        assert (await call("slopsearx_read_results", cursor=original["cursor"]))["results"] == original_results[
            "results"
        ]
        retry = await call("slopsearx_retry_research", job_id=job["job_id"])
        assert retry["state"] == "succeeded"
        assert (await call("slopsearx_get_job", job_id=job["job_id"]))["queries"] == completed["queries"]
        rejected = _payload(
            await client.call_tool("slopsearx_extend_research", {**followup, "continuation_key": "step-3"})
        )
        assert rejected["error"]["code"] == "invalid_job_state"
