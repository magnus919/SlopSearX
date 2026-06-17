"""Tests for GreyNoise adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestGreyNoiseAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"greynoise": {"enabled": True}})["greynoise"]

    @pytest.fixture
    def sample_ip(self) -> dict:
        return {
            "ip": "8.8.8.8",
            "noise": True,
            "riot": True,
            "classification": "benign",
            "name": "Google DNS",
            "link": "https://viz.greynoise.io/ip/8.8.8.8",
            "last_seen": "2026-06-10",
            "tags": ["dns", "anycast"],
        }

    async def test_ip_lookup(self, adapter, sample_ip):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_ip)):
            result = await adapter.search("8.8.8.8")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "background scanner" in result.results[0].content
        assert "benign" in result.results[0].content

    async def test_no_ip_in_query(self, adapter):
        result = await adapter.search("google.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_404(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("10.0.0.1")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("8.8.8.8")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_with_api_key(self):
        adapter = discover_engines({"greynoise": {"enabled": True, "api_key": "test-key"}})["greynoise"]
        headers = {}

        def _handler(r):
            headers["key"] = r.headers.get("key", "")
            return httpx.Response(200, json={"ip": "8.8.8.8", "noise": False, "riot": False})

        async with MockHTTP(_handler):
            await adapter.search("8.8.8.8")
        assert headers.get("key") == "test-key"

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "greynoise" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        assert "threat-intel" in list_engines()["greynoise"].categories
