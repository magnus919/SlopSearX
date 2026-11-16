"""Manual offline W06 reference capture. Never run from a native gate or runtime.

Pinned CPython 3.14.7; G08 removes this executable after migration. All checks
are explicit exceptions, including under python -O. No network or Valkey used.
"""

# ruff: noqa: E402 -- manual script imports the pinned checkout after path binding

from __future__ import annotations

import ast
import asyncio
import copy
import dataclasses
import hashlib
import json
import sys
import uuid
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
if sys.version_info[:3] != (3, 14, 7):
    raise RuntimeError(f"requires CPython3.14.7, found {sys.version}")
for source_path, source_sha in json.loads(Path(__file__).with_name("source-controls.json").read_text()).items():
    if hashlib.sha256((ROOT / source_path).read_bytes()).hexdigest() != source_sha:
        raise RuntimeError(f"source hash guard rejected drift before import: {source_path}")

import pytest

from slopsearx import staged
from slopsearx.adapter import SearchResult
from slopsearx.artifacts import artifact_ref, composite_artifact_id
from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp import composition, staged_tools
from slopsearx.mcp.state import set_state
from slopsearx.service import EngineOutcome, ScopeDecision, SearchResponse
from tests.test_mcp_tools import _build_state, _FakeStore

BASELINE = "9b7d888da1ee3f7e2ed045c2185ea16f419381e9"
SOURCES = [
    "slopsearx/staged.py",
    "slopsearx/mcp/staged_tools.py",
    "slopsearx/mcp/composition.py",
    "slopsearx/mcp/result_serialization.py",
    "slopsearx/artifacts.py",
    "slopsearx/snapshot.py",
    "tests/test_staged_search.py",
    "tests/test_staged_valkey_integration.py",
    "tests/test_workflow_composition.py",
    "tests/test_mcp_tools.py",
]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def json_safe(value):
    if dataclasses.is_dataclass(value):
        return json_safe(dataclasses.asdict(value))
    if isinstance(value, dict):
        return {key: json_safe(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [json_safe(item) for item in value]
    if isinstance(value, set):
        return sorted(json_safe(item) for item in value)
    return value


def state():
    policy = MCPPolicy()
    policy.enabled_tools["staged_search"] = True
    value = _build_state(["wikipedia", "brave"], policy=policy)
    shared = _FakeStore()
    value.ctx.cache = shared
    value.snapshots._store = shared
    value.staged_store = staged.StagedSearchStore(shared)
    value.staged_runner = SimpleNamespace(enqueue=lambda *_: asyncio.sleep(0))
    set_state(value)
    return value


def plan_inputs(**changes):
    return (
        dict(
            query="  durable evidence 雪  ",
            objectives={"deadline_ms": 5000, "max_engine_calls": 2},
            initial_scope={"engines": ["wikipedia"]},
            fallback_scope={"engines": ["brave"]},
            allow_scope_expansion=True,
            filters=None,
        )
        | changes
    )


def plan_cases():
    cases = []
    variants = [
        ("valid", {}),
        ("valid_single", {"fallback_scope": None, "allow_scope_expansion": False}),
        (
            "valid_filters",
            {
                "filters": {
                    "language": "fr",
                    "time_range": "week",
                    "safesearch": "moderate",
                    "freshness": "prefer_fresh",
                }
            },
        ),
        ("query_blank", {"query": " "}),
        ("query_too_long", {"query": "x" * 10001}),
        ("objectives_extra", {"objectives": {"deadline_ms": 2, "max_engine_calls": 2, "extra": 1}}),
        ("expansion_missing", {"allow_scope_expansion": False}),
        ("expansion_without_scope", {"fallback_scope": None}),
        ("filters_unknown", {"filters": {"x": 1}}),
        ("filters_safesearch", {"filters": {"safesearch": "bad"}}),
        ("filters_freshness", {"filters": {"freshness": "bad"}}),
        ("filters_time_range", {"filters": {"time_range": "bad"}}),
        ("language_nonstring_source_accepts", {"filters": {"language": 1}}),
        ("scope_empty", {"initial_scope": {}}),
        ("fallback_empty_dict", {"fallback_scope": {}, "allow_scope_expansion": False}),
        ("scope_both", {"initial_scope": {"engines": ["wikipedia"], "intent": "auto"}}),
        ("scope_duplicate", {"initial_scope": {"engines": ["wikipedia", "wikipedia"]}}),
        ("scope_unknown", {"initial_scope": {"engines": ["unknown"]}}),
        ("scope_intent_media", {"initial_scope": {"intent": "media"}}),
        ("scope_overlap", {"fallback_scope": {"engines": ["wikipedia"]}}),
        ("scope_exceeds_calls", {"objectives": {"deadline_ms": 5000, "max_engine_calls": 1}}),
        ("strict_full_scope", {"filters": {"safesearch": "strict"}}),
        ("order_query_before_objectives", {"query": " ", "objectives": {}}),
        ("order_deadline_before_calls", {"objectives": {"deadline_ms": False, "max_engine_calls": False}}),
        ("order_expansion_before_filters", {"allow_scope_expansion": False, "filters": {"x": 1}}),
        (
            "order_overlap_before_budget_strict",
            {
                "fallback_scope": {"engines": ["wikipedia"]},
                "objectives": {"deadline_ms": 5000, "max_engine_calls": 1},
                "filters": {"safesearch": "strict"},
            },
        ),
    ]
    for field, values in [
        ("deadline_ms", [False, 0, 1, 30000, 30001, 1.5]),
        ("max_engine_calls", [True, 0, 1, 64, 65, 1.5]),
    ]:
        for index, value in enumerate(values):
            variants.append(
                (f"{field}_{index}", {"objectives": {"deadline_ms": 5000, "max_engine_calls": 2} | {field: value}})
            )
    for name, changes in variants:
        value = state()
        inputs = plan_inputs(**changes)
        plan, error = staged_tools._build_plan(**inputs)
        output = {"plan": plan, "error": error}
        row = dict(id=f"plan.{name}", inputs=inputs, output=output)
        if plan:
            encoded = canonical(plan["identity"])
            require(hashlib.sha256(encoded.encode()).hexdigest() == plan["digest"], "plan digest mismatch")
            row.update(identity_canonical=encoded, identity_sha256=plan["digest"])
        require(all(engine.calls == 0 for engine in value.ctx.active_engines.values()), "preview dispatched")
        cases.append(row)
    value = state()
    value.policy.staged_max_deadline_ms = 1000
    value.policy.staged_max_engine_calls = 1
    inputs = plan_inputs()
    plan, error = staged_tools._build_plan(**inputs)
    cases.append(
        dict(
            id="plan.operator_cap",
            inputs=inputs,
            operator_caps={"deadline_ms": 1000, "max_engine_calls": 1},
            output={"plan": plan, "error": error},
        )
    )
    return cases


async def runner_cases():
    cases = []
    variants = [
        "nonempty",
        "clean_empty",
        "clean_empty_single",
        "fallback_nonempty",
        "fallback_clean_empty",
        "cached_empty",
        "partial_empty",
        "deadline_empty",
        "error_empty",
        "missing_outcomes",
        "wrong_engine_ok",
        "nonempty_partial",
        "nonempty_error",
        "nonempty_deadline",
        "nonempty_missing_outcomes",
        "absolute_deadline",
        "execution_exception",
        "initial_policy_rejection",
        "deadline_before_dispatch",
        "deadline_after_renewal",
        "renewal_lost",
    ]
    for name in variants:
        value = state()
        clock = [1000.0]
        with pytest.MonkeyPatch.context() as patch:
            counter = iter(range(1, 100))
            patch.setattr(uuid, "uuid4", lambda: uuid.UUID(int=next(counter) << 80))
            patch.setattr(staged.time, "time", lambda: clock[0])
            patch.setattr(staged.secrets, "token_hex", lambda _: "fixed-lease-token")
            args = plan_inputs(query="durable evidence")
            if name == "clean_empty_single":
                args.update(fallback_scope=None, allow_scope_expansion=False)
            accepted = await staged_tools.slopsearx_search_staged(**args, idempotency_key="capture")
            require("error" not in accepted, str(accepted))
            operation = accepted["operation_id"]
            before = copy.deepcopy((await value.staged_store.read("default", operation)).record)
            rows = (
                [
                    SearchResult(
                        "https://example.test/full", "title", "content", "wikipedia", payload={"raw": "unsliced"}
                    )
                ]
                if name.startswith("nonempty") or name == "absolute_deadline"
                else []
            )
            outcomes = (
                []
                if name in {"missing_outcomes", "nonempty_missing_outcomes"}
                else [
                    EngineOutcome(
                        "brave" if name == "wrong_engine_ok" else "wikipedia",
                        "timeout" if "error" in name else "ok",
                        len(rows),
                        2.0,
                        "source message" if "error" in name else None,
                    )
                ]
            )
            response = SearchResponse(
                "durable evidence",
                rows,
                ScopeDecision(selected_engines=["wikipedia"]),
                outcomes,
                query_id="q-fixed",
                cached=name == "cached_empty",
                partial="partial" in name,
                deadline_exceeded=name in {"deadline_empty", "nonempty_deadline"},
                dispatched_engine_count=1,
            )
            requests = []
            request_times = []
            responses = []
            claimed = []
            snapshot_ids = []
            snapshot_create_inputs = []
            original_create = value.snapshots.create

            async def create(*args, **kwargs):
                snapshot_create_inputs.append(json_safe({"args": args, "kwargs": kwargs}))
                cursor = await original_create(*args, **kwargs)
                snapshot_ids.append(cursor)
                return cursor

            patch.setattr(value.snapshots, "create", create)
            original_claim = value.staged_store.claim

            async def claim(*args):
                result = await original_claim(*args)
                if result is not None:
                    claimed.append(copy.deepcopy(result[0]))
                return result

            patch.setattr(value.staged_store, "claim", claim)

            class Service:
                async def search(self, request):
                    request_times.append(clock[0])
                    requests.append(json_safe(dataclasses.asdict(request)))
                    if name == "execution_exception":
                        raise RuntimeError("fixed execution failure")
                    if name == "absolute_deadline":
                        clock[0] = 1006.0
                    responses.append(json_safe(dataclasses.asdict(response)))
                    return response

            original_renew = value.staged_store.renew

            async def renew(*args):
                if name == "renewal_lost":
                    return False
                if name == "deadline_after_renewal":
                    clock[0] = 1006.0
                return await original_renew(*args)

            patch.setattr(value.staged_store, "renew", renew)
            policy_rejection = (
                {"error": {"code": "policy_rejected", "message": "fixed revoked"}}
                if name == "initial_policy_rejection"
                else None
            )

            def policy(_):
                return policy_rejection

            runner = staged.StagedSearchRunner(Service(), value.staged_store, value.snapshots, policy)
            if name == "deadline_before_dispatch":
                clock[0] = 1006.0
            await runner.run_one("default", operation)
            intermediate = copy.deepcopy((await value.staged_store.read("default", operation)).record)
            if name in {"fallback_nonempty", "fallback_clean_empty"}:
                require(intermediate["state"] == "queued", "clean initial did not queue fallback")
                response.scope = ScopeDecision(selected_engines=["brave"])
                response.engine_outcomes = [EngineOutcome("brave", "ok", 1 if name == "fallback_nonempty" else 0, 2.0)]
                if name == "fallback_nonempty":
                    response.results = [SearchResult("https://example.test/fallback", "fallback", "full", "brave")]
                await runner.run_one("default", operation)
            record = copy.deepcopy((await value.staged_store.read("default", operation)).record)
            full = await staged_tools._render(record, ["results", "payload"], 1)
            cases.append(
                dict(
                    id=f"runner.{name}",
                    before=before,
                    claimed=claimed,
                    intermediate=intermediate,
                    completion_time=clock[0],
                    response={
                        "rows": [dataclasses.asdict(r) | {"engines": sorted(r.engines)} for r in rows],
                        "outcomes": [dataclasses.asdict(o) for o in outcomes],
                        "cached": response.cached,
                        "partial": response.partial,
                        "deadline_exceeded": response.deadline_exceeded,
                    },
                    responses=responses,
                    snapshot_ids=snapshot_ids,
                    snapshot_create_inputs=snapshot_create_inputs,
                    policy_rejection=policy_rejection,
                    execution_error={"code": "execution_failed", "message": "fixed execution failure"}
                    if name == "execution_exception"
                    else None,
                    requests=requests,
                    request_times=request_times,
                    record=record,
                    rendered=full,
                    render_input={
                        "include": ["results", "payload"],
                        "max_results": 1,
                        "policy_max_results": value.policy.max_results,
                    },
                    record_canonical=canonical(record),
                    record_sha256=hashlib.sha256(canonical(record).encode()).hexdigest(),
                )
            )
    return cases


async def source_cases():
    cases = []

    class Store:
        available = True

        def for_tenant(self, _):
            return self

        async def load(self, _):
            return SimpleNamespace(engines=["wikipedia"])

        async def reports(self, _, **kwargs):
            self.read_arguments = kwargs
            return self.rows

    variants = [
        "valid_historical_query",
        "missing",
        "truncated",
        "report_size",
        "incomparable",
        "reasons",
        "blank_query",
        "nonstring_query",
        "missing_engines",
        "empty_engines",
        "grant_denied",
        "store_unavailable",
        "report_scope_revoked",
        "malformed_id",
    ]
    for name in variants:
        value = state()
        value.policy.enabled_tools["saved_searches"] = name != "grant_denied"
        store = Store()
        store.available = name != "store_unavailable"
        report = {
            "run_id": "run",
            "status": "changed",
            "revision": 3,
            "incomparable_reasons": [],
            "observation": {"window": {"query": "historical query", "engines": ["wikipedia"]}},
        }
        if name == "truncated":
            report.update(status="incomparable", incomparable_reasons=["dispatch_failed", "observation_truncated"])
        if name == "report_size":
            report.update(status="incomparable", incomparable_reasons=["report_size_exceeded"])
        if name == "incomparable":
            report["status"] = "incomparable"
        if name == "reasons":
            report["incomparable_reasons"] = ["dispatch_failed"]
        window = report["observation"]["window"]
        if name == "blank_query":
            window["query"] = " "
        if name == "nonstring_query":
            window["query"] = 42
        if name == "missing_engines":
            del window["engines"]
        if name == "empty_engines":
            window["engines"] = []
        if name == "report_scope_revoked":
            window["engines"] = ["brave"]
            value.policy.sensitive_engines.add("brave")
        store.rows = [] if name == "missing" else [report]
        value.saved_store = store
        ref = artifact_ref(
            "saved_report", "malformed" if name == "malformed_id" else composite_artifact_id("saved", "run")
        )
        result = await composition.resolve_source(ref, "staged_search")
        output = result.stored() if isinstance(result, composition.ResolvedSource) else result
        expected_error = {
            "missing": "source_not_found",
            "truncated": "source_truncated",
            "report_size": "source_truncated",
            "incomparable": "source_incomparable",
            "reasons": "source_incomparable",
            "blank_query": "source_incomparable",
            "nonstring_query": "source_incomparable",
            "missing_engines": "source_incomparable",
            "empty_engines": "source_incomparable",
            "grant_denied": "source_policy_denied",
            "store_unavailable": "source_unavailable",
            "report_scope_revoked": "source_policy_denied",
            "malformed_id": "source_not_found",
        }
        if name == "valid_historical_query":
            require(
                output.get("query") == "historical query" and output.get("details", {}).get("revision") == 3,
                "valid saved source capture did not resolve its historical report",
            )
        else:
            require(
                output.get("error", {}).get("code") == expected_error[name],
                f"saved source capture did not reach {name}: {output}",
            )
        require(all(engine.calls == 0 for engine in value.ctx.active_engines.values()), "source dispatched")
        cases.append(
            dict(
                id=f"source.{name}",
                artifact=ref,
                inputs={
                    "saved_granted": value.policy.tool_enabled("saved_searches"),
                    "store_available": store.available,
                    "definition": {"engines": ["wikipedia"]},
                    "reports": copy.deepcopy(store.rows),
                    "sensitive_engines": sorted(value.policy.sensitive_engines),
                    "saved_max_reports": value.policy.saved_max_reports,
                },
                report=report,
                output=output,
                listing_arguments=getattr(store, "read_arguments", None),
            )
        )
    return cases


async def replay_cases():
    value = state()
    cases = []
    with pytest.MonkeyPatch.context() as patch:
        counter = iter(range(100, 200))
        patch.setattr(uuid, "uuid4", lambda: uuid.UUID(int=next(counter) << 80))
        patch.setattr(staged.time, "time", lambda: 1000.0)
        args = plan_inputs(query="durable evidence")
        accepted = await staged_tools.slopsearx_search_staged(**args, idempotency_key="replay")
        require("error" not in accepted, "replay setup rejected")
        read = await value.staged_store.read("default", accepted["operation_id"])
        record = read.record
        for name, changes in [
            ("same", {}),
            ("query", {"query": "changed"}),
            ("requested_objectives", {"objectives": {"deadline_ms": 4999, "max_engine_calls": 2}}),
            ("scope_order", {"initial_scope": {"engines": ["brave"]}}),
            ("fallback", {"fallback_scope": None}),
            ("consent", {"allow_scope_expansion": False}),
            ("filters", {"filters": {"language": "fr"}}),
            ("normalized_default_filters", {"filters": {"language": "en"}}),
        ]:
            inputs = args | changes
            result = staged_tools._replay_equivalent(record, **inputs)
            cases.append(dict(id=f"replay.{name}", record=copy.deepcopy(record), inputs=inputs, equivalent=result))
        value.policy.staged_max_deadline_ms = 1000
        replay = await staged_tools.slopsearx_search_staged(**args, idempotency_key="replay")
        cases.append(
            dict(
                id="replay.narrowed_cap_rejects_after_equivalence",
                record=copy.deepcopy(record),
                inputs=args,
                limits={"deadline_ms": 1000, "max_engine_calls": value.policy.staged_max_engine_calls, "granted": True},
                submission=dict(args, idempotency_key="replay", source=None),
                output=replay,
            )
        )
        # Source resolution takes precedence over an existing valid idempotency key.
        patch.setattr(staged_tools, "resolve_source", lambda *_: unavailable())

        async def unavailable():
            return {"error": {"code": "source_unavailable", "message": "source artifact store is unavailable"}}

        source = artifact_ref("saved_report", composite_artifact_id("saved", "run"))
        output = await staged_tools.slopsearx_search_staged(
            **(args | {"query": None}), idempotency_key="replay", source=source
        )
        cases.append(
            dict(
                id="replay.source_resolution_before_lookup",
                record=copy.deepcopy(record),
                inputs=args | {"query": None},
                submission=dict(args | {"query": None}, idempotency_key="replay", source=source),
                source_resolution_error=await unavailable(),
                output=output,
            )
        )
        output = await staged_tools.slopsearx_search_staged(**args, idempotency_key="replay", source=source)
        cases.append(
            dict(
                id="replay.query_source_conflict_before_resolution",
                record=copy.deepcopy(record),
                inputs=args,
                submission=dict(args, idempotency_key="replay", source=source),
                source_resolution_error=await unavailable(),
                output=output,
            )
        )
    return cases


async def main():
    require(sys.version_info[:3] == (3, 14, 7), f"requires CPython3.14.7, found {sys.version}")
    controls = Path(__file__).with_name("source-controls.json")
    hashes = {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES}
    require(hashes == json.loads(controls.read_text()), "source hash guards rejected drift")
    cases = plan_cases() + await runner_cases()
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(composition.time, "time", lambda: 1000.0)
        cases += await source_cases()
    cases += await replay_cases()
    require(len({row["id"] for row in cases}) == len(cases), "duplicate source case")
    fixture = state()
    context = dict(
        catalog=[
            dataclasses.asdict(cap) | {"active": cap.name in fixture.ctx.active_engines}
            for cap in fixture.catalog.all()
        ],
        active_engines={
            name: {"categories": engine.categories, "engine_type": engine.engine_type}
            for name, engine in fixture.ctx.active_engines.items()
        },
        tier1_engines=sorted(fixture.ctx.tier1_engines),
        sensitive_engines=sorted(fixture.ctx.sensitive_engines),
        router=None,
        max_query_length=fixture.policy.max_query_length,
        staged_max_deadline_ms=fixture.policy.staged_max_deadline_ms,
        staged_max_engine_calls=fixture.policy.staged_max_engine_calls,
    )
    functions = []
    for path in SOURCES:
        text = (ROOT / path).read_text()
        tree = ast.parse(text)

        def walk(node, prefix=""):
            for child in ast.iter_child_nodes(node):
                if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    functions.append(
                        dict(
                            path=path,
                            symbol=prefix + child.name,
                            line=child.lineno,
                            signature=ast.unparse(child.args),
                            decorators=[ast.unparse(d) for d in child.decorator_list],
                            body_sha256=hashlib.sha256(ast.get_source_segment(text, child).encode()).hexdigest(),
                        )
                    )
                    walk(child, prefix + child.name + ".")
                elif isinstance(child, ast.ClassDef):
                    walk(child, prefix + child.name + ".")

        walk(tree)
    packet = dict(
        schema_version=1,
        plan_id="W06",
        baseline=BASELINE,
        sources=hashes,
        source_functions=functions,
        context=context,
        nondeterminism={
            "wall_time": "1000.0 except explicitly advanced deadline cases",
            "uuid": "sequential fixed UUID integers shifted left80 per runner/replay case so snapshot first12 hex characters remain unique",
            "lease_token": "fixed-lease-token",
            "query_id": "q-fixed",
            "latency_ms": 2.0,
        },
        provider_calls=0,
        cases=cases,
    )
    destination = Path(__file__).with_name("source.json")
    destination.write_text(json.dumps(packet, ensure_ascii=True, indent=2) + "\n")
    set_state(None)
    print(f"W06 captured {len(cases)} complete source cases; providers=0")


if __name__ == "__main__":
    asyncio.run(main())
