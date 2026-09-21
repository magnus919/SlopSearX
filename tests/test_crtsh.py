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
        assert result.error_message is None

    async def test_search_rejects_non_list_json(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"error": "temporarily unavailable"})):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert result.error_message == (
            "CRT.sh returned an unexpected JSON shape; expected a list (HTTP 200, content-type application/json)"
        )

    async def test_search_rejects_malformed_json(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, text="not json")):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert result.error_message == "CRT.sh returned malformed JSON (HTTP 200, content-type text/plain)"

    async def test_search_classifies_html_challenge_without_body(self, adapter):
        html = "<html><body><div class='captcha'>verify you are human</div></body></html>"
        async with MockHTTP(
            lambda r: httpx.Response(
                200,
                text=html,
                headers={"content-type": "text/html; charset=UTF-8"},
            )
        ):
            result = await adapter.search("github.com")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []
        assert result.error_message == (
            "CRT.sh returned an upstream challenge/block page "
            "(HTTP 200, content-type text/html; markers=captcha,verify_human)"
        )
        assert html not in (result.error_message or "")

    async def test_search_classifies_nonchallenge_html_as_upstream_error(self, adapter):
        async with MockHTTP(
            lambda r: httpx.Response(
                200,
                text="<html><body>maintenance</body></html>",
                headers={"content-type": "text/html; charset=UTF-8"},
            )
        ):
            result = await adapter.search("github.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert result.error_message == (
            "CRT.sh returned non-JSON HTML (HTTP 200, content-type text/html; no challenge markers)"
        )

    async def test_search_rejects_malformed_certificate_row(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=[{"id": 1}, "not-a-certificate-row"])):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert result.error_message == (
            "CRT.sh returned a malformed certificate row (HTTP 200, content-type application/json)"
        )

    async def test_search_blocked(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(403)):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []

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
