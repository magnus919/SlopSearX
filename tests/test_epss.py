"""Tests for FIRST EPSS adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestEPSSAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"epss": {"enabled": True}})["epss"]

    @pytest.fixture
    def sample_epss(self) -> dict:
        return {"data": [{"cve": "CVE-2024-3094", "epss": "0.96410", "percentile": "0.99990", "date": "2026-06-09"}]}

    async def test_search_with_cve_id(self, adapter, sample_epss):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_epss)):
            result = await adapter.search("CVE-2024-3094")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "EPSS Score: 0.96410" in result.results[0].content
        assert "Critical" in result.results[0].content

    async def test_search_without_cve_returns_empty(self, adapter):
        result = await adapter.search("linux kernel")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("CVE-2024-12345")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_search_lower_percentile(self, adapter):
        data = {"data": [{"cve": "CVE-2024-12345", "epss": "0.00012", "percentile": "0.15000", "date": "2026-06-09"}]}
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("CVE-2024-12345")
        assert "Very Low" in result.results[0].content

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines
        assert "epss" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines
        assert "threat-intel" in list_engines()["epss"].categories
