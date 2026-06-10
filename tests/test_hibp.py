"""Tests for HIBP adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestHIBPAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"hibp": {"enabled": True, "api_key": "test-key"}})["hibp"]

    def _adapter_no_key(self):
        return discover_engines({"hibp": {"enabled": True, "api_key": ""}})["hibp"]

    async def test_search_returns_breaches(self, adapter):
        data = [
            {
                "Name": "Adobe",
                "Domain": "adobe.com",
                "BreachDate": "2013-10-04",
                "PwnCount": 15238235,
                "DataClasses": ["Email addresses", "Passwords"],
                "Description": "In October 2013, Adobe suffered a major breach.",
            }
        ]
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("test@example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "Adobe" in result.results[0].title
        assert "Email addresses" in result.results[0].content

    async def test_no_breaches_404(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("nonexistent@example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("test@example.com")
        assert result.status == EngineStatus.ERROR
        assert "API key" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test@example.com")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "hibp" in list_engines()
