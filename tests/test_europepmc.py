"""Captured Europe PMC lite replay and fail-closed contracts."""

import json
import os
from pathlib import Path
from unittest.mock import patch

import httpx
import pytest

from engines.europepmc import EuropePMCAdapter
from slopsearx.adapter import EngineStatus
from slopsearx.config import load_config

FIXTURES = Path(__file__).parent / "fixtures" / "europepmc"


@pytest.mark.parametrize("fixture", ["biomedical", "preprints"])
async def test_captured_lite_records(fixture):
    data = json.loads((FIXTURES / f"{fixture}.json").read_text())
    requests = []

    def handle(request):
        requests.append(request)
        return httpx.Response(200, json=data)

    adapter = EuropePMCAdapter()
    adapter._http_transport = httpx.MockTransport(handle)
    result = await adapter.search("test")
    await adapter.shutdown()
    assert result.status == EngineStatus.OK
    assert len(result.results) == 5
    assert requests[0].url.params["resultType"] == "lite"
    assert "Europe PMC / EMBL-EBI" in result.results[0].content
    if fixture == "biomedical":
        assert result.results[0].url.startswith("https://europepmc.org/article/MED/")
    else:
        assert "Preprint (not peer reviewed)" in result.results[0].content
        assert result.results[0].url.startswith("https://europepmc.org/article/PPR/")


@pytest.mark.parametrize("body,status", [({}, EngineStatus.ERROR), ({"resultList": {"result": []}}, EngineStatus.OK)])
async def test_schema_and_empty(body, status):
    adapter = EuropePMCAdapter()
    adapter._http_transport = httpx.MockTransport(lambda r: httpx.Response(200, json=body))
    result = await adapter.search("test")
    await adapter.shutdown()
    assert result.status == status
    assert result.results == []


@pytest.mark.parametrize("code,status", [(429, EngineStatus.RATE_LIMITED), (500, EngineStatus.ERROR)])
async def test_classified_http_failures(code, status):
    adapter = EuropePMCAdapter()
    adapter._http_transport = httpx.MockTransport(lambda r: httpx.Response(code))
    result = await adapter.search("test")
    await adapter.shutdown()
    assert result.status == status


def test_opt_in_environment_config(monkeypatch):
    monkeypatch.setenv("ENGINE_EUROPEPMC_ENABLED", "true")
    monkeypatch.setenv("ENGINE_EUROPEPMC_MAX_RESULTS", "7")
    with patch.dict("os.environ", {}, clear=False):
        config = load_config()
    assert config.engines["europepmc"].enabled
    assert config.engines["europepmc"].max_results == 7


async def test_timeout_classification():
    def fail(request):
        raise httpx.ReadTimeout("bounded probe timeout", request=request)

    adapter = EuropePMCAdapter()
    adapter.set_http_transport(httpx.MockTransport(fail))
    result = await adapter.search("test")
    await adapter.shutdown()
    assert result.status == EngineStatus.TIMEOUT


async def test_attribution_visible_in_portal():
    from slopsearx.formatter import format_html

    adapter = EuropePMCAdapter()
    data = json.loads((FIXTURES / "preprints.json").read_text())
    adapter.set_http_transport(httpx.MockTransport(lambda r: httpx.Response(200, json=data)))
    result = await adapter.search("COVID-19")
    await adapter.shutdown()
    html = format_html(result.results, "COVID-19")
    assert "Europe PMC / EMBL-EBI" in html
    assert "Preprint (not peer reviewed)" in html


async def test_invalid_dates_and_optional_text_are_not_fabricated():
    record = {
        "source": "MED",
        "id": "1",
        "title": "title",
        "firstPublicationDate": "2026-99-99",
        "journalTitle": [],
        "authorString": {"name": "x"},
        "pmcid": "PMC1",
        "doi": "10.1/test",
    }
    adapter = EuropePMCAdapter()
    adapter.set_http_transport(
        httpx.MockTransport(lambda r: httpx.Response(200, json={"resultList": {"result": [record]}}))
    )
    response = await adapter.search("test")
    await adapter.shutdown()
    result = response.results[0]
    assert result.published_date is None
    assert "[]" not in result.content and "name" not in result.content
    assert result.payload["data"]["pmid"] == "1"
    assert result.payload["data"]["pmcid"] == "PMC1"
    assert result.payload["data"]["doi"] == "10.1/test"


@pytest.mark.skipif(
    os.environ.get("SLOPSEARX_EUROPEPMC_LIVE") != "1",
    reason="one bounded upstream request requires explicit live opt-in",
)
async def test_bounded_live_contract():
    adapter = EuropePMCAdapter({"max_results": 2, "timeout_ms": 10000})
    try:
        response = await adapter.search("SRC:PPR AND COVID-19")
        assert response.status == EngineStatus.OK, response.error_message
        assert 0 < len(response.results) <= 2
        assert all(result.url.startswith("https://europepmc.org/article/PPR/") for result in response.results)
    finally:
        await adapter.shutdown()


async def test_pubmed_first_merge_retains_provider_attribution():
    from slopsearx.adapter import SearchResult
    from slopsearx.formatter import format_html, format_json
    from slopsearx.merger import PresenceRanker

    data = json.loads((FIXTURES / "biomedical.json").read_text())
    adapter = EuropePMCAdapter()
    adapter.set_http_transport(httpx.MockTransport(lambda r: httpx.Response(200, json=data)))
    response = await adapter.search("CRISPR sickle cell")
    await adapter.shutdown()
    pmid = response.results[0].payload["data"]["pmid"]
    pubmed = SearchResult(
        url=f"https://pubmed.ncbi.nlm.nih.gov/{pmid}/", title="PubMed article", content="journal", engine="pubmed"
    )
    merged = PresenceRanker().rank({"pubmed": [pubmed], "europepmc": response.results}, "test")
    assert len(merged) == 6
    assert "Europe PMC / EMBL-EBI" in format_html(merged, "test")
    assert "Europe PMC / EMBL-EBI" in json.dumps(format_json(merged, "test"))


async def test_compact_mcp_retains_attribution_and_preprint_notice():
    from slopsearx.mcp.result_serialization import _result_to_dict

    record = {"source": "PPR", "id": "PPR1", "title": "Preprint", "journalTitle": "J" * 200, "authorString": "A" * 400}
    adapter = EuropePMCAdapter()
    adapter.set_http_transport(
        httpx.MockTransport(lambda r: httpx.Response(200, json={"resultList": {"result": [record]}}))
    )
    response = await adapter.search("test")
    await adapter.shutdown()
    snippet = _result_to_dict(response.results[0])["snippet"]
    assert "Europe PMC / EMBL-EBI" in snippet
    assert snippet.startswith("Source: Europe PMC / EMBL-EBI (https://europepmc.org)")
    assert "Preprint (not peer reviewed)" in snippet
