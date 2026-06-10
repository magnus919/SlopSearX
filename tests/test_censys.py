"""Tests for Censys adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestCensysAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"censys": {"enabled": True, "api_key": "test-id", "api_secret": "test-secret"}})[
            "censys"
        ]

    def _adapter_no_key(self):
        return discover_engines({"censys": {"enabled": True, "api_key": "", "api_secret": ""}})["censys"]

    async def test_search_returns_hits(self, adapter):
        data = {
            "result": {
                "hits": [
                    {
                        "ip": "1.2.3.4",
                        "location": {"country": "US", "city": "Mountain View"},
                        "services": [{"service_name": "HTTP", "port": 80}],
                    }
                ]
            }
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("1.2.3.4")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "1.2.3.4" in result.results[0].title

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API credentials" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "censys" in list_engines()
