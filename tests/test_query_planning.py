"""Planning boundaries and execution revalidation; no provider/model calls."""

import copy
import hashlib
import json
import time

import pytest

from slopsearx.mcp import planning_tools as p
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state, tenant_scope
from slopsearx.research_models import _job_from_payload, _job_to_payload
from slopsearx.research_planning import validate_variant
from tests.test_research_retry_followup import _build_state


@pytest.fixture
def state():
    value = _build_state(["wikipedia"])
    set_state(value)
    yield value
    set_state(None)


async def first_job(state, **kwargs):
    args = {
        "question": "Compare maintenance and alternatives",
        "max_queries": 4,
        "max_attempts": 4,
        "max_engine_attempts": 4,
        "max_results": 20,
        "subquestions": [{"id": "a", "question": "Maintained?"}],
        "initial_plan": [{"query": "maintenance", "engines": ["wikipedia"], "subquestion_id": "a"}],
    }
    args.update(kwargs)
    response = await t.slopsearx_start_research(**args)
    assert "error" not in response, response
    job = await state.job_store.load(response["job_id"])
    await state.runner.run_direct(job)
    return await t.slopsearx_get_job(job.job_id)


def followup_args(job):
    attempt = job["queries"][0]["attempts"][0]
    return {
        "job_id": job["job_id"],
        "query": "new maintenance evidence",
        "engines": ["wikipedia"],
        "parent_attempt_id": attempt["attempt_id"],
        "evidence_result_ids": attempt["admitted_result_ids"][:1],
        "rationale": "Investigate the lead",
        "subquestion_id": "a",
    }


async def test_decomposition_preview_to_durable_execution(state):
    before = copy.deepcopy(state.job_store._store._data)
    preview = await p.slopsearx_plan_research(
        "Compare maintenance and alternatives",
        engines=["wikipedia"],
        max_queries=3,
        max_attempts=3,
        max_engine_attempts=3,
        subquestions=[{"id": "a", "question": "maintenance"}, {"id": "b", "question": "alternatives"}],
    )
    assert "error" not in preview, preview
    assert preview["dispatched"] is False
    assert state.job_store._store._data == before
    assert state.ctx.active_engines["wikipedia"].calls == 0
    args = preview["execution"]["arguments"]
    assert args["initial_plan"][0]["query"] == args["question"]
    started = await t.slopsearx_start_research(**args)
    assert "error" not in started, started
    job = await state.job_store.load(started["job_id"])
    await state.runner.run_direct(job)
    assert state.ctx.active_engines["wikipedia"].calls == 3
    assert [q.planning_method for q in job.queries] == ["original", "decomposition", "decomposition"]
    assert _job_from_payload(_job_to_payload(job)) == job
    assert all(item["state"] == "unresolved" for item in job.subquestions.values())


@pytest.mark.parametrize(
    "changes",
    [
        {"max_queries": True},
        {"max_attempts": 1},
        {"max_engine_attempts": 1},
        {"subquestions": []},
        {"subquestions": [{"id": "a", "question": "x"}] * 2},
        {"subquestions": [{"id": "a", "question": "x", "extra": "no"}]},
        {"subquestions": [{"id": "a", "question": "x", "query": True}]},
        {"subquestions": [{"id": "a", "question": "x", "intent": "science"}]},
        {"subquestions": [{"id": "a", "question": "x", "engines": ["wikipedia", "hibp"]}]},
    ],
)
async def test_invalid_plan_is_atomic(state, changes):
    args = {"question": "q", "subquestions": [{"id": "a", "question": "x"}], "engines": ["wikipedia"]}
    args.update(changes)
    before = copy.deepcopy(state.job_store._store._data)
    assert "error" in await p.slopsearx_plan_research(**args)
    assert state.job_store._store._data == before
    assert state.ctx.active_engines["wikipedia"].calls == 0


@pytest.mark.parametrize("identifier", ["CVE-2026-12345", "10.1000/example", "2303.07678v2", "@scope/package"])
async def test_variant_identifier_preservation_rechecked_at_execution(state, identifier):
    preview = await p.slopsearx_plan_query_variants(
        f"Find {identifier}", [f"evidence for {identifier}"], engines=["wikipedia"], max_queries=2
    )
    assert "error" not in preview, preview
    args = preview["execution"]["arguments"]
    args["initial_plan"][1]["query"] = "missing identifier"
    rejected = await t.slopsearx_start_research(**args)
    assert rejected["error"]["code"] == "invalid_input"
    assert state.ctx.active_engines["wikipedia"].calls == 0
    invalid = await p.slopsearx_plan_query_variants(f"Find {identifier}", ["different"], engines=["wikipedia"])
    assert invalid["error"]["code"] == "invalid_input"


def test_identifier_matching_rejects_prefix_collision():
    with pytest.raises(ValueError):
        validate_variant("CVE-2026-1234", "CVE-2026-12345")
    validate_variant("See DOI 10.1000/example.", "10.1000/example evidence")


async def test_variants_reject_duplicate_without_dispatch(state):
    result = await p.slopsearx_plan_query_variants("original", [" ORIGINAL "], engines=["wikipedia"])
    assert result["error"]["code"] == "duplicate_query"
    assert state.ctx.active_engines["wikipedia"].calls == 0


async def test_evidence_preview_execution_and_replay_preserve_history(state):
    job = await first_job(state)
    before = copy.deepcopy(state.job_store._store._data)
    preview = await p.slopsearx_plan_research_followup(**followup_args(job))
    assert "error" not in preview, preview
    assert preview["dispatched"] is False
    assert preview["evidence"][0]["title"] == "wikipedia 0"
    assert state.job_store._store._data == before
    assert state.ctx.active_engines["wikipedia"].calls == 1
    args = preview["execution"]["arguments"]
    args["continuation_key"] = "next"
    result = await t.slopsearx_extend_research(**args)
    assert "error" not in result, result
    assert result["queries"][0]["attempts"] == job["queries"][0]["attempts"]
    assert result["queries"][1]["planning_method"] == "evidence_followup"
    assert result["queries"][1]["evidence_result_ids"] == args["evidence_result_ids"]
    loaded = await state.job_store.load(job["job_id"])
    assert loaded.queries[1].evidence_engines == ["wikipedia"]
    assert _job_from_payload(_job_to_payload(loaded)) == loaded
    await t.slopsearx_update_research(job["job_id"], {}, complete=True)
    replay = await t.slopsearx_extend_research(**args)
    assert "error" not in replay, replay
    assert state.ctx.active_engines["wikipedia"].calls == 2
    args["evidence_result_ids"] = job["queries"][0]["attempts"][0]["admitted_result_ids"][1:2]
    assert (await t.slopsearx_extend_research(**args))["error"]["code"] == "idempotency_conflict"


@pytest.mark.parametrize(
    "changes",
    [
        {"parent_attempt_id": "foreign"},
        {"evidence_result_ids": ["foreign:0"]},
        {"evidence_result_ids": []},
        {"evidence_result_ids": [True]},
        {"evidence_result_ids": ["a:0"] * 2},
        {"evidence_result_ids": [f"a:{i}" for i in range(11)]},
        {"subquestion_id": "foreign"},
        {"rationale": ""},
        {"query": " MAINTENANCE "},
    ],
)
async def test_followup_rejection_is_read_only(state, changes):
    job = await first_job(state)
    args = followup_args(job)
    args.update(changes)
    before = copy.deepcopy(state.job_store._store._data)
    assert "error" in await p.slopsearx_plan_research_followup(**args)
    assert state.job_store._store._data == before
    assert state.ctx.active_engines["wikipedia"].calls == 1


async def test_cross_tenant_unadmitted_expired_and_resolved_evidence(state):
    job = await first_job(state, max_results=1)
    args = followup_args(job)
    with tenant_scope("other"):
        assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "invalid_job_id"
    # Admission cap excludes the other snapshot records from usable parent references.
    args["evidence_result_ids"] = [job["queries"][0]["cursor"] + ":1"]
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "invalid_result_id"
    job = await first_job(state)
    args = followup_args(job)
    await t.slopsearx_update_research(job["job_id"], {"a": "resolved"})
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "invalid_job_state"
    await t.slopsearx_update_research(job["job_id"], {"a": "unresolved"})
    cursor = job["queries"][0]["cursor"]
    for key, value in state.snapshots._store._data.items():
        if key.endswith(cursor):
            value["expires_at"] = time.time() - 1
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "expired_handle"


async def test_preview_cannot_authorize_changed_progress_or_policy(state):
    job = await first_job(state)
    preview = await p.slopsearx_plan_research_followup(**followup_args(job))
    args = preview["execution"]["arguments"]
    await t.slopsearx_update_research(job["job_id"], {"a": "resolved"})
    assert (await t.slopsearx_extend_research(**args))["error"]["code"] == "invalid_job_state"
    assert state.ctx.active_engines["wikipedia"].calls == 1
    await t.slopsearx_update_research(job["job_id"], {"a": "unresolved"})
    state.policy.sensitive_engines.add("wikipedia")
    assert "error" in await t.slopsearx_extend_research(**args)
    assert "error" in await p.slopsearx_plan_research_followup(**followup_args(job))
    assert state.ctx.active_engines["wikipedia"].calls == 1


async def test_recovered_query_rechecks_captured_evidence_scope(state):
    job = await first_job(state)
    preview = await p.slopsearx_plan_research_followup(**followup_args(job))
    await t.slopsearx_extend_research(**preview["execution"]["arguments"])
    loaded = await state.job_store.load(job["job_id"])
    query = loaded.queries[-1]
    query.evidence_engines = ["hibp"]
    assert t._research_dispatch_error(state, query, check_evidence=True) is not None
    query.evidence_engines = ["wikipedia"]
    query.evidence_intent = "science"
    assert t._research_dispatch_error(state, query, check_evidence=True) is not None


async def test_legacy_digest_and_payload_remain_compatible(state):
    job = await first_job(state)
    result = await t.slopsearx_extend_research(
        job["job_id"], "old style", engines=["wikipedia"], continuation_key="legacy"
    )
    assert "error" not in result, result
    loaded = await state.job_store.load(job["job_id"])
    expected = hashlib.sha256(json.dumps(["old style", "web", ["wikipedia"], None, None, None]).encode()).hexdigest()
    assert loaded.queries[-1].continuation_digest == expected
    payload = _job_to_payload(loaded)
    for item in payload["queries"]:
        for key in ["planning_method", "evidence_result_ids", "evidence_engines", "evidence_intent"]:
            item.pop(key)
    legacy = _job_from_payload(payload)
    assert all(q.planning_method is None and q.evidence_result_ids == [] for q in legacy.queries)


async def test_grants_and_exhaustion(state):
    job = await first_job(state, max_queries=1)
    assert (await p.slopsearx_plan_research_followup(**followup_args(job)))["error"]["code"] == "job_budget_exceeded"
    state.policy.enabled_tools["research"] = False
    assert (await p.slopsearx_plan_query_variants("q", ["v"]))["error"]["code"] == "tool_disabled"
    assert (await p.slopsearx_plan_research("q", [{"id": "a", "question": "x"}]))["error"]["code"] == "tool_disabled"
    assert (await p.slopsearx_plan_research_followup(**followup_args(job)))["error"]["code"] == "tool_disabled"


async def test_evidence_replay_rechecks_revoked_parent_intent(state):
    job = await first_job(state)
    loaded = await state.job_store.load(job["job_id"])
    loaded.queries[0].intent = "science"
    loaded.queries[0].requires_intent_grant = True
    state.policy.enabled_tools["science"] = True
    await state.job_store.save(loaded)
    preview = await p.slopsearx_plan_research_followup(**followup_args(job))
    assert "error" not in preview, preview
    args = {**preview["execution"]["arguments"], "continuation_key": "bound-parent"}
    result = await t.slopsearx_extend_research(**args)
    assert "error" not in result, result
    state.policy.enabled_tools["science"] = False
    replay = await t.slopsearx_extend_research(**args)
    assert replay["error"]["code"] == "policy_rejected"
    assert state.ctx.active_engines["wikipedia"].calls == 2


async def test_followup_running_missing_store_and_expired_job(state):
    job = await first_job(state)
    args = followup_args(job)
    loaded = await state.job_store.load(job["job_id"])
    loaded.state = "running"
    await state.job_store.save(loaded)
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "invalid_job_state"
    loaded.state = "succeeded"
    loaded.deadline = time.time() - 1
    await state.job_store.save(loaded)
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "deadline_exceeded"
    state.job_store._store.is_connected = False
    assert (await p.slopsearx_plan_research_followup(**args))["error"]["code"] == "store_unavailable"


async def test_resolved_state_race_invalidates_leased_execution(state):
    job = await first_job(state)
    preview = await p.slopsearx_plan_research_followup(**followup_args(job))
    original = state.runner.run_direct

    async def concurrent_resolution(pending, *, mutate=None):
        current = await state.job_store.load(pending.job_id)
        current.subquestions["a"]["state"] = "resolved"
        await state.job_store.save(current)
        return await original(pending, mutate=mutate)

    state.runner.run_direct = concurrent_resolution
    rejected = await t.slopsearx_extend_research(**preview["execution"]["arguments"])
    assert rejected["error"]["code"] == "invalid_job_state"
    current = await state.job_store.load(job["job_id"])
    assert len(current.queries) == 1
    assert state.ctx.active_engines["wikipedia"].calls == 1


async def test_actual_snapshot_provenance_checked_before_disclosure(state):
    job = await first_job(state)
    cursor = job["queries"][0]["cursor"]
    for key, value in state.snapshots._store._data.items():
        if key.endswith(cursor):
            value["results"][0]["engine"] = "hibp"
            value["results"][0]["engines"] = ["hibp"]
    denied = await p.slopsearx_plan_research_followup(**followup_args(job))
    assert "error" in denied
    assert "evidence" not in denied
    assert state.ctx.active_engines["wikipedia"].calls == 1
