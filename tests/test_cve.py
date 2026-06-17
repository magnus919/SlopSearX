"""Tests for the CVE Program (MITRE) adapter."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines

from .test_adapters import MockHTTP


class TestCVEAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"cve": {"enabled": True}})
        return instances["cve"]

    @pytest.fixture
    def sample_cve_record(self) -> dict:
        return {
            "dataType": "CVE_RECORD",
            "dataVersion": "5.2",
            "cveId": "CVE-2024-12345",
            "published": "2024-03-15T10:00:00.000",
            "dateUpdated": "2024-03-16T08:00:00.000",
            "datePublished": "2024-02-01",
            "containers": {
                "cna": {
                    "providerMetadata": {"orgId": "00000000-0000-0000-0000-000000000000"},
                    "datePublic": "2024-02-01",
                    "title": "Buffer Overflow in Example Software",
                    "descriptions": [
                        {
                            "lang": "en",
                            "value": (
                                "A buffer overflow vulnerability in Example Software allows remote"
                                " attackers to execute arbitrary code via a crafted request."
                                " This is the authoritative MITRE CVE description."
                            ),
                        },
                        {
                            "lang": "es",
                            "value": (
                                "Una vulnerabilidad de desbordamiento de búfer en Example Software"
                                " permite a atacantes remotos ejecutar código arbitrario."
                            ),
                        },
                    ],
                    "metrics": [
                        {
                            "cvssV3_1": {
                                "version": "3.1",
                                "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                                "baseScore": 9.8,
                                "baseSeverity": "CRITICAL",
                            },
                        },
                    ],
                    "references": [
                        {"url": "https://example.com/advisory", "name": "Vendor Advisory"},
                        {"url": "https://example.com/patch", "name": "Patch"},
                        {"url": "https://example.com/details", "name": "Details"},
                        {"url": "https://example.com/extra", "name": "Extra"},
                    ],
                },
                "adp": [
                    {
                        "providerMetadata": {"orgId": "adp-org-id"},
                        "title": "CISA ADP",
                        "descriptions": [{"lang": "en", "value": "CISA analysis: This CVE is in KEV."}],
                    },
                ],
            },
        }

    async def test_search_with_cve_id(self, adapter, sample_cve_record):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_cve_record)):
            result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert result.results[0].title == "CVE-2024-12345"
        assert "nvd.nist.gov/vuln/detail/CVE-2024-12345" in result.results[0].url
        assert "authoritative MITRE CVE description" in result.results[0].content
        assert result.results[0].published_date == "2024-02-01"

    async def test_search_cve_in_lowercase(self, adapter, sample_cve_record):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_cve_record)):
            result = await adapter.search("cve-2024-12345")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert result.results[0].title == "CVE-2024-12345"

    async def test_search_cve_in_context(self, adapter, sample_cve_record):
        """CVE ID can appear in longer query text."""
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_cve_record)):
            result = await adapter.search("the vulnerability CVE-2024-12345 affects all versions")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1

    async def test_search_no_cve_id_returns_empty(self, adapter):
        """Plain keyword queries return empty (no public keyword search API)."""
        result = await adapter.search("buffer overflow")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_404_returns_empty(self, adapter):
        """Unknown CVE ID returns 404 → empty results, not error."""
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("CVE-2099-99999")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_blocked(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(403)):
            result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_no_api_key_config(self, adapter):
        """CVE adapter never sends an API key (not required by API)."""
        captured_headers = {}

        def _handler(r):
            captured_headers["Authorization"] = r.headers.get("Authorization", "")
            return httpx.Response(
                200,
                json={
                    "dataType": "CVE_RECORD",
                    "dataVersion": "5.2",
                    "cveId": "CVE-2024-12345",
                    "containers": {
                        "cna": {
                            "descriptions": [{"lang": "en", "value": "Test description."}],
                        },
                    },
                },
            )

        async with MockHTTP(_handler):
            await adapter.search("CVE-2024-12345")
        assert captured_headers.get("Authorization", "") == ""

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "cve" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        cls = list_engines()["cve"]
        cats = cls.categories
        assert "general" not in cats
        assert "it" in cats
        assert "security" in cats

    def test_cve_id_pattern(self):
        """Verify the CVE ID regex pattern used by the adapter."""
        from engines.cve import _CVE_ID_PATTERN

        assert _CVE_ID_PATTERN.match("CVE-2024-12345")
        assert _CVE_ID_PATTERN.match("CVE-1999-001234")
        assert _CVE_ID_PATTERN.match("cve-2024-12345")  # case insensitive
        assert not _CVE_ID_PATTERN.match("CVE-2024-123")  # too few digits
        assert not _CVE_ID_PATTERN.match("hello world")

    def test_cve_works_without_api_key(self):
        """CVE adapter does not require any API key config."""
        instances = discover_engines({"cve": {"enabled": True}})
        adapter = instances["cve"]
        assert adapter.config.get("api_key", "") == ""

    def test_extract_description_english(self, adapter):
        container = {
            "descriptions": [
                {"lang": "en", "value": "English description."},
                {"lang": "es", "value": "Descripción en español."},
            ],
        }
        desc = adapter._extract_description(container)
        assert desc == "English description."

    def test_extract_description_fallback(self, adapter):
        """If no English description, fall back to first available."""
        container = {
            "descriptions": [
                {"lang": "fr", "value": "Description française."},
            ],
        }
        desc = adapter._extract_description(container)
        assert desc == "Description française."

    def test_extract_description_empty(self, adapter):
        assert adapter._extract_description({}) == ""

    def test_extract_metrics_v31(self, adapter):
        container = {
            "metrics": [
                {
                    "cvssV3_1": {
                        "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                        "baseScore": 9.8,
                    },
                },
            ],
        }
        text = adapter._extract_metrics(container)
        assert "CVSS 9.8" in text
        assert "CVSS:3.1" in text

    def test_extract_metrics_empty(self, adapter):
        assert adapter._extract_metrics({}) == ""

    def test_extract_metrics_no_metrics(self, adapter):
        assert adapter._extract_metrics({"descriptions": []}) == ""

    def test_parse_cve_record_sets_title(self, adapter, sample_cve_record):
        results = adapter._parse_cve_record(sample_cve_record, "CVE-2024-12345")
        assert len(results) == 1
        assert results[0].title == "CVE-2024-12345"

    def test_parse_cve_record_includes_references(self, adapter, sample_cve_record):
        results = adapter._parse_cve_record(sample_cve_record, "CVE-2024-12345")
        assert "Refs:" in results[0].content
        assert "example.com/advisory" in results[0].content
        # Should show first 3 refs + "(+1 more)" since there are 4 refs
        assert "+1 more" in results[0].content


class TestCVEAdapterNoAuthRequired:
    """CVE Program API requires no authentication for read-only access."""

    def test_instantiate_without_key(self):
        instances = discover_engines({"cve": {"enabled": True}})
        assert "cve" in instances
