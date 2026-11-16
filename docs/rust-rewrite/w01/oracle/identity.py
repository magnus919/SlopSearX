"""Developer-only W01 lead identity capture; never a runtime/native gate dependency."""

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

BASELINE = "cfd7a0af61a22cfd040a7fcd41ad950f82005959"
SOURCES = ["slopsearx/research_budget.py", "slopsearx/research_models.py", "slopsearx/merger.py"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--f11-fixtures", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 14, 7):
        raise SystemExit("Python3.14.7 required")
    root = args.source_root.resolve()
    sources = []
    for name in SOURCES:
        raw = subprocess.check_output(["git", "show", f"{BASELINE}:{name}"], cwd=root)
        if (root / name).read_bytes() != raw:
            raise SystemExit(f"source drift: {name}")
        sources.append(
            {
                "path": name,
                "sha256": hashlib.sha256(raw).hexdigest(),
                "git_blob": subprocess.check_output(
                    ["git", "rev-parse", f"{BASELINE}:{name}"], cwd=root, text=True
                ).strip(),
            }
        )
    sys.path.insert(0, str(root))
    from slopsearx import research_budget as budget
    from slopsearx import research_models as models
    from slopsearx.merger import _normalise_url

    budget.time = SimpleNamespace(time=lambda: 100.0)
    fixtures = []
    urls = []
    for path in sorted(args.f11_fixtures.glob("goldens-*.jsonl")):
        raw = path.read_bytes()
        fixtures.append({"path": path.name, "sha256": hashlib.sha256(raw).hexdigest()})
        for line in raw.splitlines():
            row = json.loads(line)
            if row["kind"] == "url":
                urls.append(row["input"])

    def capture(urls, cursor="snap", cap=8, used=0, seen=None):
        data = {
            "job_id": "job-identity",
            "tenant": "org:team",
            "question": "identity",
            "created_at": 1,
            "deadline": 1000,
            "state": "running",
            "budget_limits": {
                "queries": 1,
                "engines_per_query": 1,
                "attempts": 2,
                "engine_attempts": 2,
                "results": cap,
            },
            "budget_used": {"attempts": 1, "engine_attempts": 1, "results": used},
            "seen_lead_ids": seen or [],
            "queries": [
                {
                    "index": 0,
                    "query": "q",
                    "engines": ["fixture"],
                    "state": "done",
                    "cursor": cursor,
                    "result_count": len(urls or []),
                    "attempts": [{"attempt_id": "attempt-identity", "state": "running", "attempted_at": 1}],
                }
            ],
        }
        job = models._job_from_payload(data)
        initial = models._job_to_payload(job)
        response = None if urls is None else SimpleNamespace(results=[SimpleNamespace(url=url) for url in urls])
        budget.finish_attempt(job, job.queries[0], response)
        return {"input": initial, "urls": urls, "expected": models._job_to_payload(job)}

    identities = []
    for url in urls:
        captured = capture([url])
        identities.append(
            {"url": url, "normalized": _normalise_url(url), "expected": captured["expected"]["seen_lead_ids"][0]}
        )
    pair = ["https://example.com/p?utm_source=x", "https://example.com/p", "https://other.test/雪"]
    seen = capture([pair[0]])["expected"]["seen_lead_ids"]
    settlements = []
    for name, arguments in [
        ("duplicates", {"urls": pair}),
        ("ceiling", {"urls": pair, "cap": 2}),
        ("prior-seen", {"urls": pair, "seen": seen}),
        ("no-response", {"urls": None}),
        ("no-cursor", {"urls": pair, "cursor": None}),
        ("empty-cursor", {"urls": pair, "cursor": ""}),
        ("no-results", {"urls": []}),
        ("already-exhausted", {"urls": pair, "cap": 1, "used": 1}),
        ("lowered-cap", {"urls": pair, "cap": 0, "used": 2}),
    ]:
        settlements.append({"name": name, **capture(**arguments)})
    artifact = {
        "schema_version": 1,
        "baseline": BASELINE,
        "python": "3.14.7",
        "sources": sources,
        "f11_fixtures": fixtures,
        "exporter_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "identities": identities,
        "settlements": settlements,
    }
    args.output.write_text(json.dumps(artifact, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
