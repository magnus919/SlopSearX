"""Developer-only source oracle; never invoked by native gates/runtime."""

import argparse
import copy
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

BASELINE = "cfd7a0af61a22cfd040a7fcd41ad950f82005959"
SOURCES = [
    "slopsearx/research_models.py",
    "slopsearx/research_budget.py",
    "slopsearx/research_store.py",
    "slopsearx/merger.py",
    "tests/test_research_durability.py",
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 14, 7):
        raise SystemExit("Python 3.14.7 required")
    root = args.source_root.resolve()
    sources = []
    for name in SOURCES:
        raw = subprocess.check_output(["git", "show", f"{BASELINE}:{name}"], cwd=root)
        if (root / name).read_bytes() != raw:
            raise SystemExit(f"source drift: {name}")
        sources.append(
            {
                "path": name,
                "git_blob": subprocess.check_output(
                    ["git", "rev-parse", f"{BASELINE}:{name}"], cwd=root, text=True
                ).strip(),
                "sha256": hashlib.sha256(raw).hexdigest(),
            }
        )
    sys.path.insert(0, str(root))
    from slopsearx import research_budget as budget
    from slopsearx import research_models as models
    from slopsearx.merger import _normalise_url

    clock = SimpleNamespace(time=lambda: 100.0)
    serial = 0

    def uuid4():
        nonlocal serial
        serial += 1
        return SimpleNamespace(hex=f"{serial:032x}")

    for name, cell in zip(
        models.ResearchQueryAttempt.__init__.__code__.co_freevars, models.ResearchQueryAttempt.__init__.__closure__
    ):
        if name == "__dataclass_dflt_attempted_at__":
            cell.cell_contents = lambda: clock.time()
    budget.time = models.time = clock
    budget.uuid = models.uuid = SimpleNamespace(uuid4=uuid4)
    base = {
        "job_id": "job-fixture",
        "question": "Evidence 雪",
        "strategy": "triangulate",
        "created_at": 1,
        "deadline": 1000,
        "tenant": "org:team",
        "queries": [{"index": 0, "query": "q", "intent": "science", "engines": ["a", "b"]}],
    }
    cases = []

    def record(name, data, operations):
        nonlocal serial
        serial = 0
        job = models._job_from_payload(copy.deepcopy(data))
        initial = models._job_to_payload(job)
        outputs = []
        ops = [copy.deepcopy(op) for op in operations]
        for op in ops:
            clock.time = lambda: op.get("now", 100.0)
            kind = op["kind"]
            if kind == "initialize":
                p = op["policy"]
                budget.initialize_budget(
                    job, SimpleNamespace(job_max_queries=p[0], job_max_engines_per_query=p[1], job_max_results=p[2])
                )
            elif kind == "reserve":
                a = budget.reserve_attempt(job, job.queries[op.get("index", 0)])
                op["attempt_id"] = a.attempt_id if a else None
            elif kind == "finish":
                q = job.queries[op.get("index", 0)]
                for key, value in op["outcome"].items():
                    setattr(q, key, value)
                urls = op.get("urls")
                response = None if urls is None else SimpleNamespace(results=[SimpleNamespace(url=u) for u in urls])
                if urls is not None:
                    op["lead_ids"] = ["lead-v1-" + hashlib.sha256(_normalise_url(u).encode()).hexdigest() for u in urls]
                budget.finish_attempt(job, q, response)
            elif kind == "recover":
                models.recover_orphan_attempts(job)
                op["generated_ids"] = [a.attempt_id for q in job.queries for a in q.attempts if a.attempt_id]
            elif kind == "retry":
                models._reset_retryable_queries(job)
            else:
                raise ValueError(kind)
            outputs.append({"job": models._job_to_payload(job), "summary": budget.budget_summary(job)})
        cases.append(
            {"name": name, "input": copy.deepcopy(data), "initial": initial, "operations": ops, "outputs": outputs}
        )

    init = {"kind": "initialize", "policy": [4, 2, 3]}
    reserve = {"kind": "reserve", "now": 100.0}
    record("legacy-defaults", base, [init])
    record(
        "charged-duplicates-and-capacity",
        base,
        [
            init,
            reserve,
            {
                "kind": "finish",
                "now": 101.0,
                "outcome": {
                    "state": "done",
                    "cursor": "snap:雪",
                    "query_id": "query-id",
                    "result_count": 9,
                    "enforcement": {"language": {"status": "unsupported"}},
                },
                "urls": [
                    "https://e.test/a?utm_source=x",
                    "https://e.test/a",
                    "HTTPS://e.test/a?x=1&x=2",
                    "https://e.test/b",
                ],
            },
        ],
    )
    record(
        "unknown-crash-retry-conservation",
        base,
        [
            init,
            reserve,
            {"kind": "recover", "now": 105.0},
            reserve,
            {"kind": "finish", "outcome": {"state": "failed", "error": "timeout", "result_count": 0}, "urls": None},
            {"kind": "retry"},
            {"kind": "initialize", "policy": [2, 1, 1]},
            reserve,
        ],
    )
    record(
        "reserve-interrupts-before-budget-rejection",
        base,
        [{"kind": "initialize", "policy": [1, 2, 1]}, reserve, reserve],
    )
    record(
        "result-exhaustion",
        base,
        [
            init,
            reserve,
            {
                "kind": "finish",
                "outcome": {"state": "done", "cursor": "snap", "result_count": 3},
                "urls": ["a", "b", "c"],
            },
            reserve,
        ],
    )
    record("engine-limit", base, [{"kind": "initialize", "policy": [4, 1, 4]}, reserve])
    legacy = copy.deepcopy(base)
    legacy["queries"][0].update(state="running", result_count=2)
    record("legacy-orphan-before-accounting", legacy, [{"kind": "recover", "now": 77.0}, init])
    legacy["queries"].append(
        {
            "index": "1",
            "query": "failed",
            "intent": "general",
            "engines": ["a"],
            "state": "failed",
            "result_count": "3",
            "attempts": [{"result_count": "2", "cursor": "old-snapshot", "state": "done", "attempted_at": "5.25"}],
        }
    )
    record("legacy-attempts-provenance-coercion", legacy, [init])
    empty = copy.deepcopy(base)
    empty.update(
        workflow={"kind": "dependency_dossier", "provenance": {"n": 9007199254740993, "null": None}},
        completion_subquestion_states=None,
        warnings=None,
    )
    empty["queries"][0].update(attempts=None, engine_coverage=None, enforcement=None)
    record("nullable-empty-and-lossless-provenance", empty, [init])
    terminal = copy.deepcopy(base)
    terminal["state"] = "cancelled"
    terminal["queries"][0].update(state="failed", error="old")
    record("retry-does-not-resurrect-cancelled", terminal, [{"kind": "retry"}])
    coverage_cases = []
    for dispatched in [False, True]:
        for credential_missing in [False, True]:
            for status in [None, "ok", "rate_limited", "blocked", "error", "timeout", "unavailable"]:
                for count in [0, 2]:
                    params = {
                        "engine": "synthetic",
                        "dispatched": dispatched,
                        "status": status,
                        "result_count": count,
                        "credential_missing": credential_missing,
                    }
                    cov = models.classify_coverage(**params)
                    import dataclasses

                    coverage_cases.append(
                        {
                            "input": params,
                            "coverage": dataclasses.asdict(cov),
                            "summary": models.summarize_coverage([cov]).as_dict(),
                        }
                    )
    output = {
        "schema_version": 1,
        "plan_id": "W01",
        "baseline": BASELINE,
        "python": sys.version.split()[0],
        "sources": sources,
        "exporter_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "cases": cases,
        "coverage_cases": coverage_cases,
    }
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    main()
