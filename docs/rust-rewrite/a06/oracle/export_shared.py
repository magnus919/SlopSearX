"""Additional inspected shared-test inputs; temporary offline A06 source oracle."""

import asyncio
import hashlib
import json
import runpy
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))
ROOT = BASE["ROOT"]


def cases():
    reply, feed = BASE["reply"], BASE["FEED"]
    rows = []

    def add(engine, name, body=None, **kwargs):
        rows.append(
            {
                "id": engine + ".shared." + name,
                "engine": engine,
                "query": "transformer",
                "responses": kwargs.pop("responses", [reply(body)]),
                **kwargs,
            }
        )

    for name, response in [("upgrade_success", reply(feed)), ("upgrade_rate_limit", reply("", 429))]:
        add(
            "arxiv",
            name,
            config={"base_url": "http://export.arxiv.org/api/query"},
            responses=[
                reply("", 301, {"Location": "https://export.arxiv.org/api/query"}),
                response,
            ],
        )
    add("arxiv", "external_redirect", responses=[reply("", 301, {"Location": "https://example.com/redirect"})])
    for name, abstract in [
        ("abstract_none", None),
        ("abstract_empty", {}),
        ("abstract_simple", {"hello": [0], "world": [1]}),
        ("abstract_repeated", {"the": [0, 3], "cat": [1], "sat": [2]}),
    ]:
        add("openalex", name, {"results": [{"id": "https://openalex.org/W1", "abstract_inverted_index": abstract}]})
    paper = {
        "paperId": "abc123",
        "title": "Deep Learning",
        "abstract": "Survey",
        "citationCount": 250,
        "publicationDate": "2023-06-15",
        "externalIds": {"ArXiv": "2306.12345", "DOI": "10.1234/example"},
        "authors": [{"name": "Alice Smith"}, {"name": "Bob Jones"}],
    }
    add("semanticscholar", "external_ids", {"data": [paper]})
    add("semanticscholar", "external_ids_nonmapping", {"data": [{**paper, "externalIds": ["10.1234/example"]}]})
    add(
        "semanticscholar",
        "configured_default_endpoint",
        {"data": []},
        config={"timeout_ms": 750, "api_key": "offline-fake-key"},
    )
    return rows


async def main():
    paths = [
        "engines/arxiv.py",
        "engines/openalex.py",
        "engines/semanticscholar.py",
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "slopsearx/filters.py",
        "tests/test_adapters.py",
        "tests/test_tier1_adapters.py",
        "tests/test_service_engine_config.py",
    ]
    packet = {
        "schema_version": 1,
        "plan_id": "A06",
        "baseline": "730c37a5f48b9830088594582cefb4c2f10a35e8",
        "status": "supplemental-shared-source-pack",
        "normalization": "latency_ms omitted; sets sorted; enum values serialized; no other output normalization",
        "source_sha256": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths},
        "records": [await BASE["capture"](case) for case in cases()],
    }
    print(json.dumps(packet, ensure_ascii=False, indent=2, allow_nan=False))


if __name__ == "__main__":
    asyncio.run(main())
