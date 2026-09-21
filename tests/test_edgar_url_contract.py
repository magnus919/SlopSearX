"""SEC EDGAR result URL contract tests."""

from __future__ import annotations

import httpx

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


def _adapter():
    return discover_engines({"edgar": {"enabled": True}})["edgar"]


async def test_filing_identity_builds_distinct_archive_urls() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["Apple Inc."],
                        "form_type": "10-K",
                        "filed_at": "2024-10-31T00:00:00Z",
                        "cik": "0000320193",
                        "accession_no": "0000320193-24-000123",
                        "primary_doc": "aapl-20240928.htm",
                    },
                    "_score": 10.5,
                },
                {
                    "_source": {
                        "display_names": ["Apple Inc."],
                        "form_type": "10-Q",
                        "filed_at": "2025-02-01T00:00:00Z",
                        "cik": "0000320193",
                        "accession_no": "0000320193-25-000456",
                    },
                    "_score": 9.5,
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("Apple")

    assert result.status == EngineStatus.OK
    assert result.error_message is None
    assert [item.url for item in result.results] == [
        "https://www.sec.gov/Archives/edgar/data/320193/000032019324000123/aapl-20240928.htm",
        "https://www.sec.gov/Archives/edgar/data/320193/000032019325000456/0000320193-25-000456-index.html",
    ]
    assert len({item.url for item in result.results}) == 2
    assert all(item.url for item in result.results)


async def test_cik_only_result_retains_canonical_company_url() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["NVIDIA CORP"],
                        "cik": "0001045810",
                    },
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("NVIDIA")

    assert result.status == EngineStatus.OK
    assert result.error_message is None
    assert result.results[0].url == "https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK=1045810"


async def test_same_cik_filing_looking_records_without_accessions_are_omitted() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["NVIDIA CORP"],
                        "form_type": "10-K",
                        "cik": "0001045810",
                    },
                },
                {
                    "_source": {
                        "display_names": ["NVIDIA CORP"],
                        "filed_at": "2025-02-26T00:00:00Z",
                        "cik": "0001045810",
                    },
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("NVIDIA")

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert result.error_message == (
        "2 SEC result(s) omitted because no validated SEC URL or filing identity was available"
    )


async def test_valid_provider_archive_link_is_used_without_inventing_identity() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["Example Corp"],
                        "form_type": "8-K",
                        "filing_url": ("https://www.sec.gov/Archives/edgar/data/1234567/000123456724000001/filing.htm"),
                    },
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("Example")

    assert result.status == EngineStatus.OK
    assert result.results[0].url.endswith("/filing.htm")


async def test_provider_archive_link_with_nonstandard_port_is_rejected() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["Example Corp"],
                        "filing_url": (
                            "https://www.sec.gov:8443/Archives/edgar/data/1234567/000123456724000001/filing.htm"
                        ),
                    },
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("Example")

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert result.error_message == (
        "1 SEC result(s) omitted because no validated SEC URL or filing identity was available"
    )


async def test_unresolved_or_unsafe_identity_is_omitted_and_reported() -> None:
    response = {
        "hits": {
            "hits": [
                {
                    "_source": {
                        "display_names": ["Incomplete Corp"],
                        "form_type": "10-K",
                        "accession_no": "not-an-accession",
                    },
                },
                {
                    "_source": {
                        "display_names": ["Unsafe Corp"],
                        "filing_url": "https://example.com/filing.htm",
                    },
                },
            ],
        },
    }

    async with MockHTTP(lambda request: httpx.Response(200, json=response)):
        result = await _adapter().search("unresolved")

    assert result.status == EngineStatus.OK
    assert result.results == []
    assert result.error_message == (
        "2 SEC result(s) omitted because no validated SEC URL or filing identity was available"
    )
