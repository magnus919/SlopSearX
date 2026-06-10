"""Tests for VirusTotal adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestVirusTotalAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"virustotal": {"enabled": True, "api_key": "test-key"}})["virustotal"]

    def _adapter_no_key(self):
        return discover_engines({"virustotal": {"enabled": True, "api_key": ""}})["virustotal"]

    async def test_search_returns_results(self, adapter):
        data = {
            "data": [
                {
                    "id": "d41d8cd98f00b204e9800998ecf8427e",
                    "attributes": {
                        "last_analysis_stats": {"malicious": 5, "suspicious": 2, "harmless": 50, "undetected": 10},
                        "meaningful_name": "bad.exe",
                        "reputation": -5,
                    },
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("d41d8cd98f00b204e9800998ecf8427e")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "bad.exe" in result.results[0].title
        assert "5/67" in result.results[0].content

    async def test_missing_api_key(self):
        adapter = self._adapter_no_key()
        result = await adapter.search("test")
        assert result.status == EngineStatus.ERROR
        assert "API key" in (result.error_message or "")

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "virustotal" in list_engines()
