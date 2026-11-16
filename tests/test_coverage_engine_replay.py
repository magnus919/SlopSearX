"""Synthetic, offline scenarios exercise the real four-engine parsers."""

from __future__ import annotations

import base64
import json
from urllib.parse import urlencode

import httpx
import pytest

from engines.arxiv import ArxivAdapter
from engines.github import GitHubAdapter
from engines.openalex import OpenAlexAdapter
from engines.wikipedia import WikipediaAdapter
from scripts.coverage_engine_replay import (
    EngineReplay,
    ReplayBundleError,
    ReplayMismatchError,
    _request_key,
    _validate_bundle,
    build_bundle,
    sha256,
)
from slopsearx.adapter import EngineStatus


def _http(
    engine: str, method: str, url: str, body: bytes, *, status: int = 200, content_type: str = "application/json"
):
    return {
        "scenario_id": f"{engine}-{status}-{len(body)}",
        "engine": engine,
        "request": {"method": method, "url": url},
        "outcome": {
            "kind": "http",
            "status_code": status,
            "headers": {"content-type": content_type},
            "body_base64": base64.b64encode(body).decode(),
        },
    }


def _fault(engine: str, method: str, url: str, kind: str):
    return {
        "scenario_id": f"{engine}-{kind}",
        "engine": engine,
        "request": {"method": method, "url": url},
        "outcome": {"kind": kind},
    }


def _replay(*scenarios, mode="synthetic-scenario"):
    raw = build_bundle(mode, scenarios)
    return EngineReplay(raw, sha256(raw))


def _base_query(base: str, params: dict[str, object]) -> str:
    return f"{base}?{urlencode(params)}"


@pytest.mark.asyncio
async def test_synthetic_replay_drives_all_four_real_engine_parsers():
    github_url = _base_query(
        "https://replay.example/github/search/repositories", {"q": "synthetic topic", "per_page": 2, "page": 1}
    )
    github_body = json.dumps(
        {
            "total_count": 1,
            "items": [
                {
                    "full_name": "sample/project",
                    "html_url": "https://github.com/sample/project",
                    "description": "Synthetic repository fixture",
                    "stargazers_count": 7,
                }
            ],
        }
    ).encode()
    arxiv_url = _base_query(
        "https://replay.example/arxiv/api/query",
        {
            "search_query": 'all:"synthetic topic"',
            "start": 0,
            "max_results": 2,
            "sortBy": "relevance",
            "sortOrder": "descending",
        },
    )
    arxiv_body = b"""<feed xmlns="http://www.w3.org/2005/Atom">
      <entry><id>https://arxiv.org/abs/2601.12345</id>
      <title>Synthetic paper</title><summary>Fixture abstract.</summary>
      <published>2026-01-01T00:00:00Z</published></entry></feed>"""
    openalex_url = "https://replay.example/openalex/works?search=synthetic%20topic&per_page=2"
    openalex_body = json.dumps(
        {
            "results": [
                {
                    "id": "https://openalex.org/W123",
                    "doi": None,
                    "title": "Synthetic work",
                    "relevance_score": 1.2,
                    "publication_date": "2025-01-01",
                    "abstract_inverted_index": {"Replay": [0], "fixture.": [1]},
                }
            ]
        }
    ).encode()
    wiki_base = "https://replay.example/wikipedia/api.php"
    wiki_search = _base_query(
        wiki_base, {"action": "opensearch", "search": "synthetic topic", "limit": 2, "format": "json", "origin": "*"}
    )
    wiki_detail = _base_query(
        wiki_base,
        {
            "action": "query",
            "titles": "Synthetic topic",
            "prop": "extracts|pageimages|pageprops",
            "exintro": 1,
            "explaintext": 1,
            "pithumbsize": 300,
            "format": "json",
            "origin": "*",
        },
    )
    scenarios = [
        _http("github", "GET", github_url, github_body),
        _http("arxiv", "GET", arxiv_url, arxiv_body, content_type="application/atom+xml"),
        _http("openalex", "GET", openalex_url, openalex_body),
        _http(
            "wikipedia",
            "GET",
            wiki_search,
            b'["synthetic topic",["Synthetic topic"],["Fixture snippet"],["https://en.wikipedia.org/wiki/Synthetic_topic"]]',
        ),
        _http(
            "wikipedia",
            "GET",
            wiki_detail,
            json.dumps(
                {
                    "query": {
                        "pages": {
                            "123": {
                                "pageid": 123,
                                "title": "Synthetic topic",
                                "extract": "Fixture article extract.",
                            }
                        }
                    }
                }
            ).encode(),
        ),
    ]
    replay = _replay(*scenarios)
    adapters = [
        GitHubAdapter({"base_url": "https://replay.example/github", "max_results": 2}),
        ArxivAdapter({"base_url": "https://replay.example/arxiv/api/query", "max_results": 2}),
        OpenAlexAdapter({"base_url": "https://replay.example/openalex", "max_results": 2}),
        WikipediaAdapter({"base_url": wiki_base, "max_results": 2}),
    ]
    outputs = []
    for adapter in adapters:
        adapter.set_http_transport(replay.transport)
        outputs.append(await adapter.search("synthetic topic"))
    assert [output.status for output in outputs] == [EngineStatus.OK] * 4
    assert [output.results[0].title for output in outputs] == [
        "sample/project",
        "Synthetic paper",
        "Synthetic work",
        "Synthetic topic",
    ]
    receipt = replay.receipt()
    assert receipt.mode == "synthetic-scenario"
    assert len(receipt.consumed_scenario_ids) == 5
    assert not receipt.unused_scenario_ids
    assert receipt.quality_credit is False
    await replay.aclose()


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("engine", "adapter", "url", "outcome", "expected"),
    [
        (
            "github",
            GitHubAdapter,
            "https://replay.example/github/search/repositories?q=test&per_page=2&page=1",
            (403, b"forbidden"),
            EngineStatus.BLOCKED,
        ),
        (
            "arxiv",
            ArxivAdapter,
            "https://replay.example/arxiv/api/query?search_query=all%3Atest&start=0&max_results=2&sortBy=relevance&sortOrder=descending",
            (429, b"rate limited"),
            EngineStatus.RATE_LIMITED,
        ),
        (
            "openalex",
            OpenAlexAdapter,
            "https://replay.example/openalex/works?search=test&per_page=2",
            ("timeout", None),
            EngineStatus.TIMEOUT,
        ),
        (
            "wikipedia",
            WikipediaAdapter,
            "https://replay.example/wikipedia/api.php?action=opensearch&search=test&limit=2&format=json&origin=%2A",
            ("incomplete", None),
            EngineStatus.ERROR,
        ),
        (
            "github",
            GitHubAdapter,
            "https://replay.example/github/search/repositories?q=test&per_page=2&page=1",
            (200, b"{malformed"),
            EngineStatus.ERROR,
        ),
    ],
)
async def test_failure_scenarios_are_delivered_to_real_adapters(engine, adapter, url, outcome, expected):
    if outcome[0] == "timeout":
        scenario = _fault(engine, "GET", url, "timeout")
    elif outcome[0] == "incomplete":
        scenario = _fault(engine, "GET", url, "incomplete")
    else:
        scenario = _http(engine, "GET", url, outcome[1], status=outcome[0])
    replay = _replay(scenario)
    instance = adapter(
        {
            "base_url": url.split("/search/")[0] if engine == "github" else url.split("?")[0].removesuffix("/works"),
            "max_results": 2,
        }
    )
    instance.set_http_transport(replay.transport)
    response = await instance.search("test")
    assert response.status is expected
    assert replay.receipt().consumed_scenario_ids == (scenario["scenario_id"],)
    assert replay.receipt().quality_credit is False
    await replay.aclose()


@pytest.mark.asyncio
async def test_unmatched_request_fails_closed_without_live_transport_fallback():
    replay = _replay(_http("github", "GET", "https://replay.example/other", b"{}"))
    adapter = GitHubAdapter({"base_url": "https://unmatched.invalid", "max_results": 2})
    adapter.set_http_transport(replay.transport)
    response = await adapter.search("test")
    assert response.status is EngineStatus.ERROR
    assert not replay.receipt().consumed_scenario_ids
    assert len(replay.receipt().unused_scenario_ids) == 1
    await replay.aclose()


@pytest.mark.asyncio
async def test_header_mismatch_does_not_consume_the_recorded_response():
    url = "https://replay.example/works?search=test"
    row = _http("openalex", "GET", url, b'{"results":[]}')
    row["request"]["headers"] = {"accept": "application/json"}
    replay = _replay(row)
    async with httpx.AsyncClient(transport=replay.transport) as client:
        with pytest.raises(ReplayMismatchError, match="request-header-mismatch"):
            await client.get(url, headers={"accept": "text/plain"})
        assert replay.receipt().consumed_scenario_ids == ()
        assert replay.receipt().unused_scenario_ids == (row["scenario_id"],)
        response = await client.get(url, headers={"accept": "application/json"})
        assert response.json() == {"results": []}
        assert replay.receipt().consumed_scenario_ids == (row["scenario_id"],)


def test_bundle_hash_and_recorded_capture_provenance_are_required():
    row = _http("openalex", "GET", "https://replay.example/works?search=test", b'{"results":[]}')
    raw = build_bundle("synthetic-scenario", [row])
    with pytest.raises(ReplayBundleError, match="bundle-hash-mismatch"):
        EngineReplay(raw, "0" * 64)
    with pytest.raises(ReplayBundleError, match="recorded-capture-provenance-required"):
        EngineReplay(build_bundle("recorded-replay", [row]), sha256(build_bundle("recorded-replay", [row])))
    recorded = build_bundle(
        "recorded-replay",
        [row],
        capture={"capture_id": "sanitized-capture-1", "captured_at": "2026-10-14T00:00:00Z", "source_digest": "a" * 64},
    )
    replay = EngineReplay(recorded, sha256(recorded))
    assert replay.receipt().mode == "recorded-replay"
    assert replay.receipt().quality_credit is False


@pytest.mark.parametrize(
    "url",
    [
        "https://replay.example/search?client_secret=hidden",
        "https://replay.example/search?subscription-key=hidden",
        "https://replay.example/search?access_token=hidden",
    ],
)
def test_credential_like_query_names_are_rejected(url):
    with pytest.raises(ReplayBundleError, match="request-credential-query-forbidden"):
        _request_key("GET", url)


@pytest.mark.parametrize("header", ["x-api-key", "x-auth-token", "x-github-token", "subscription-key"])
def test_credential_like_request_headers_are_rejected(header):
    with pytest.raises(ReplayBundleError, match="request-credential-header-forbidden"):
        _request_key("GET", "https://replay.example/search", {header: "hidden"})


def test_arbitrary_request_headers_are_not_replayable():
    with pytest.raises(ReplayBundleError, match="request-header-not-allowlisted"):
        _request_key("GET", "https://replay.example/search", {"x-custom-context": "value"})


@pytest.mark.parametrize("header", ["x-api-key", "x-auth-token", "x-github-token"])
def test_credential_like_response_headers_are_rejected(header):
    scenario = _http("github", "GET", "https://replay.example/search", b"{}")
    scenario["outcome"]["headers"] = {header: "hidden"}
    raw = build_bundle("synthetic-scenario", [scenario])
    with pytest.raises(ReplayBundleError, match="scenario-response-headers-invalid"):
        _validate_bundle(raw)
