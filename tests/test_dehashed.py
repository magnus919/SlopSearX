"""Tests for DeHashed adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestDeHashedAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"dehashed": {"enabled": True, "api_key": "email@test.com:test-api-key"}})["dehashed"]

    def _adapter_no_key(self):
        return discover_engines({"dehashed": {"enabled": True, "api_key": ""}})["dehashed"]

    async def test_search_returns_entries(self, adapter):
        data = {
            "entries": [
                {
                    "email": "test@example.com",
                    "username": "testuser",
                    "password": "secret123",
                    "hashed_password": "hash123",
                    "ip_address": "1.2.3.4",
                    "database_name": "TestDB",
                    "obtained_from": "BreachCorp",
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=data)):
            result = await adapter.search("test@example.com")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1
        assert "test@example.com" in result.results[0].content
        assert "secr" in result.results[0].content  # truncated password appears

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

        assert "dehashed" in list_engines()
