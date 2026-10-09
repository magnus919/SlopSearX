"""Offline tests for the mock-only coverage acquisition seam."""

from __future__ import annotations

import json
from typing import Any

import pytest

from engines.arxiv import ArxivAdapter
from engines.github import GitHubAdapter
from engines.openalex import OpenAlexAdapter
from engines.wikipedia import WikipediaAdapter
from scripts.coverage_study_acquire import (
    ENGINE_TIMEOUT_MS,
    MAX_RESULTS_PER_ENGINE,
    MockResponseFixture,
    RecordedMockTransport,
    acquire_coverage_stage,
)
from slopsearx.adapter import EngineAdapter


class FakeClock:
    def __init__(self) -> None:
        self.sleeps: list[float] = []

    async def sleep(self, seconds: float) -> None:
        self.sleeps.append(seconds)


def _adapter_configs(engines: tuple[str, ...]) -> dict[str, EngineAdapter]:
    classes: dict[str, Any] = {
        "arxiv": ArxivAdapter,
        "github": GitHubAdapter,
        "openalex": OpenAlexAdapter,
        "wikipedia": WikipediaAdapter,
    }
    bases = {
        "arxiv": "https://export.arxiv.org/api/query",
        "github": "https://api.github.com",
        "openalex": "https://api.openalex.org",
        "wikipedia": "https://en.wikipedia.org/w/api.php",
    }
    adapters: dict[str, EngineAdapter] = {
        name: classes[name](
            {"base_url": bases[name], "max_results": MAX_RESULTS_PER_ENGINE, "timeout_ms": ENGINE_TIMEOUT_MS}
        )
        for name in engines
    }
    return adapters


def _github_fixture(count: int = 20) -> MockResponseFixture:
    items = [
        {
            "full_name": f"sample/repository-{index}",
            "html_url": f"https://github.com/sample/repository-{index}",
            "description": f"Repository fixture {index}",
            "stargazers_count": index,
            "language": "Python",
        }
        for index in range(count)
    ]
    return MockResponseFixture(
        "github-repositories",
        "api.github.com",
        "/search/repositories",
        json.dumps({"total_count": count, "items": items}).encode(),
        required_query=(("q", "coding agents"),),
    )


def _openalex_fixture(count: int = 20, *, transport_error: str | None = None) -> MockResponseFixture:
    items = [
        {
            "id": f"https://openalex.org/W{index}",
            "doi": f"https://doi.org/10.1234/{index}",
            "title": f"OpenAlex result {index}",
            "publication_date": "2025-01-01",
            "relevance_score": 10 - index / 100,
            "abstract_inverted_index": None,
        }
        for index in range(count)
    ]
    return MockResponseFixture(
        "openalex-works",
        "api.openalex.org",
        "/works",
        json.dumps({"results": items}).encode(),
        required_query=(("search", "coding agents"),),
        transport_error=transport_error,
    )


def _arxiv_fixture(count: int = 2) -> MockResponseFixture:
    entries = "".join(
        f"""<entry><id>https://arxiv.org/abs/2601.{index:05d}</id><title>Arxiv result {index}</title>
        <summary>Fixture abstract</summary><published>2026-01-01T00:00:00Z</published></entry>"""
        for index in range(count)
    )
    body = (
        "<?xml version='1.0'?><feed xmlns='http://www.w3.org/2005/Atom'><title>Search</title>" + entries + "</feed>"
    ).encode()
    return MockResponseFixture("arxiv-feed", "export.arxiv.org", "/api/query", body)


def _wikipedia_empty_fixture() -> MockResponseFixture:
    return MockResponseFixture(
        "wikipedia-opensearch",
        "en.wikipedia.org",
        "/w/api.php",
        json.dumps(["coding agents", [], [], []]).encode(),
        required_query=(("action", "opensearch"),),
    )


def _case(band: str, engines: list[str]) -> dict[str, object]:
    return {
        "task_id": "R01",
        "search_query": "coding agents",
        "pool_plan": {
            "band": band,
            "engines": engines,
            "searx_categories": ["reference"],
            "engine_configs": {name: {"max_results": 20} for name in engines},
        },
    }


def _manifest(band: str, engines: list[str]) -> dict[str, object]:
    return {"stage": "development", "research_cases": [_case(band, engines)], "navigation_targets": []}


@pytest.mark.asyncio
async def test_two_engine_pool_keeps_complete_canonical_results_and_paces() -> None:
    engines = ("github", "openalex")
    transport = RecordedMockTransport([_github_fixture(), _openalex_fixture()])
    adapters = _adapter_configs(engines)
    for adapter in adapters.values():
        adapter.set_http_transport(transport)
    clock = FakeClock()

    receipt = await acquire_coverage_stage(_manifest("research_le_40", list(engines)), adapters, transport, clock)

    op = receipt.operations[0]
    assert receipt.status == "complete"
    assert op.band_valid is True and op.pool_count == 40
    assert op.canonical_response is not None and len(op.canonical_response.results) == 40
    assert op.canonical_response.scope.selected_engines == list(engines)
    assert receipt.physical_request_count == 2
    assert clock.sleeps == []
    assert all(exchange.timeout_s == 10 for exchange in receipt.exchanges)
    assert all(exchange.request_sha256 and exchange.response_sha256 for exchange in receipt.exchanges)


@pytest.mark.asyncio
async def test_four_engine_pool_over_40_is_kept_and_operation_pacing_is_fixed() -> None:
    engines = ("github", "openalex", "arxiv", "wikipedia")
    transport = RecordedMockTransport(
        [_github_fixture(), _openalex_fixture(), _arxiv_fixture(2), _wikipedia_empty_fixture()]
    )
    adapters = _adapter_configs(engines)
    for adapter in adapters.values():
        adapter.set_http_transport(transport)
    clock = FakeClock()
    manifest = _manifest("research_41_80", list(engines))
    research_cases = manifest["research_cases"]
    assert isinstance(research_cases, list)
    research_cases.append({**_case("research_41_80", list(engines)), "task_id": "R02"})

    receipt = await acquire_coverage_stage(manifest, adapters, transport, clock)

    op = receipt.operations[0]
    assert receipt.status == "complete"
    assert op.band_valid is True and op.pool_count == len(op.canonical_response.results)  # type: ignore[union-attr]
    assert 41 <= op.pool_count <= 80
    assert receipt.physical_request_count == 8
    assert receipt.reserved_worst_case_requests == 12
    assert clock.sleeps == [7.0]


@pytest.mark.asyncio
async def test_research_band_engine_count_mismatch_fails_before_any_dispatch() -> None:
    engines = ("github", "openalex", "arxiv", "wikipedia")
    transport = RecordedMockTransport([])
    adapters = _adapter_configs(engines)
    for adapter in adapters.values():
        adapter.set_http_transport(transport)
    clock = FakeClock()
    manifest = _manifest("research_le_40", list(engines))

    with pytest.raises(ValueError, match="research band requires exactly two engines"):
        await acquire_coverage_stage(manifest, adapters, transport, clock)
    assert transport.exchanges == []


@pytest.mark.asyncio
async def test_navigation_requires_github_only_and_exact_rank_one_url() -> None:
    target_url = "https://github.com/sample/repository-0"
    transport = RecordedMockTransport([_github_fixture()])
    adapters = _adapter_configs(("github",))
    adapters["github"].set_http_transport(transport)
    manifest = {
        "stage": "development",
        "research_cases": [],
        "navigation_targets": [
            {
                "target_id": "N01",
                "search_query": "coding agents",
                "target_url": target_url,
                "navigation_pool_plan": {
                    "engines": ["github"],
                    "searx_categories": ["reference"],
                    "engine_configs": {"github": {"max_results": 20}},
                },
            }
        ],
    }

    receipt = await acquire_coverage_stage(manifest, adapters, transport, FakeClock())

    assert receipt.operations[0].target_found_at_rank1 is True
    assert receipt.status == "complete"


@pytest.mark.asyncio
async def test_failures_are_retained_without_retry_or_rescue() -> None:
    engines = ("github", "openalex")
    transport = RecordedMockTransport([_github_fixture(), _openalex_fixture(transport_error="timeout")])
    adapters = _adapter_configs(engines)
    for adapter in adapters.values():
        adapter.set_http_transport(transport)
    clock = FakeClock()

    receipt = await acquire_coverage_stage(_manifest("research_le_40", list(engines)), adapters, transport, clock)

    assert receipt.status == "inconclusive"
    assert receipt.operations[0].status == "inconclusive"
    assert receipt.operations[0].failure_reasons
    assert receipt.physical_request_count == 2
    assert len(transport.exchanges) == 2
