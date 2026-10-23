"""Initial A06 actual-source observations; offline migration oracle, not runtime.

This initial pack is deliberately incomplete. Add whole-source cases and map every
regression before native acceptance. No provider traffic or real credentials.
"""

import argparse
import asyncio
import base64
import dataclasses
import hashlib
import json
from pathlib import Path
from unittest.mock import patch

import httpx

from engines.arxiv import ArxivAdapter
from engines.openalex import OpenAlexAdapter
from engines.semanticscholar import SemanticScholarAdapter

ROOT = Path(__file__).resolve().parents[4]
TYPES = {"arxiv": ArxivAdapter, "openalex": OpenAlexAdapter, "semanticscholar": SemanticScholarAdapter}
FEED = (
    '<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/'
    'atom"><entry><id>https://arxiv.org/abs/2401.01234v2</id><title>  Paper title  </'
    "title><summary> First\n second </summary><published>2024-01-03</published><update"
    "d>2024-01-04</updated><author><name>Ada</name></author><arxiv:doi>10.1/example</"
    "arxiv:doi></entry></feed>"
)


def reply(body, status=200, headers=None):
    if not isinstance(body, str):
        body = json.dumps(body, ensure_ascii=False)
    return {"status": status, "body_b64": base64.b64encode(body.encode()).decode(), "headers": headers or {}}


def plain(value):
    if dataclasses.is_dataclass(value):
        return plain(dataclasses.asdict(value))
    if isinstance(value, dict):
        return {k: plain(v) for k, v in value.items() if k != "latency_ms"}
    if isinstance(value, (list, tuple)):
        return [plain(v) for v in value]
    if isinstance(value, set):
        return sorted(value)
    if hasattr(value, "value"):
        return value.value
    return value


async def capture(case):
    requests, clients = [], []
    responses = case["responses"]
    index = 0

    def transport(request):
        nonlocal index
        requests.append({"method": request.method, "url": str(request.url), "headers": dict(request.headers)})
        if index >= len(responses):
            raise AssertionError("unexpected extra source request")
        item = responses[index]
        index += 1
        if item.get("exception") == "timeout":
            raise httpx.ReadTimeout("offline timeout", request=request)
        if item.get("exception") == "connect":
            raise httpx.ConnectError("offline connection failure", request=request)
        return httpx.Response(item["status"], content=base64.b64decode(item["body_b64"]), headers=item["headers"])

    def client(**kwargs):
        clients.append(kwargs)
        return httpx.AsyncClient(transport=httpx.MockTransport(transport), **kwargs)

    adapter = TYPES[case["engine"]](case.get("config", {}))
    with patch.object(adapter, "http_client", side_effect=client), patch("time.time", return_value=1700000000.0):
        result = await adapter.search(case["query"], case.get("params"))
    return {**case, "requests": requests, "client_options": clients, "output": plain(result)}


def cases():
    rows = []
    bodies = {
        "openalex": {
            "results": [
                {
                    "id": "https://openalex.org/W1",
                    "doi": "10.1/example",
                    "title": "Title",
                    "publication_date": "2024-01-03",
                    "relevance_score": 2.5,
                    "abstract_inverted_index": {"Second": [1], "First": [0]},
                }
            ]
        },
        "semanticscholar": {
            "data": [
                {
                    "title": "Title",
                    "url": "https://paper.test/1",
                    "abstract": " Abstract ",
                    "citationCount": 12,
                    "publicationDate": "2024-01-03",
                    "externalIds": {"DOI": "10.1/example", "ArXiv": "2401.01234v2"},
                    "authors": [{"name": "Ada"}, {"name": "Bo"}],
                }
            ]
        },
        "arxiv": FEED,
    }

    def add(engine, name, body=None, **kwargs):
        rows.append(
            {
                "id": engine + "." + name,
                "engine": engine,
                "query": kwargs.pop("query", "graph neural networks"),
                "responses": kwargs.pop("responses", [reply(body)]),
                **kwargs,
            }
        )

    for engine, body in bodies.items():
        add(engine, "success", body)
        add(engine, "query_punctuation", body, query="q*~ /%雪")
        add(engine, "empty", '<feed xmlns="http://www.w3.org/2005/Atom"/>' if engine == "arxiv" else {})
        add(engine, "malformed", "{broken")
        for status in [429, 500]:
            add(engine, f"http_{status}", responses=[reply({}, status)])
        for error in ["timeout", "connect"]:
            add(engine, error, responses=[{"exception": error}])
    for value in [None, [], 1, "x"]:
        for engine in ["openalex", "semanticscholar"]:
            add(engine, f"root_{type(value).__name__}", value)
    for i, doi in enumerate([None, "", " 10.1/example ", "https://doi.org/10.1/example", 5]):
        add("openalex", f"doi_{i}", {"results": [{"id": "fallback", "doi": doi}]})
    for i, bounds in enumerate(
        [
            {"date_from": "2024-01-03", "date_to": "2024-01-03"},
            {"date_from": "2024-01-04"},
            {"date_from": "bad"},
            {"date_from": "2024-02-01", "date_to": "2024-01-01"},
        ]
    ):
        add("openalex", f"dates_{i}", bodies["openalex"], params=bounds)
    for i, score in enumerate([None, "1_000", True, "bad", 0]):
        add("semanticscholar", f"citation_{i}", {"data": [{"title": "T", "citationCount": score}]})
    add("semanticscholar", "auth", bodies["semanticscholar"], config={"api_key": "offline-fake-key"})
    for i, header in enumerate(["0", "12.5", "nan", "-1", "Tue, 14 Nov 2023 22:15:00 GMT"]):
        add("semanticscholar", f"retry_after_{i}", responses=[reply({}, 429, {"Retry-After": header})])
    for i, query in enumerate(
        [
            "word",
            "graph neural networks",
            " cat  dog ",
            "cat AND dog",
            "ti:rust",
            '"exact phrase"',
            "α β",
            "one-two three",
            "a\x1cb",
        ]
    ):
        add("arxiv", f"query_{i}", FEED, query=query)
    add("arxiv", "mixed_content", FEED.replace("Paper title", "Before<b>child</b>after"))
    add("arxiv", "missing_fields", '<feed xmlns="http://www.w3.org/2005/Atom"><entry/></feed>')
    for i, location in enumerate([None, "http://export.arxiv.org/api/query", "https://other.test/query", "/api/next"]):
        add(
            "arxiv",
            f"redirect_{i}",
            responses=[reply("", 302, {} if location is None else {"Location": location}), reply(FEED)],
        )
    return rows


def regression_cases():
    """Supplement the initial pack using inspected source regression inputs."""
    rows = []

    def add(engine, name, body=None, **kwargs):
        rows.append(
            {
                "id": engine + ".regression." + name,
                "engine": engine,
                "query": kwargs.pop("query", "graph neural networks"),
                "responses": kwargs.pop("responses", [reply(body)]),
                **kwargs,
            }
        )

    works = [
        {
            "id": "https://openalex.org/W123",
            "title": "A Study on Machine Learning",
            "doi": "https://doi.org/10.1234/test",
            "relevance_score": 42.5,
            "cited_by_count": 450,
            "publication_date": "2024-03-15",
            "abstract_inverted_index": {"Machine": [0], "learning": [1], "is": [2], "important": [3]},
        },
        {
            "id": "https://openalex.org/W456",
            "title": "Deep Learning Advances",
            "doi": None,
            "cited_by_count": 120,
            "publication_date": "2023-11-01",
            "abstract_inverted_index": None,
        },
    ]
    add("openalex", "original_order_and_abstract", {"results": works}, query="C++ learning")
    for i, doi in enumerate(
        [
            "https://doi.org/10.1038/nature14539",
            "http://dx.doi.org/10.1038/nature14539",
            "10.1038/nature14539",
            None,
            "  ",
        ]
    ):
        add("openalex", f"doi_{i}", {"results": [{**works[0], "doi": doi}]})
    for i, payload in enumerate(
        [
            [],
            {"results": None},
            {"results": [{}, {"abstract_inverted_index": ["bad"]}]},
            {"results": [{"cited_by_count": "bad", "relevance_score": {"bad": 1}}]},
            {"results": [{"abstract_inverted_index": {"word": None}}]},
        ]
    ):
        add("openalex", f"malformed_{i}", payload)
    dates = ["2024-02-28", "2024-02-29", "2024-03-01", None, "2024", "bad"]
    body = {"results": [{"id": f"https://openalex.org/W{i}", "publication_date": date} for i, date in enumerate(dates)]}
    for i, bounds in enumerate(
        [
            {"date_from": "2024-02-29"},
            {"date_to": "2024-02-29"},
            {"date_from": "2024-02-29", "date_to": "2024-02-29"},
        ]
    ):
        add("openalex", f"inclusive_dates_{i}", body, params=bounds, query="C++ & climate")
    for i, bounds in enumerate(
        [
            {"date_from": "2023-02-29"},
            {"date_to": "2024-13-01"},
            {"date_from": "20240101"},
            {"date_from": ""},
            {"date_from": "2024-01-01,publication_year:2020"},
            {"date_from": "2024-03-01", "date_to": "2024-02-01"},
            {"time_range": "all"},
        ]
    ):
        add("openalex", f"invalid_dates_{i}", body, params=bounds)
    for i, query in enumerate(
        [
            "graph neural networks",
            "protein-protein interactions",
            "graph",
            'ti:"graph neural networks"',
            "ti:graph neural",
            "graph AND neural",
            "graph OR neural",
            "graph ANDNOT neural",
            "graph and neural",
            "graph (neural networks)",
        ]
    ):
        add("arxiv", f"original_query_{i}", FEED, query=query)
    for i, header in enumerate(
        [
            "45",
            "not-a-duration",
            "Wed, 01 Jan 2020 00:00:00 GMT",
            "999999",
            "-5",
            "Tue, 14 Nov 2023 22:14:05 GMT",
            "Tue, 14 Nov 2023 22:12:35 GMT",
        ]
    ):
        add("semanticscholar", f"original_retry_after_{i}", responses=[reply({}, 429, {"Retry-After": header})])
    return rows


async def main(regressions=False):
    records = [await capture(case) for case in (regression_cases() if regressions else cases())]
    paths = [
        "engines/openalex.py",
        "engines/semanticscholar.py",
        "engines/arxiv.py",
        "slopsearx/adapter.py",
        "slopsearx/payload.py",
        "slopsearx/filters.py",
    ]
    out = {
        "schema_version": 1,
        "plan_id": "A06",
        "baseline": "730c37a5f48b9830088594582cefb4c2f10a35e8",
        "status": "supplemental-regression-source-pack" if regressions else "initial-incomplete-source-pack",
        "normalization": "latency_ms omitted; sets sorted; enum values serialized; no other output normalization",
        "source_sha256": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths},
        "records": records,
    }
    print(json.dumps(out, ensure_ascii=False, indent=2, allow_nan=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--regressions", action="store_true")
    asyncio.run(main(parser.parse_args().regressions))
