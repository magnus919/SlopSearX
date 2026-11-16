"""Temporary F12 offline oracle; G08 retires it. Never called by native gates."""

import asyncio
import dataclasses
import importlib.metadata
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace as N

import httpx

from slopsearx.cache import SearchCache
from slopsearx.jev import MODEL, JevRoutingDecision, JevSpecialistRouter
from slopsearx.router import QueryRouter
from slopsearx.routing import RoutingBudget
from slopsearx.service import AppContext, SearchRequest, SearchService

if sys.version_info[:3] != (3, 14, 7):
    raise RuntimeError("F12 requires Python3.14.7")
if importlib.metadata.version("mcp") != "2.3.0" or importlib.metadata.version("fastmcp") != "4.0.11":
    raise RuntimeError("F12 requires MCP2.3.0/FastMCP4.0.11")
ROOT = Path(__file__).resolve().parents[4]
SOURCES = [
    "slopsearx/jev.py",
    "slopsearx/service.py",
    "slopsearx/routing.py",
    "slopsearx/router.py",
    "tests/test_jev.py",
    "tests/test_routing_eval.py",
]
PINS = json.loads((ROOT / "docs/rust-rewrite/f12/source-pins.json").read_text())
for pin in PINS["sources"]:
    actual = subprocess.check_output(["git", "hash-object", "--", pin["path"]], cwd=ROOT, text=True).strip()
    if actual != pin["git_blob"]:
        raise RuntimeError("F12 source drift; independent review required")
real_client = httpx.AsyncClient


# All sends use this injected offline transport; outbound DNS/socket cannot occur.
def engine(n):
    catalog = json.loads((Path.cwd() / "docs/rust-rewrite/f06/catalog.json").read_text())
    display = next(e["display_name"] for e in catalog if e["name"] == n)
    return N(name=n, display_name=display, categories=["science"], circuit_open=False)


class Cache:
    def __init__(self, value=None):
        self.value = value
        self.writes = []
        self.keys = []

    async def get(self, key):
        self.keys.append(key)
        return self.value

    async def set(self, key, value, ttl):
        self.writes.append(dict(key=key, value=value, ttl=ttl))


async def capture():
    cases = []
    for name, answer, status in [
        ("threshold", 0.65, 200),
        ("below", 0.649, 200),
        ("bool", True, 200),
        ("false", False, 200),
        ("above", 1.01, 200),
        ("negative", -0.1, 200),
        ("missing", None, 200),
        ("string", "0.9", 200),
        ("wrong_model", 0.9, 200),
        ("http_error", 0.9, 503),
        ("tie", 0.9, 200),
    ]:
        calls = []

        def handle(req):
            calls.append(
                dict(
                    method=req.method,
                    url=str(req.url),
                    authorization=req.headers["authorization"],
                    body=req.content.decode(),
                )
            )
            return httpx.Response(
                status,
                json={
                    "model": "other" if name == "wrong_model" else MODEL,
                    "answers": {n: {"noul": answer} for n in ["arxiv", "pubmed"]},
                },
            )

        httpx.AsyncClient = lambda **kw: real_client(transport=httpx.MockTransport(handle), **kw)
        cache = Cache()
        router = JevSpecialistRouter("synthetic")
        d = await router.route(" café 🐙 ", {n: engine(n) for n in ["pubmed", "arxiv"]}, None, set(), cache)
        cases.append(
            dict(
                name=name,
                answer=answer,
                status=status,
                decision=None if d is None else dict(engines=d.engines, scores=d.scores),
                requests=calls,
                cache_keys=cache.keys,
                cache_writes=cache.writes,
            )
        )
    httpx.AsyncClient = real_client
    configs = []
    for threshold, timeout in [
        ("0.65", "1000"),
        ("nan", "1000"),
        ("inf", "1000"),
        ("-inf", "1000"),
        ("bad", "50"),
        ("1_0", "5_0"),
        ("0.3", "bad"),
        (" 0.7 ", "-5"),
    ]:
        os.environ.update(TYPESAFE_API_KEY=" synthetic ", JEV_SPECIALIST_THRESHOLD=threshold, JEV_TIMEOUT_MS=timeout)
        r = JevSpecialistRouter.from_environment()
        configs.append(
            dict(
                threshold_input=threshold,
                timeout_input=timeout,
                threshold=r.threshold,
                timeout=r.timeout,
                identity=r.cache_identity(),
            )
        )

    class Catalog:
        def get(self, name):
            return N(
                enabled=True,
                auth_class="none",
                auth_configured=True,
                cost_class="free",
                last_known_status="ok",
                last_known_status_stale=False,
                circuit_open=False,
            )

    class Advice:
        def __init__(self, result):
            self.result = result
            self.calls = 0

        async def route(self, *args, **kwargs):
            self.calls += 1
            return self.result

        def cache_identity(self):
            return "fixture"

    scopes = []
    for name, result, engines, budget in [
        ("keyless", None, None, None),
        ("enhanced", JevRoutingDecision(["arxiv", "pubmed"], {"arxiv": 0.9, "pubmed": 0.9}, 0), None, None),
        ("failure", None, None, None),
        ("explicit", JevRoutingDecision(["pubmed"], {"pubmed": 0.9}, 0), ["brave"], None),
        (
            "budget_bypass",
            JevRoutingDecision(["arxiv", "pubmed"], {"arxiv": 0.9, "pubmed": 0.9}, 0),
            None,
            RoutingBudget(max_engines=1, max_cost_class="free"),
        ),
    ]:
        advice = None if name == "keyless" else Advice(result)
        ctx = AppContext(
            active_engines={n: engine(n) for n in ["brave", "arxiv", "pubmed"]},
            catalog=Catalog(),
            router=QueryRouter(),
            jev_router=advice,
            routing_budget=budget,
        )
        scope = await SearchService(ctx).resolve_scope(SearchRequest(query=" café 🐙 ", engines=engines))
        scopes.append(
            dict(name=name, scope=dataclasses.asdict(scope), advice_calls=0 if advice is None else advice.calls)
        )
    return dict(
        scopes=scopes,
        schema_version=1,
        plan_id="F12",
        baseline=PINS["baseline"],
        python=sys.version.split()[0],
        mcp="2.3.0",
        fastmcp="4.0.11",
        sources=[
            dict(path=p, git_blob=subprocess.check_output(["git", "hash-object", "--", p], text=True).strip())
            for p in SOURCES
        ],
        cases=cases,
        configs=configs,
    )


async def capture_overflow():
    valkey_server = os.environ.get("SLOPSEARX_TEST_VALKEY_SERVER")
    if not valkey_server:
        raise RuntimeError("Set SLOPSEARX_TEST_VALKEY_SERVER to the pinned disposable Valkey executable")
    cache_blob = subprocess.check_output(["git", "hash-object", "--", "slopsearx/cache.py"], text=True).strip()
    if cache_blob != "354dbc179c4ca6ebc4ac9f85dd66a40d07ba3c09":
        raise RuntimeError("F12 overflow cache source drift")
    rows = []
    for value in ["100000000000000000000", "9" * 400, "-100000000000000000000", "-0", "１２３"]:
        os.environ.update(TYPESAFE_API_KEY="synthetic", JEV_SPECIALIST_THRESHOLD="-0.0", JEV_TIMEOUT_MS=value)
        try:
            r = JevSpecialistRouter.from_environment()
            rows.append(
                dict(
                    field="timeout",
                    input=value,
                    threshold=r.threshold,
                    timeout_repr=repr(r.timeout),
                    identity=r.cache_identity(),
                    error=None,
                )
            )
        except Exception as e:
            rows.append(dict(field="timeout", input=value, error=type(e).__name__))
    with tempfile.TemporaryDirectory(prefix="f12-source-valkey-") as d:
        sock = Path(d) / "s.sock"
        p = subprocess.Popen(
            [
                valkey_server,
                "--port",
                "0",
                "--save",
                "",
                "--appendonly",
                "no",
                "--unixsocket",
                str(sock),
                "--dir",
                d,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            for _ in range(100):
                if sock.exists():
                    break
                await asyncio.sleep(0.01)
            real = httpx.AsyncClient
            count = 0

            def handle(req):
                nonlocal count
                count += 1
                return httpx.Response(200, json={"model": MODEL, "answers": {"pubmed": {"noul": 0.9}}})

            httpx.AsyncClient = lambda **kw: real(transport=httpx.MockTransport(handle), **kw)
            for value in ["100000000000000000000", "9" * 400, "bad", "-100000000000000000000"]:
                os.environ["JEV_ROUTING_CACHE_TTL_SECONDS"] = value
                cache = SearchCache("unix://" + str(sock))
                r = JevSpecialistRouter("synthetic")
                before = count
                dec = await r.route(
                    "synthetic",
                    {"pubmed": N(display_name="PubMed", categories=["science"], circuit_open=False)},
                    None,
                    set(),
                    cache,
                )
                cached = await cache.get(r._decision_cache_key("synthetic", ["pubmed"]))
                rows.append(
                    dict(
                        field="ttl",
                        input=value,
                        decision=None if dec is None else dict(engines=dec.engines, scores=dec.scores),
                        provider_calls=count - before,
                        cached=cached,
                    )
                )
                await cache.close()
        finally:
            p.terminate()
            p.wait()
    return dict(
        python=sys.version.split()[0],
        baseline=PINS["baseline"],
        cache_source=dict(path="slopsearx/cache.py", git_blob=cache_blob),
        cases=rows,
    )


if __name__ == "__main__":
    os.chdir(ROOT)
    overflow = len(sys.argv) > 1 and sys.argv[1] == "--overflow"
    out = asyncio.run(capture_overflow() if overflow else capture())
    argument = sys.argv[2:] if overflow else sys.argv[1:]
    default = "overflow.json" if overflow else "source.json"
    path = Path(argument[0]) if argument else ROOT / "crates/xtask/fixtures/f12" / default
    path.write_text(json.dumps(out, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
