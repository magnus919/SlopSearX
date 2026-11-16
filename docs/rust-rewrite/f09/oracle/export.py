"""F09 temporary offline reference capture; G08 retires this executable.

Run only with pinned Python 3.14.7 and PYTHONPATH pointing at the reviewed source.
This writes JSON observations; it is never imported by native runtime or gates.
"""

import asyncio
import dataclasses
import datetime
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace

import httpx

from slopsearx.capabilities import INTENT_PROFILES, MCPPolicy, engine_policy_rejection
from slopsearx.config import load_config
from slopsearx.filters import (
    publication_date_bounds,
    publication_date_in_bounds,
    published_date_within,
    resolve_filter_enforcement,
)
from slopsearx.mcp import tools
from slopsearx.mcp.tools import EVIDENCE_TYPE_ENGINES, SOURCE_TYPE_ENGINES
from slopsearx.research_models import ResearchQuery
from slopsearx.router import _DEFAULT_TOPICS, QueryRouter
from slopsearx.routing import RoutingBudget
from slopsearx.service import ScopeResolver, SearchRequest


def deny_provider_call(*_args, **_kwargs):
    raise RuntimeError("F09 reference capture cannot dispatch HTTP")


httpx.Client.send = deny_provider_call
httpx.AsyncClient.send = deny_provider_call


if sys.version_info[:3] != (3, 14, 7):
    raise RuntimeError("F09 reference interpreter must be Python 3.14.7")
_pins = json.loads(Path("docs/rust-rewrite/f09/source-pins.json").read_text())
for _pin in _pins["sources"]:
    _actual = subprocess.check_output(["git", "hash-object", "--", _pin["path"]], text=True).strip()
    if _actual != _pin["git_blob"]:
        raise RuntimeError("F09 pinned reference source differs")


p = Path("docs/rust-rewrite/f09")
p.mkdir(exist_ok=True)
data = {
    "intents": {k: dataclasses.asdict(v) for k, v in INTENT_PROFILES.items()},
    "security": EVIDENCE_TYPE_ENGINES,
    "science": SOURCE_TYPE_ENGINES,
    "topics": _DEFAULT_TOPICS,
}
(p / "profiles.json").write_text(json.dumps(data, sort_keys=True, indent=2) + "\n")


class Catalog:
    def __init__(self, engines):
        self.engines = {
            e["name"]: SimpleNamespace(
                **dict(
                    e,
                    auth_class="required" if not e["auth_ready"] else "none",
                    auth_configured=e["auth_ready"],
                    last_known_status=e["health_status"],
                    last_known_status_stale=e["health_stale"],
                )
            )
            for e in engines
        }

    def get(self, name):
        return self.engines.get(name)

    def known_names(self):
        return set(self.engines)


def engine(name, **kw):
    d = dict(
        name=name,
        enabled=True,
        active=True,
        available=True,
        categories=["general"],
        media_types=[],
        auth_ready=True,
        circuit_open=False,
        cost_class="free",
        health_status="unknown",
        health_stale=False,
        enforced_filters={},
    )
    d.update(kw)
    return d


engines = [
    engine(
        "alpha",
        media_types=["image"],
        enforced_filters={"time_range": "local", "language": "local", "pagination": "upstream"},
    ),
    engine(
        "beta",
        categories=["science"],
        enforced_filters={
            "time_range": "upstream",
            "safesearch": "upstream",
            "date_from": "upstream",
            "date_to": "upstream",
        },
    ),
    engine("paid", cost_class="paid"),
    engine("auth", auth_ready=False),
    engine("open", circuit_open=True),
    engine("hibp", categories=["general", "security"]),
    engine("disabled", enabled=False, active=False),
]
active = {
    e["name"]: SimpleNamespace(
        categories=e["categories"], supported_media_types=e["media_types"], enforced_filters=e["enforced_filters"]
    )
    for e in engines
    if e["active"]
}
cat = Catalog(engines)
scopes = []


def capture(name, request, topics=None, budget=None, catalog=True):
    router = None if topics is None else QueryRouter({"topics": topics})
    rr = ScopeResolver(
        active,
        router,
        tier1_engines={"alpha", "beta"},
        catalog=cat if catalog else None,
        budget=RoutingBudget(**budget) if budget else None,
    )
    expected = dataclasses.asdict(rr.resolve(SearchRequest(**{"query": "query", **request})))
    if expected["routing_rule"] == "tier-1 fallback":
        expected["selected_engines"].sort()
    scopes.append(
        dict(
            name=name,
            request={"query": "query", **request},
            topics=topics,
            budget=budget,
            catalog_routing=catalog,
            expected=expected,
        )
    )


for name, req in [
    ("all", {}),
    ("explicit", {"engines": ["paid", "auth", "open", "hibp", "unknown", "alpha", "alpha"]}),
    ("explicit-whitespace", {"engines": ["\x1calpha\x1f", " ", "unknown"]}),
    ("categories-or", {"categories": ["general", "science"]}),
    ("category-whitespace", {"categories": ["\x1cscience\x1f", " "]}),
    ("category-empty", {"categories": [" "]}),
    ("media", {"media_type": "image"}),
    ("media-unknown", {"media_type": "audio"}),
    ("media-conflict", {"engines": ["beta"], "media_type": "image"}),
    ("empty-category", {"categories": ["absent"]}),
]:
    capture(name, req)
capture("router-disabled", {}, [])
for query in ["BETA topic", "İΣ\x1c", "\x1fSTRASSE\x1c", "ABC"]:
    capture(
        "topic-" + repr(query),
        {"query": query},
        [
            {"name": "first", "keywords": ["beta", "i̇σ", "strasse", "abc"], "engines": ["beta", "alpha", "missing"]},
            {"name": "second", "keywords": ["beta"], "engines": ["paid"]},
        ],
    )
capture("budget", {}, budget={"max_engines": 1, "max_cost_class": "free", "coverage_target": 2})
capture("no-catalog", {}, catalog=False)
filters = []
for selected in [[], ["alpha"], ["beta"], ["alpha", "beta"], ["alpha", "alpha", "beta"], ["missing"]]:
    for field, requested in [
        ("language", "en"),
        ("time_range", "week"),
        ("time_range", "all"),
        ("safesearch", "strict"),
        ("pagination", 2),
        ("date_from", "2026-01-01"),
    ]:
        filters.append(
            dict(
                selected=selected,
                field=field,
                requested=requested,
                expected=resolve_filter_enforcement(selected, field, requested, active),
            )
        )
policy = []
for names in [["alpha"], ["unknown", "disabled", "hibp"], ["alpha", "hibp"], ["hibp", "hibp"], [], ["disabled"]]:
    for allowed in [False, True]:
        p = MCPPolicy(targeted_sensitive_allowed=allowed)
        policy.append(dict(names=names, allowed=allowed, expected=engine_policy_rejection(cat, p, names)))
dates = []
for start, end in [
    (None, None),
    ("2024-02-29", "2024-03-01"),
    ("2023-02-29", None),
    ("20240101", None),
    ("2026-02-02", "2026-01-01"),
]:
    try:
        a, b = publication_date_bounds(start, end)
        expected = {
            "contains": [
                publication_date_in_bounds(s, a, b) for s in [None, "2024-02-29", "2024-03-01", "20240101", "bad"]
            ]
        }
    except Exception as e:
        expected = {"error": str(e), "field": e.field}
    dates.append(dict(start=start, end=end, expected=expected))
relative = []
today = datetime.date(2024, 3, 1)
for value in [
    None,
    "2024-03-01T12:00:00Z",
    "20240229",
    "2024-W09-4",
    "2024W094",
    "2024-W09",
    "2024-02-22",
    "2024-02-21",
    "bad",
]:
    relative.append(
        dict(value=value, range="week", today="2024-03-01", expected=published_date_within(value, "week", today))
    )
data = dict(
    schema_version=1,
    plan_id="F09",
    baseline="1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5",
    engines=engines,
    scopes=scopes,
    filters=filters,
    policy=policy,
    dates=dates,
    relative=relative,
)
p = Path("crates/xtask/fixtures/f09")
p.mkdir(exist_ok=True)
(p / "oracle.json").write_text(json.dumps(data, sort_keys=True, indent=2, ensure_ascii=False) + "\n")


cases = [
    (
        "named",
        """routing:
  topics:
    zebra: {keywords: [overlap], engines: [wikipedia]}
    alpha: {keywords: [overlap], engines: [brave]}
""",
    ),
    (
        "duplicate",
        """routing:
  topics:
    zebra: {keywords: [overlap], engines: [wikipedia]}
    alpha: {keywords: [overlap], engines: [brave]}
    zebra: {keywords: [overlap], engines: [reddit]}
""",
    ),
    (
        "alias",
        """topics: &topics
  zebra: {keywords: [overlap], engines: [wikipedia]}
  alpha: {keywords: [overlap], engines: [brave]}
routing: {topics: *topics}
""",
    ),
    (
        "merge",
        """a: &a
  zebra: {keywords: [overlap], engines: [wikipedia]}
  common: {keywords: [overlap], engines: [reddit]}
b: &b
  alpha: {keywords: [overlap], engines: [brave]}
  common: {keywords: [overlap], engines: [google]}
routing:
  topics:
    <<: [*a, *b]
    alpha: {keywords: [overlap], engines: [stackexchange]}
""",
    ),
    (
        "list",
        """routing:
  topics:
    - {name: zebra, keywords: [overlap], engines: [wikipedia]}
    - {name: alpha, keywords: [overlap], engines: [brave]}
""",
    ),
]
out = []
for name, yaml in cases:
    fd, path = tempfile.mkstemp(suffix=".yaml")
    os.close(fd)
    try:
        open(path, "w").write(yaml)
        c = load_config(path)
        router = QueryRouter({"topics": c.routing.topics})
        out.append(
            dict(
                name=name,
                yaml=yaml,
                order=list(c.routing.topics) if isinstance(c.routing.topics, dict) else [],
                selected=router.route("overlap"),
                topic=router.match_topic("overlap"),
            )
        )
    finally:
        os.unlink(path)
open("crates/xtask/fixtures/f09/topic-order.json", "w").write(
    json.dumps(dict(schema_version=1, plan_id="F09", cases=out), indent=2) + "\n"
)

# Actual public tool denial paths and worker dispatch policy, still no dispatch.


async def capture_surface_denials():
    output = []
    for surface, call in [
        ("generic", lambda: tools.slopsearx_search("query", engines=["alpha", "hibp"])),
        ("targeted", lambda: tools.slopsearx_search_targeted("query", ["alpha", "hibp"])),
        ("preview", lambda: tools.slopsearx_explain_search_scope("query", engines=["alpha", "hibp"])),
        ("jobs", lambda: tools.slopsearx_search_jobs("synthetic company")),
        ("security", lambda: tools.slopsearx_search_security("query")),
        ("science", lambda: tools.slopsearx_search_science("query")),
        ("research", lambda: tools.slopsearx_start_research("query")),
    ]:
        tools.get_state = lambda: SimpleNamespace(
            policy=MCPPolicy(), catalog=cat, ctx=SimpleNamespace(active_engines=active)
        )
        output.append({"surface": surface, "expected": await call()})
    return output


worker_cases = []
base_query = dict(index=0, query="query", intent="science", engines=["alpha"], requires_intent_grant=True)
for name, grants, changes in [
    ("research-revoked", [], {}),
    ("intent-revoked", ["research"], {}),
    ("legacy-no-intent-grant", ["research"], {"requires_intent_grant": False}),
    (
        "captured-intent-revoked",
        ["research"],
        {
            "intent": "web",
            "evidence_result_ids": ["r"],
            "parent_attempt_id": "a",
            "rationale": "why",
            "evidence_engines": ["beta"],
            "evidence_intent": "science",
        },
    ),
    (
        "captured-sensitive",
        ["research", "science"],
        {"evidence_result_ids": ["r"], "parent_attempt_id": "a", "rationale": "why", "evidence_engines": ["hibp"]},
    ),
    (
        "captured-empty",
        ["research", "science"],
        {"evidence_result_ids": ["r"], "parent_attempt_id": "a", "rationale": "why"},
    ),
    ("metadata-missing", ["research", "science"], {"evidence_result_ids": ["r"]}),
    ("no-engines", ["research", "science"], {"engines": []}),
]:
    kwargs = dict(base_query, **changes)
    policy_state = SimpleNamespace(policy=MCPPolicy(enabled_tools={name: True for name in grants}), catalog=cat)
    expected = tools._research_dispatch_error(policy_state, ResearchQuery(**kwargs), check_evidence=True)
    worker_cases.append({"name": name, "grants": grants, "query": kwargs, "expected": expected})
Path("crates/xtask/fixtures/f09/authority.json").write_text(
    json.dumps(
        {
            "schema_version": 1,
            "plan_id": "F09",
            "surface_denials": asyncio.run(capture_surface_denials()),
            "workers": worker_cases,
        },
        sort_keys=True,
        indent=2,
    )
    + "\n"
)
