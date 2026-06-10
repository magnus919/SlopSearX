"""Tests for AbuseIPDB adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestAbuseIPDBAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"abuseipdb": {"enabled": True, "api_key": "test-key"}})["abuseipdb"]

    def _adapter_no_key(self):
        return discover_engines({"abuseipdb": {"enabled": True, "api_key": ""}})["abuseipdb"]

    async def test_search_returns_reputation(self, adapter):
        data = {
            "data": {
                "ipAddress": "8.8.8.8",
                "abuseConfidenceScore": 0,
                "totalReports": 0,
                "countryCode": "US",
                "isp": "Google LLC",
                "domain": "google.com",
                "usageType": "Data Center/Web Hosting",
            }
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("8.8.8.8")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Abuse Confidence: 0%" in result.results[0].content
        assert "Google LLC" in result.results[0].content

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("8.8.8.8")
        assert result.status == EngineStatus.ERROR
        assert "API key" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("8.8.8.8")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "abuseipdb" in list_engines()
