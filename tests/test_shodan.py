"""Tests for Shodan adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestShodanAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"shodan": {"enabled": True, "api_key": "test-key"}})["shodan"]

    def _adapter_no_key(self):
        return discover_engines({"shodan": {"enabled": True, "api_key": ""}})["shodan"]

    async def test_search_returns_results(self, adapter):
        data = {
            "matches": [
                {
                    "ip_str": "1.2.3.4",
                    "port": 80,
                    "org": "Example ISP",
                    "product": "nginx",
                    "hostnames": ["www.example.com"],
                    "transport": "tcp",
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("nginx")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "1.2.3.4:80" in result.results[0].title
        assert "nginx" in result.results[0].content

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "shodan" in list_engines()
