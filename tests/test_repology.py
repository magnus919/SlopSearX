"""Deterministic status and response-shape tests for the Repology adapter."""

from __future__ import annotations

from unittest.mock import patch

import httpx

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines


class MockHTTP:
    def __init__(self, handler):
        self.transport = httpx.MockTransport(handler)

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


def _adapter():
    return discover_engines({"repology": {"enabled": True}})["repology"]


def _positive_response() -> dict:
    return {
        "curl": [
            {
                "repo": "debian",
                "version": "8.10.0",
                "summary": "Transfer data with URLs",
                "status": "newest",
            },
        ],
    }


def test_failure_metadata_covers_returned_failure_statuses():
    adapter = _adapter()

    assert set(adapter.failure_classes) >= {"rate_limited", "error", "timeout"}


async def test_positive_mapping_is_normalized():
    requests = []

    def handler(request):
        requests.append(request)
        return httpx.Response(200, json=_positive_response())

    async with MockHTTP(handler):
        result = await _adapter().search("curl")

    assert result.status is EngineStatus.OK
    assert len(result.results) == 1
    assert result.results[0].title == "curl 8.10.0"
    assert requests[0].url.path == "/api/v1/projects/"
    assert requests[0].url.params["search"] == "curl"


async def test_malformed_json_is_a_sanitized_error():
    async with MockHTTP(lambda request: httpx.Response(200, text="<!doctype html>upstream secret")):
        result = await _adapter().search("python")

    assert result.status is EngineStatus.ERROR
    assert result.error_message == "Repology returned malformed JSON; expected an object"
    assert "upstream secret" not in (result.error_message or "")


async def test_structurally_invalid_json_is_an_error():
    async with MockHTTP(lambda request: httpx.Response(200, json={"curl": {"repo": "debian"}})):
        result = await _adapter().search("curl")

    assert result.status is EngineStatus.ERROR
    assert result.error_message == (
        "Repology returned an unexpected JSON shape; expected a mapping of project names to package lists"
    )


async def test_invalid_package_entry_is_an_error_without_raising():
    async with MockHTTP(lambda request: httpx.Response(200, json={"curl": ["not-an-object"]})):
        result = await _adapter().search("curl")

    assert result.status is EngineStatus.ERROR
    assert result.error_message == "Repology returned an unexpected JSON shape; package entries must be objects"


async def test_upstream_http_error_is_sanitized():
    async with MockHTTP(
        lambda request: httpx.Response(
            503,
            text="provider secret response body",
            headers={"content-type": "text/html"},
        ),
    ):
        result = await _adapter().search("python")

    assert result.status is EngineStatus.ERROR
    assert result.error_message == "Repology upstream returned HTTP 503"
    assert "provider secret" not in (result.error_message or "")


async def test_rate_limit_is_classified():
    async with MockHTTP(lambda request: httpx.Response(429, text="rate limit details")):
        result = await _adapter().search("python")

    assert result.status is EngineStatus.RATE_LIMITED
    assert result.error_message == "Repology rate limited the request (HTTP 429)"
    assert "rate limit details" not in (result.error_message or "")


async def test_valid_empty_mapping_is_success():
    async with MockHTTP(lambda request: httpx.Response(200, json={})):
        result = await _adapter().search("nonexistent")

    assert result.status is EngineStatus.OK
    assert result.results == []
    assert result.error_message is None


async def test_timeout_is_classified():
    def handler(request):
        raise httpx.ReadTimeout("provider timeout details", request=request)

    async with MockHTTP(handler):
        result = await _adapter().search("python")

    assert result.status is EngineStatus.TIMEOUT
    assert result.error_message is None
