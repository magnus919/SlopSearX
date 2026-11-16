import asyncio
import dataclasses
import hashlib
import json
import pathlib
import sys

import httpx

root = pathlib.Path("/Users/magnus/.codex/worktrees/rust-a04-broad-api-adapters/SlopSearX")
sys.path.insert(0, str(root))
from engines.europepmc import EuropePMCAdapter  # noqa: E402
from engines.pubmed import PubMedAdapter  # noqa: E402

cases = []
search = {"esearchresult": {"idlist": ["123"]}}
article = {
    "title": "title",
    "source": "Journal",
    "authors": [{"name": "one"}, {"name": "two"}, {"name": "three"}, {"name": "\ud800"}, {"name": "🙂"}],
}
cases.append(
    {"id": "pm-raw-author-after-display", "engine": "pubmed", "bodies": [search, {"result": {"123": article}}]}
)
cases.append(
    {
        "id": "ep-raw-payload-only-surrogate",
        "engine": "europepmc",
        "bodies": [
            {
                "resultList": {
                    "result": [
                        {"id": "1", "source": "MED", "title": "title", "doi": {"\ud800": ["\ud800", None, "🙂"]}}
                    ]
                }
            }
        ],
    }
)
cases.append(
    {
        "id": "pm-later-source-error-before-nonfinite",
        "engine": "pubmed",
        "bodies": [
            {"esearchresult": {"idlist": ["123", "456"]}},
            {
                "result": {
                    "123": {"title": "first", "articleids": [{"idtype": "doi", "value": float("nan")}]},
                    "456": {"title": []},
                }
            },
        ],
    }
)
cases.append(
    {
        "id": "ep-later-source-error-before-nonfinite",
        "engine": "europepmc",
        "bodies": [
            {"resultList": {"result": [{"id": "1", "source": "MED", "title": "first", "doi": float("nan")}, []]}}
        ],
    }
)


def safe(v):
    if isinstance(v, set):
        return sorted(v)
    if isinstance(v, dict):
        return {k: safe(x) for k, x in v.items()}
    if isinstance(v, (tuple, list)):
        return [safe(x) for x in v]
    return v


async def main():
    rows = []
    for c in cases:
        requests = []
        wire_bodies = []
        bodies = iter(c["bodies"])

        def handle(r):
            requests.append({"method": r.method, "url": str(r.url)})
            wire = json.dumps(next(bodies), ensure_ascii=True, allow_nan=True)
            wire_bodies.append(wire)
            return httpx.Response(200, content=wire.encode())

        a = {"pubmed": PubMedAdapter, "europepmc": EuropePMCAdapter}[c["engine"]]({})
        a.set_http_transport(httpx.MockTransport(handle))
        try:
            result = await a.search("test")
            output = safe(dataclasses.asdict(result))
            output["status"] = result.status.value
            output.pop("latency_ms", None)
        finally:
            await a.shutdown()
        rows.append(
            {
                "id": c["id"],
                "engine": c["engine"],
                "query": "test",
                "config": {},
                "wire_response_bodies": wire_bodies,
                "requests": requests,
                "output": output,
            }
        )
    sources = ["engines/pubmed.py", "engines/europepmc.py", "slopsearx/adapter.py", "slopsearx/payload.py"]
    out = {
        "status": (
            "actual original adapters; isolated HTTPX MockTransport; no sockets/provider calls; "
            "temporary historical evidence pending inventory registration"
        ),
        "python": sys.version,
        "httpx": httpx.__version__,
        "sources": {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in sources},
        "cases": rows,
    }
    p = pathlib.Path("/private/tmp/a07-payload-adapter-controls.json")
    p.write_text(json.dumps(out, ensure_ascii=True, allow_nan=False, indent=2) + "\n")
    print([(x["id"], x["output"]["status"], x["output"]["error_message"]) for x in rows])


asyncio.run(main())
