"""Deterministic relevance and request-contract tests for openFDA."""

from __future__ import annotations

import dataclasses
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines
from slopsearx.config import EngineEntry
from slopsearx.formatter import format_json
from slopsearx.merger import PresenceRanker


class MockHTTP:
    def __init__(self, handler):
        self.transport = httpx.MockTransport(handler)

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


def _record(
    *,
    brand: str,
    generic: str = "",
    substance: str = "",
    set_id: str = "",
    manufacturer: str = "Acme Pharma",
) -> dict:
    openfda = {"brand_name": [brand], "manufacturer_name": [manufacturer]}
    if generic:
        openfda["generic_name"] = [generic]
    if substance:
        openfda["substance_name"] = [substance]
    return {
        "set_id": set_id,
        "openfda": openfda,
        "purpose": ["Pain relief"],
        "indications_and_usage": ["For the temporary relief of minor aches"],
    }


@pytest.fixture
def adapter():
    return discover_engines({"openfda": {"enabled": True}})["openfda"]


async def test_exact_name_fields_rank_before_related_and_preserve_provenance(adapter):
    response = {
        "results": [
            _record(brand="Naproxen", generic="Naproxen", substance="NAPROXEN", set_id="unrelated"),
            _record(brand="ASPIRIN", generic="Acetylsalicylic Acid", substance="ASPIRIN", set_id="brand"),
            _record(brand="ASA Tablets", generic="ASPIRIN", substance="ACETYLSALICYLIC ACID", set_id="generic"),
            _record(
                brand="Pain Relief Tablets",
                generic="Acetylsalicylic Acid",
                substance="ASPIRIN",
                set_id="substance",
            ),
            _record(
                brand="Aspirin Extra Strength",
                generic="Acetylsalicylic Acid",
                substance="ACETYLSALICYLIC ACID",
                set_id="related",
            ),
        ],
    }
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json=response)

    async with MockHTTP(handler):
        result = await adapter.search("aspirin")

    assert result.status == EngineStatus.OK
    assert len(requests) == 1
    search = requests[0].url.params["search"]
    # The official openFDA query syntax defines whitespace-separated clauses
    # as OR; +AND+ would require one record to satisfy every field clause.
    assert "openfda.brand_name.exact:" in search
    assert " openfda.generic_name.exact:" in search
    assert " openfda.substance_name.exact:" in search
    assert "+AND+" not in search
    assert requests[0].url.params["limit"] == "10"

    assert [item.title for item in result.results[:3]] == ["ASPIRIN", "ASA Tablets", "Pain Relief Tablets"]
    assert all(item.score == 1.0 for item in result.results[:3])
    assert result.results[-1].title == "Naproxen"
    assert result.results[-1].score < result.results[0].score
    assert result.results[0].payload is not None
    assert result.results[0].payload["data"]["generic_name"] == "Acetylsalicylic Acid"
    assert result.results[0].payload["data"]["substance"] == "ASPIRIN"
    assert result.results[0].payload["data"]["set_id"] == "brand"
    assert result.results[0].payload["provenance"]["engine"] == "openfda"
    first_url = urlparse(result.results[0].url)
    assert first_url.netloc == "api.fda.gov"
    assert parse_qs(first_url.query)["search"] == ['set_id.exact:"brand"']


async def test_identified_same_brand_records_keep_distinct_source_links(adapter):
    response = {
        "results": [
            _record(brand="ASPIRIN", generic="Acetylsalicylic Acid", substance="ASPIRIN", set_id="label-one"),
            _record(brand="ASPIRIN", generic="Acetylsalicylic Acid", substance="ASPIRIN", set_id="label-two"),
        ],
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await adapter.search("aspirin")

    assert len(result.results) == 2
    assert len({item.url for item in result.results}) == 2
    assert "set_id.exact" in result.results[0].url
    assert "label-one" in result.results[0].url
    assert "label-two" in result.results[1].url


async def test_engine_entry_blank_base_url_uses_openfda_default():
    entry = dataclasses.asdict(EngineEntry(timeout_ms=1_234, max_results=2, api_key=""))
    configured = discover_engines({"openfda": entry})["openfda"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": []})

    async with MockHTTP(handler):
        result = await configured.search("aspirin")

    assert result.status == EngineStatus.OK
    assert len(requests) == 2
    assert all(request.url.host == "api.fda.gov" for request in requests)
    assert all(request.url.path == "/drug/label.json" for request in requests)
    assert all(request.url.params["limit"] == "2" for request in requests)


async def test_identifier_free_labels_are_omitted_before_merge_and_format(adapter):
    response = {
        "results": [
            _record(brand="ASPIRIN", generic="Acetylsalicylic Acid", substance="ASPIRIN"),
            _record(brand="ASPIRIN", generic="Acetylsalicylic Acid", substance="ASPIRIN"),
        ],
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await adapter.search("aspirin")

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert result.error_message == "omitted 2 openFDA label record(s) without a stable source identifier"

    merged = PresenceRanker().rank({"openfda": result.results}, "aspirin")
    rendered = format_json(merged, "aspirin")
    assert merged == []
    assert rendered["results"] == []


async def test_empty_exact_query_uses_one_bounded_fielded_broad_fallback(adapter):
    requests: list[httpx.Request] = []
    fallback = {
        "results": [
            _record(brand="Aspirin Plus", generic="Acetylsalicylic Acid", substance="ASPIRIN", set_id="broad-1"),
            _record(brand="Naproxen", generic="Naproxen", substance="NAPROXEN", set_id="broad-2"),
        ],
    }

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"results": []} if len(requests) == 1 else fallback)

    async with MockHTTP(handler):
        result = await adapter.search("aspirin")

    assert result.status == EngineStatus.OK
    assert len(requests) == 2
    assert ".exact:" in requests[0].url.params["search"]
    assert ".exact:" not in requests[1].url.params["search"]
    assert "openfda.brand_name:" in requests[1].url.params["search"]
    assert "openfda.generic_name:" in requests[1].url.params["search"]
    assert "openfda.substance_name:" in requests[1].url.params["search"]
    assert requests[0].url.params["limit"] == requests[1].url.params["limit"] == "10"
    assert [item.title for item in result.results] == ["Aspirin Plus", "Naproxen"]


async def test_empty_exact_and_broad_404_is_valid_empty(adapter):
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(404)

    async with MockHTTP(handler):
        result = await adapter.search("does-not-exist")

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert len(requests) == 2


async def test_malformed_results_are_an_error(adapter):
    async with MockHTTP(lambda request: httpx.Response(200, json={"results": {}})):
        result = await adapter.search("aspirin")

    assert result.status == EngineStatus.ERROR
    assert result.results == []


async def test_timeout_and_rate_limit_classification_is_preserved(adapter):
    async with MockHTTP(lambda request: httpx.Response(429)):
        rate_limited = await adapter.search("aspirin")
    assert rate_limited.status == EngineStatus.RATE_LIMITED

    async with MockHTTP(lambda request: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
        with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
            timed_out = await adapter.search("aspirin")
    assert timed_out.status == EngineStatus.TIMEOUT
