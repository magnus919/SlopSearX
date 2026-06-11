"""Tests for the NVD adapter."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines

from .test_adapters import MockHTTP


class TestNVDAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"nvd": {"enabled": True}})
        return instances["nvd"]

    @pytest.fixture
    def adapter_with_key(self):
        instances = discover_engines({"nvd": {"enabled": True, "api_key": "test-nvd-key"}})
        return instances["nvd"]

    @pytest.fixture
    def sample_cve_response(self) -> dict:
        return {
            "resultsPerPage": 2,
            "startIndex": 0,
            "totalResults": 2,
            "format": "NVD_CVE",
            "version": "2.0",
            "timestamp": "2026-06-10T12:00:00.000",
            "vulnerabilities": [
                {
                    "cve": {
                        "id": "CVE-2024-12345",
                        "sourceIdentifier": "cve@mitre.org",
                        "published": "2024-03-15T10:00:00.000",
                        "lastModified": "2024-03-16T08:00:00.000",
                        "vulnStatus": "Analyzed",
                        "descriptions": [
                            {
                                "lang": "en",
                                "value": (
                    "A critical buffer overflow vulnerability in Example Software"
                    " allows remote attackers to execute arbitrary code via a crafted request."
                ),
                            },
                            {
                                "lang": "es",
                                "value": (
                    "Una vulnerabilidad de desbordamiento de búfer crítica en Example Software"
                    " permite a atacantes remotos ejecutar código arbitrario."
                ),
                            },
                        ],
                        "metrics": {
                            "cvssMetricV31": [
                                {
                                    "source": "nvd@nist.gov",
                                    "type": "Primary",
                                    "cvssData": {
                                        "version": "3.1",
                                        "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                                        "baseScore": 9.8,
                                        "baseSeverity": "CRITICAL",
                                    },
                                },
                            ],
                        },
                        "weaknesses": [
                            {
                                "source": "nvd@nist.gov",
                                "type": "Primary",
                                "description": [{"lang": "en", "value": "CWE-120"}],
                            },
                            {
                                "source": "nvd@nist.gov",
                                "type": "Secondary",
                                "description": [{"lang": "en", "value": "CWE-20"}],
                            },
                        ],
                        "references": [
                            {
                                "url": "https://example.com/advisory1",
                                "source": "vendor",
                            },
                            {
                                "url": "https://example.com/patch",
                                "source": "vendor",
                            },
                        ],
                    },
                },
                {
                    "cve": {
                        "id": "CVE-2024-67890",
                        "sourceIdentifier": "cve@mitre.org",
                        "published": "2024-04-01T00:00:00.000",
                        "lastModified": "2024-04-02T00:00:00.000",
                        "vulnStatus": "Analyzed",
                        "descriptions": [
                            {
                                "lang": "en",
                                "value": (
                    "An information disclosure vulnerability in Example Core"
                    " allows local users to access sensitive log data."
                ),
                            },
                        ],
                        "metrics": {
                            "cvssMetricV31": [
                                {
                                    "source": "nvd@nist.gov",
                                    "type": "Primary",
                                    "cvssData": {
                                        "version": "3.1",
                                        "vectorString": "CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:H/I:N/A:N",
                                        "baseScore": 5.5,
                                        "baseSeverity": "MEDIUM",
                                    },
                                },
                            ],
                        },
                        "weaknesses": [
                            {
                                "source": "nvd@nist.gov",
                                "type": "Primary",
                                "description": [{"lang": "en", "value": "CWE-200"}],
                            },
                        ],
                        "references": [
                            {"url": "https://example.com/security", "source": "vendor"},
                        ],
                    },
                },
            ],
        }

    async def test_search_returns_results(self, adapter, sample_cve_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_cve_response)):
            result = await adapter.search("buffer overflow")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "CVE-2024-12345"
        assert "nvd.nist.gov/vuln/detail/CVE-2024-12345" in result.results[0].url
        assert "critical buffer overflow" in result.results[0].content.lower()
        assert "CVSS 9.8" in result.results[0].content
        assert "(CRITICAL)" in result.results[0].content
        assert "CWE-120" in result.results[0].content
        assert result.results[0].published_date == "2024-03-15"
        assert result.results[1].title == "CVE-2024-67890"
        assert result.results[1].published_date == "2024-04-01"

    async def test_search_cve_id_direct(self, adapter, sample_cve_response):
        """CVE ID in query triggers cveIds param."""
        captured_params = {}

        def _handler(r):
            captured_params["cveIds"] = r.url.params.get("cveIds", "")
            return httpx.Response(200, json=sample_cve_response)

        async with MockHTTP(_handler):
            await adapter.search("CVE-2024-12345")
        assert "CVE-2024-12345" in captured_params.get("cveIds", "")

    async def test_search_cve_id_case_insensitive(self, adapter, sample_cve_response):
        """Lowercase CVE ID is normalized to uppercase."""
        captured_params = {}

        def _handler(r):
            captured_params["cveIds"] = r.url.params.get("cveIds", "")
            return httpx.Response(200, json=sample_cve_response)

        async with MockHTTP(_handler):
            await adapter.search("cve-2024-12345")
        assert "CVE-2024-12345" in captured_params.get("cveIds", "")

    async def test_search_empty_results(self, adapter):
        async with MockHTTP(
            lambda r: httpx.Response(
                200,
                json={
                    "resultsPerPage": 0,
                    "startIndex": 0,
                    "totalResults": 0,
                    "format": "NVD_CVE",
                    "version": "2.0",
                    "timestamp": "2026-06-10T12:00:00.000",
                    "vulnerabilities": [],
                },
            ),
        ):
            result = await adapter.search("xyznonexistent")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_blocked(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(403)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("test")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_http_status_error_sanitized(self):
        """HTTPStatusError (500) must not leak API key in error_message."""
        instances = discover_engines({"nvd": {"enabled": True, "api_key": "test-nvd-key-12345"}})
        adapter = instances["nvd"]

        async with MockHTTP(lambda r: httpx.Response(500)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "test-nvd-key-12345" not in result.error_message

    async def test_search_broad_exception_sanitized(self):
        """Non-JSON response caught by broad handler produces sanitized error_message."""
        instances = discover_engines({"nvd": {"enabled": True, "api_key": "test-nvd-key-12345"}})
        adapter = instances["nvd"]

        # 200 OK with non-JSON body — resp.json() will raise JSONDecodeError
        async with MockHTTP(lambda r: httpx.Response(200, content=b"<html>not json</html>")):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        # The API key must not appear in the error
        assert "test-nvd-key-12345" not in result.error_message

    async def test_search_sends_api_key_as_header_when_configured(self, adapter_with_key):
        """NVD sends API key as apiKey HTTP header, not query param."""
        captured_headers = {}
        captured_params = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            captured_params.update(dict(r.url.params))
            return httpx.Response(
                200,
                json={
                    "vulnerabilities": [],
                    "totalResults": 0,
                    "format": "NVD_CVE",
                    "version": "2.0",
                    "timestamp": "2026-01-01T00:00:00.000",
                },
            )

        async with MockHTTP(_handler):
            await adapter_with_key.search("test")
        assert captured_headers.get("apikey") == "test-nvd-key"
        assert captured_params.get("apiKey", "") == ""

    async def test_search_no_api_key_by_default(self, adapter):
        captured_headers = {}
        captured_params = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            captured_params.update(dict(r.url.params))
            return httpx.Response(
                200,
                json={
                    "vulnerabilities": [],
                    "totalResults": 0,
                    "format": "NVD_CVE",
                    "version": "2.0",
                    "timestamp": "2026-01-01T00:00:00.000",
                },
            )

        async with MockHTTP(_handler):
            await adapter.search("test")
        assert captured_params.get("apiKey", "") == ""
        assert "apikey" not in captured_headers

    async def test_whitespace_key_treated_as_no_key(self):
        """Whitespace-only key treated as absent — no header sent."""
        instances = discover_engines({"nvd": {"enabled": True, "api_key": "   "}})
        adapter = instances["nvd"]
        assert adapter._has_api_key is False

        captured_headers = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            return httpx.Response(
                200,
                json={
                    "vulnerabilities": [],
                    "totalResults": 0,
                    "format": "NVD_CVE",
                    "version": "2.0",
                    "timestamp": "2026-01-01T00:00:00.000",
                },
            )

        async with MockHTTP(_handler):
            await adapter.search("test")
        assert "apikey" not in captured_headers

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines
        assert "nvd" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines
        cls = list_engines()["nvd"]
        cats = cls.categories
        assert "general" not in cats
        assert "it" in cats
        assert "security" in cats

    def test_has_api_key_flag(self, adapter, adapter_with_key):
        """Adapter should detect API key presence at init time."""
        assert adapter._has_api_key is False
        assert adapter_with_key._has_api_key is True

    def test_format_cvss_v31(self, adapter):
        metrics = {
            "cvssMetricV31": [
                {
                    "cvssData": {
                        "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                        "baseScore": 9.8,
                        "baseSeverity": "CRITICAL",
                    },
                },
            ],
        }
        text = adapter._format_cvss(metrics)
        assert "CVSS 9.8" in text
        assert "(CRITICAL)" in text
        assert "CVSS:3.1" in text

    def test_format_cvss_v40_preferred(self, adapter):
        """CVSS v4 should be preferred over v3.1 when both are present."""
        metrics = {
            "cvssMetricV40": [
                {
                    "cvssData": {
                        "vectorString": "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N",
                        "baseScore": 9.3,
                        "baseSeverity": "CRITICAL",
                    },
                },
            ],
            "cvssMetricV31": [
                {
                    "cvssData": {
                        "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
                        "baseScore": 9.8,
                        "baseSeverity": "CRITICAL",
                    },
                },
            ],
        }
        text = adapter._format_cvss(metrics)
        assert "CVSS 9.3" in text
        assert "CVSS:4.0" in text
        assert "CVSS 9.8" not in text

    def test_format_cvss_empty(self, adapter):
        assert adapter._format_cvss({}) == ""

    def test_format_cvss_only_v2(self, adapter):
        metrics = {
            "cvssMetricV2": [
                {
                    "cvssData": {
                        "vectorString": "AV:N/AC:L/Au:N/C:C/I:C/A:C",
                        "baseScore": 10.0,
                    },
                },
            ],
        }
        text = adapter._format_cvss(metrics)
        assert "CVSS 10.0" in text
        assert "AV:N/AC:L" in text


class TestNVDAdapterNoKeyNeeded:
    """NVD works without an API key — key only improves rate limits."""

    def test_can_instantiate_without_key(self):
        instances = discover_engines({"nvd": {"enabled": True}})
        adapter = instances["nvd"]
        assert adapter._has_api_key is False
        assert adapter.config.get("api_key", "") == ""
