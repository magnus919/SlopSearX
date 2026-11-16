"""Temporary isolated migration oracle; retire with G08, never a native gate dependency."""

import copy
import hashlib
import json
from pathlib import Path

from slopsearx.adapter import SearchResult
from slopsearx.saved_models import SavedDefinition, compare_observations, definition_to_payload, observe
from slopsearx.service import EngineOutcome, ScopeDecision, SearchResponse


def result(url, title="title", **kw):
    return dict(url=url, title=title, content="source text", engine="wikipedia", **kw)


def response(rows, **kw):
    scope = kw.pop("scope", ["wikipedia"])
    status = kw.pop("status", "ok")
    return SearchResponse(
        "query",
        [SearchResult(**r) for r in rows],
        ScopeDecision(selected_engines=scope),
        [EngineOutcome("wikipedia", status, len(rows))],
        query_id="q-fixed",
        **kw,
    )


def comparison(d, r, now):
    observation = observe(d, r, now)
    report, baseline = compare_observations(
        d, observation, run_id="run-golden", scheduled_at=now, started_at=now, finished_at=now, missed_slots=2
    )
    return observation, report, baseline


base = SavedDefinition("saved", "tenant", "query", ["wikipedia"], 60, 3600, 100, 20, 0.0, 3600.0, 0.0)
_, _, base.baseline = comparison(base, response([result("https://a.test"), result("https://b.test")]), 10.0)
cases = []
variants = [
    ("first", [result("https://a.test")], {}, {"baseline": None}),
    ("changes", [result("https://a.test", "new"), result("https://c.test")], {}, {}),
    ("ranking", [result("https://b.test", score=200, position=10, tier=2), result("https://a.test")], {}, {}),
    ("outage", [], {"status": "timeout"}, {}),
    ("cached", [], {"cached": True}, {}),
    ("partial", [], {"partial": True}, {}),
    ("deadline", [], {"deadline_exceeded": True}, {}),
    ("unresponsive", [], {"all_unresponsive": True}, {}),
    ("scope", [], {"scope": ["brave"]}, {}),
    ("truncated", [result("https://a.test"), result("https://b.test")], {}, {"max_results": 1}),
    ("provenance", [dict(url="https://a.test", title="title", content="source text", engine="brave")], {}, {}),
    ("ambiguous", [result("https://a.test/"), result("https://a.test", "different")], {}, {}),
    ("utf8_oversize", [result("https://a.test", "雪" * 2731)], {}, {}),
    ("empty_url", [result("")], {}, {}),
    ("unicode", [result("https://雪.test/😀", "雪😀")], {}, {}),
    ("expired", [], {}, {}),
    ("window", [], {}, {"query": "changed"}),
]
variants.extend(
    [
        ("provenance_order", [result("https://a.test", engines=["wikipedia", "brave"])], {}, {}),
        ("provenance_set_changed", [result("https://a.test", engines=["brave", "wikipedia"])], {}, {}),
        ("published_date_only", [result("https://a.test", published_date="2026-10-16")], {}, {}),
        ("published_date_empty", [result("https://a.test", published_date="")], {}, {}),
    ]
)
for name, rows, kwargs, changes in variants:
    d = copy.deepcopy(base)
    for key, value in changes.items():
        setattr(d, key, value)
    if name == "provenance_order":
        for row in d.baseline["records"].values():
            row["engines"] = ["brave", "wikipedia"]
    if name == "expired":
        d.baseline["expires_at"] = 15.0
    r = response(rows, **kwargs)
    observation, report, baseline = comparison(d, r, 20.0)
    cases.append(
        dict(
            name=name,
            definition=definition_to_payload(d),
            rows=rows,
            response=kwargs,
            observation=observation,
            report=report,
            replacement=baseline,
            report_spaced_bytes=len(json.dumps(report).encode()),
            report_spaced_sha256=hashlib.sha256(json.dumps(report, sort_keys=True).encode()).hexdigest(),
        )
    )
root = Path(__file__).resolve().parents[3]
sources = [
    "slopsearx/saved_models.py",
    "slopsearx/saved_runner.py",
    "slopsearx/saved_store.py",
    "tests/test_saved_models.py",
    "tests/test_saved_searches.py",
    "tests/test_saved_valkey.py",
]
value = dict(
    baseline="1bad48bbe4e25e2f60d5b86668e92bbce922fdf0",
    sources={p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in sources},
    cases=cases,
)
Path(__file__).with_name("source.json").write_text(json.dumps(value, ensure_ascii=True, indent=2) + "\n")
