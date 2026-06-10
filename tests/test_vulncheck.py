"""Tests for VulnCheck adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestVulnCheckAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"vulncheck": {"enabled": True, "api_key": "test-key"}})["vulncheck"]

    def _adapter_no_key(self):
        return discover_engines({"vulncheck": {"enabled": True, "api_key": ""}})["vulncheck"]

    async def test_search_with_cve_id(self, adapter):
        data = {
            "data": {
                "exploit_state": "exploited",
                "date_added": "2024-04-01",
                "vendor_data": [{"vendor": "apache"}],
                "exploit_urls": ["https://example.com/exploit"],
            }
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("CVE-2024-3094")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "exploited" in result.results[0].content

    async def test_without_cve_id_returns_empty(self, adapter):
        result = await adapter.search("linux kernel")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.ERROR
        assert "API key" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "vulncheck" in list_engines()
