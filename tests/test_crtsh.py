"""Tests for CRT.sh adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestCrtShAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"crtsh": {"enabled": True}})["crtsh"]

    @pytest.fixture
    def sample_certs(self) -> list[dict]:
        return [
            {
                "id": 12345,
                "common_name": "example.com",
                "issuer_name": "C=US, O=Let's Encrypt",
                "name_value": "example.com\nwww.example.com",
                "not_before": "2024-01-01T00:00:00",
                "not_after": "2025-01-01T00:00:00",
                "serial_number": "abc123",
            },
            {
                "id": 12346,
                "common_name": "test.org",
                "issuer_name": "C=US, O=DigiCert",
                "name_value": "test.org\nmail.test.org",
                "not_before": "2024-06-01T00:00:00",
                "not_after": "2025-06-01T00:00:00",
                "serial_number": "def456",
            },
        ]

    async def test_search_returns_certs(self, adapter, sample_certs):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_certs)):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "example.com"
        assert "Let's Encrypt" in result.results[0].content
        assert "SANs: example.com" in result.results[0].content
        assert result.results[1].title == "test.org"

    async def test_search_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=[])):
            result = await adapter.search("nonexistent.xyz")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "crtsh" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        assert "security" in list_engines()["crtsh"].categories
        assert "it" in list_engines()["crtsh"].categories
