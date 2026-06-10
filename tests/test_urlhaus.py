"""Tests for URLhaus adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestURLhausAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"urlhaus": {"enabled": True}})["urlhaus"]

    @pytest.fixture
    def sample_url_response(self) -> dict:
        return {
            "query_status": "ok",
            "url_id": "123456",
            "url": "https://evil.example.com/malware.exe",
            "host": "evil.example.com",
            "threat": "malware_download",
            "tags": ["elf", "mirai"],
            "file_type": "exe",
            "url_status": "online",
            "firstseen": "2026-06-01",
            "lastseen": "2026-06-10",
        }

    async def test_url_search(self, adapter, sample_url_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_url_response)):
            result = await adapter.search("https://evil.example.com/malware.exe")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "malware_download" in result.results[0].content
        assert "Status: online" in result.results[0].content

    async def test_host_search(self, adapter, sample_url_response):
        async with MockHTTP(lambda r: httpx.Response(200, json={"query_status": "ok", "urls": [sample_url_response]})):
            result = await adapter.search("evil.example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1

    async def test_no_results(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"query_status": "no_results"})):
            result = await adapter.search("clean.example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines
        assert "urlhaus" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines
        assert "threat-intel" in list_engines()["urlhaus"].categories
