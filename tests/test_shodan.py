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

    async def test_search_http_status_error_sanitized(self):
        """HTTPStatusError (500) must not leak API key in error_message."""
        instances = discover_engines({"shodan": {"enabled": True, "api_key": "test-shodan-key-67890"}})
        adapter = instances["shodan"]

        async with MockHTTP(lambda r: httpx.Response(500)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "test-shodan-key-67890" not in result.error_message

    async def test_search_broad_exception_sanitized(self):
        """Broad handler sanitizes error_message when exception contains a URL."""
        instances = discover_engines({"shodan": {"enabled": True, "api_key": "test-shodan-key-67890"}})
        adapter = instances["shodan"]

        async with MockHTTP(lambda r: (_ for _ in ()).throw(RuntimeError(
            "https://api.shodan.io/shodan/host/search?key=test-shodan-key-67890&query=test&limit=10"
        ))):
            result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert result.error_message is not None
        assert "test-shodan-key-67890" not in result.error_message

    async def test_uses_bearer_auth_header(self):
        """Shodan sends API key as Authorization: Bearer header, not query param."""
        captured_headers = {}
        captured_params = {}

        def _handler(r):
            captured_headers.update(dict(r.headers))
            captured_params.update(dict(r.url.params))
            return httpx.Response(200, json={"matches": []})

        instances = discover_engines({"shodan": {"enabled": True, "api_key": "shodan-bearer-test"}})
        async with MockHTTP(_handler):
            await instances["shodan"].search("test")

        assert captured_headers.get("authorization") == "Bearer shodan-bearer-test"
        assert "key" not in captured_params

    async def test_empty_key_treated_as_not_configured(self):
        """Empty-string API key results in 'not configured' error, not a request."""
        instances = discover_engines({"shodan": {"enabled": True, "api_key": ""}})
        result = await instances["shodan"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    async def test_whitespace_key_treated_as_not_configured(self):
        """Whitespace-only API key treated as absent."""
        instances = discover_engines({"shodan": {"enabled": True, "api_key": "   "}})
        result = await instances["shodan"].search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "shodan" in list_engines()
