"""Tests for the service-to-adapter engine configuration boundary."""

from __future__ import annotations

import dataclasses

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import discover_engines
from slopsearx.config import EngineEntry
from slopsearx.service import _normalize_engine_configs


def test_blank_base_url_is_omitted_without_mutating_other_config() -> None:
    raw = {
        "enabled": True,
        "base_url": "  \t",
        "timeout_ms": 1234,
        "api_key": "configured",
        "categories": ["reference"],
        "_feature_brave_category_routing": True,
    }

    normalized = _normalize_engine_configs({"example": raw})

    assert "base_url" not in normalized["example"]
    assert normalized["example"]["timeout_ms"] == 1234
    assert normalized["example"]["api_key"] == "configured"
    assert normalized["example"]["categories"] == ["reference"]
    assert normalized["example"]["_feature_brave_category_routing"] is True
    assert raw["base_url"] == "  \t"


def test_none_base_url_is_treated_as_unset() -> None:
    normalized = _normalize_engine_configs({"example": {"base_url": None, "api_key": "configured"}})

    assert normalized == {"example": {"api_key": "configured"}}


def test_nonempty_base_url_override_is_preserved_exactly() -> None:
    custom_url = "https://operator.example/search"

    normalized = _normalize_engine_configs({"example": {"base_url": custom_url, "timeout_ms": 900}})

    assert normalized["example"] == {"base_url": custom_url, "timeout_ms": 900}


@pytest.mark.parametrize(
    ("name", "api_key", "expected_endpoint"),
    [
        ("github", "configured", "https://api.github.com/search/repositories"),
        ("reddit", "", "https://www.reddit.com/search.json"),
        (
            "semanticscholar",
            "configured",
            "https://api.semanticscholar.org/graph/v1/paper/search",
        ),
        ("urlhaus", "configured", "https://urlhaus-api.abuse.ch/v1/host/"),
    ],
)
async def test_configured_engines_retain_adapter_default_urls(
    name: str, api_key: str, expected_endpoint: str
) -> None:
    entry = EngineEntry(timeout_ms=750, api_key=api_key)
    normalized = _normalize_engine_configs({name: dataclasses.asdict(entry)})
    adapter = discover_engines(normalized)[name]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"items": [], "data": [], "query_status": "no_results"})

    adapter.set_http_transport(httpx.MockTransport(handler))
    await adapter.search("example.com")

    assert "base_url" not in adapter.config
    assert len(requests) == 1
    assert str(requests[0].url).split("?", maxsplit=1)[0] == expected_endpoint
    assert adapter.config["timeout_ms"] == 750
    assert adapter.config["api_key"] == api_key


async def test_configured_engine_uses_nonempty_custom_url() -> None:
    custom_url = "https://operator.example/provider"
    entry = EngineEntry(base_url=custom_url, timeout_ms=900, api_key="configured")

    adapter = discover_engines(_normalize_engine_configs({"github": dataclasses.asdict(entry)}))["github"]
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"items": []})

    adapter.set_http_transport(httpx.MockTransport(handler))
    await adapter.search("example")

    assert adapter.config["base_url"] == custom_url
    assert len(requests) == 1
    assert str(requests[0].url).split("?", maxsplit=1)[0] == "https://operator.example/provider/search/repositories"
    assert adapter.config["timeout_ms"] == 900
    assert adapter.config["api_key"] == "configured"
