"""Tests for URLhaus adapter."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


class TestURLhausAdapter:
    @pytest.fixture
    def adapter(self):
        return discover_engines({"urlhaus": {"enabled": True, "api_key": "test-auth-key"}})["urlhaus"]

    @pytest.fixture
    def adapter_without_auth(self):
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

    @pytest.mark.parametrize(
        ("query", "query_status"),
        [
            ("invalid-host.example", "invalid_host"),
            ("https://example.com/not-malware", "invalid_url"),
            ("0123456789abcdef0123456789abcdef", "invalid_md5"),
        ],
    )
    async def test_urlhaus_documented_invalid_input_is_error(self, adapter, query, query_status):
        async with MockHTTP(lambda r: httpx.Response(200, json={"query_status": query_status})):
            result = await adapter.search(query)
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert query_status in (result.error_message or "")

    @pytest.mark.parametrize(
        ("query", "query_status"),
        [
            ("example.com", "invalid_url"),
            ("https://example.com/not-malware", "invalid_host"),
            ("0123456789abcdef0123456789abcdef", "invalid_host"),
        ],
    )
    async def test_invalid_input_status_for_wrong_query_kind_is_error(self, adapter, query, query_status):
        async with MockHTTP(lambda r: httpx.Response(200, json={"query_status": query_status})):
            result = await adapter.search(query)
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    @pytest.mark.parametrize(
        "query",
        [
            "https://clean.example.com/not-malware",
            "clean.example.com",
            "0123456789abcdef0123456789abcdef",
        ],
    )
    async def test_no_results(self, adapter, query):
        async with MockHTTP(lambda r: httpx.Response(200, json={"query_status": "no_results"})):
            result = await adapter.search(query)
        assert result.status == EngineStatus.OK
        assert len(result.results) == 0

    async def test_missing_auth_is_unavailable_without_http_dispatch(self, adapter_without_auth):
        with patch("httpx.AsyncClient") as async_client:
            result = await adapter_without_auth.search("example.com")
        assert result.status == EngineStatus.UNAVAILABLE
        assert result.error_message == "URLhaus requires ENGINE_URLHAUS_API_KEY"
        async_client.assert_not_called()

    async def test_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("test")
        assert result.status == EngineStatus.RATE_LIMITED

    async def test_invalid_request_is_error(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(400)):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []
        assert result.error_message == "URLhaus rejected request (HTTP 400)"

    @pytest.mark.parametrize("status_code", [401, 403])
    async def test_auth_or_provider_block_is_classified(self, adapter, status_code):
        async with MockHTTP(lambda r: httpx.Response(status_code)):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.BLOCKED
        assert result.results == []
        assert result.error_message

    async def test_malformed_json_is_error(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, content=b"not json")):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    @pytest.mark.parametrize(
        "payload",
        [
            {"query_status": "ok"},
            {"query_status": "ok", "urls": "not-a-list"},
            {"query_status": "provider_error"},
        ],
    )
    async def test_unexpected_success_shape_is_error(self, adapter, payload):
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("example.com")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_unsupported_input_is_error_without_dispatch(self, adapter):
        async def fail_if_called(request):
            raise AssertionError("unsupported input must not call URLhaus")

        async with MockHTTP(fail_if_called):
            result = await adapter.search("!@#$%")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_auth_key_is_forwarded_as_header(self, sample_url_response):
        adapter = discover_engines({"urlhaus": {"enabled": True, "api_key": "test-auth-key"}})["urlhaus"]
        captured_headers = {}

        def handler(request):
            captured_headers.update(request.headers)
            return httpx.Response(200, json=sample_url_response)

        async with MockHTTP(handler):
            result = await adapter.search("https://evil.example.com/malware.exe")
        assert result.status == EngineStatus.OK
        assert captured_headers["auth-key"] == "test-auth-key"

    def test_adapter_registered(self):
        from slopsearx.adapter import list_engines

        assert "urlhaus" in list_engines()

    def test_adapter_categories(self):
        from slopsearx.adapter import list_engines

        assert "threat-intel" in list_engines()["urlhaus"].categories
