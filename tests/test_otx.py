"""Tests for AlienVault OTX adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestOTXAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"otx": {"enabled": True, "api_key": "test-key"}})["otx"]

    def _adapter_no_key(self):
        return discover_engines({"otx": {"enabled": True, "api_key": ""}})["otx"]

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

        assert "otx" in list_engines()
