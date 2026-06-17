"""Tests for FRED adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP

SAMPLE_FRED_RESPONSE = {
    "seriess": [
        {
            "id": "GDP",
            "title": "Gross Domestic Product",
            "observation_start": "1947-01-01",
            "units": "Billions of Dollars",
            "frequency": "Quarterly",
            "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
            "popularity": 95,
            "notes": "Gross domestic product (GDP), the value of goods and services produced.",
        },
    ],
}


class TestFREDAdapterRegistration:
    def test_adapter_registered(self):
        assert "fred" in list_engines()


class TestFREDAdapterSearch:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"fred": {"enabled": True, "api_key": "test-fred-key"}})["fred"]

    async def test_search_returns_results(self, adapter):
        def _handler(r):
            return httpx.Response(200, json=SAMPLE_FRED_RESPONSE)

        async with MockHTTP(_handler):
            result = await adapter.search("GDP")

        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "GDP" in result.results[0].title
        assert "Gross Domestic Product" in result.results[0].title

    async def test_keeps_api_key_in_query_param(self):
        """FRED still uses api_key query param (only auth method supported)."""
        captured_url = ""

        def _handler(r):
            nonlocal captured_url
            captured_url = str(r.url)
            return httpx.Response(200, json={"seriess": []})

        instances = discover_engines({"fred": {"enabled": True, "api_key": "fred-query-test"}})
        async with MockHTTP(_handler):
            await instances["fred"].search("test")

        assert "api_key=fred-query-test" in captured_url

    async def test_missing_api_key_returns_not_configured(self):
        """Missing API key returns 'not configured' error."""
        instances = discover_engines({"fred": {"enabled": True, "api_key": ""}})
        result = await instances["fred"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_empty_key_treated_as_not_configured(self):
        """Empty-string key treated as not configured."""
        instances = discover_engines({"fred": {"enabled": True, "api_key": ""}})
        result = await instances["fred"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_whitespace_key_treated_as_not_configured(self):
        """Whitespace-only key treated as absent."""
        instances = discover_engines({"fred": {"enabled": True, "api_key": "   "}})
        result = await instances["fred"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_http_status_error_sanitized(self):
        """HTTPStatusError must not leak API key in error_message."""
        instances = discover_engines({"fred": {"enabled": True, "api_key": "fred-secret-12345"}})
        adapter = instances["fred"]

        def _handler(r):
            return httpx.Response(500)

        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "fred-secret-12345" not in result.error_message

    async def test_broad_exception_sanitized(self):
        """Broad handler sanitizes error_message when exception contains a URL."""
        instances = discover_engines({"fred": {"enabled": True, "api_key": "fred-secret-12345"}})
        adapter = instances["fred"]

        async with MockHTTP(
            lambda r: (_ for _ in ()).throw(
                RuntimeError(
                    "https://api.stlouisfed.org/fred/series/search?api_key=fred-secret-12345&file_type=json&search_text=test&limit=10"
                )
            )
        ):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "fred-secret-12345" not in result.error_message

    async def test_rate_limited(self, adapter):
        def _handler(r):
            return httpx.Response(429)

        async with MockHTTP(_handler):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            result = await adapter.search("test")
        assert result.status == EngineStatus.TIMEOUT
