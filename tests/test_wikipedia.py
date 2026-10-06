"""Deterministic Wikipedia two-stage response and diagnostics tests."""

from __future__ import annotations

import dataclasses

import httpx

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines
from tests.test_adapters import MockHTTP


def _adapter():
    return discover_engines({"wikipedia": {"enabled": True}})["wikipedia"]


def _sequenced_handler(responses: list[httpx.Response]):
    calls: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        return responses[min(len(calls) - 1, len(responses) - 1)]

    return handler, calls


def _opensearch(title: str = "Ada Lovelace") -> list[object]:
    return ["Ada", [title], [f"https://en.wikipedia.org/wiki/{title.replace(' ', '_')}"], [""]]


def _rich_query(title: str = "Ada Lovelace") -> dict[str, object]:
    return {
        "batchcomplete": "",
        "query": {
            "pages": {
                "42": {
                    "pageid": 42,
                    "ns": 0,
                    "title": title,
                    "extract": "Ada Lovelace was an English mathematician and writer.",
                },
            },
        },
    }


async def test_success_preserves_canonical_wikipedia_url() -> None:
    handler, _ = _sequenced_handler(
        [httpx.Response(200, json=_opensearch()), httpx.Response(200, json=_rich_query())],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.OK
    assert result.error_message is None
    assert result.results[0].url == "https://en.wikipedia.org/wiki/Ada_Lovelace"


async def test_rich_query_http_failure_reports_stage_without_body() -> None:
    handler, calls = _sequenced_handler(
        [
            httpx.Response(200, json=_opensearch()),
            httpx.Response(
                503,
                headers={"content-type": "application/json; charset=utf-8"},
                text='{"error":"provider detail that must not leak"}',
            ),
        ],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert len(calls) == 2
    assert result.status is EngineStatus.ERROR
    assert result.results == []
    assert result.error_message == (
        "Wikipedia stage=rich_query failed: status=503; content_type=application/json; "
        "response_shape=unparsed; provider HTTP error"
    )
    assert "provider detail" not in (result.error_message or "")


async def test_opensearch_forbidden_is_blocked_with_safe_diagnostics() -> None:
    async with MockHTTP(
        lambda request: httpx.Response(
            403,
            headers={"content-type": "application/json; charset=utf-8"},
            text='{"error":"forbidden detail that must not leak"}',
        ),
    ):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.BLOCKED
    assert result.error_message == (
        "Wikipedia stage=opensearch failed: status=403; content_type=application/json; "
        "response_shape=unparsed; provider HTTP error"
    )
    assert "forbidden detail" not in (result.error_message or "")


async def test_opensearch_rate_limit_is_classified_without_body_leak() -> None:
    async with MockHTTP(
        lambda request: httpx.Response(
            429,
            headers={"content-type": "application/json"},
            text='{"error":"rate limit detail that must not leak"}',
        ),
    ):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.RATE_LIMITED
    assert result.error_message == (
        "Wikipedia stage=opensearch failed: status=429; content_type=application/json; "
        "response_shape=unparsed; provider HTTP error"
    )
    assert "rate limit detail" not in (result.error_message or "")


async def test_unsafe_content_type_is_replaced_in_diagnostic() -> None:
    async with MockHTTP(
        lambda request: httpx.Response(
            403,
            headers={"content-type": "text/html\nInjected: yes"},
            text="forbidden detail",
        ),
    ):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.BLOCKED
    assert "content_type=invalid" in (result.error_message or "")
    assert "Injected" not in (result.error_message or "")


async def test_rich_query_forbidden_is_blocked_with_safe_diagnostics() -> None:
    handler, _ = _sequenced_handler(
        [
            httpx.Response(200, json=_opensearch()),
            httpx.Response(403, headers={"content-type": "text/html"}, text="forbidden detail"),
        ],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.BLOCKED
    assert result.error_message == (
        "Wikipedia stage=rich_query failed: status=403; content_type=text/html; "
        "response_shape=unparsed; provider HTTP error"
    )


def test_failure_classes_declare_blocked() -> None:
    assert "blocked" in _adapter().failure_classes


async def test_opensearch_timeout_preserves_timeout_status() -> None:
    async with MockHTTP(lambda request: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.TIMEOUT
    assert result.results == []
    assert "stage=opensearch" in (result.error_message or "")


async def test_opensearch_malformed_json_reports_stage_and_shape() -> None:
    async with MockHTTP(
        lambda request: httpx.Response(200, headers={"content-type": "text/html"}, text="not json"),
    ):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.ERROR
    assert result.results == []
    assert result.error_message == (
        "Wikipedia stage=opensearch failed: status=200; content_type=text/html; "
        "response_shape=malformed_json; malformed JSON"
    )


async def test_rich_query_malformed_json_reports_rich_query_stage() -> None:
    handler, _ = _sequenced_handler(
        [
            httpx.Response(200, json=_opensearch()),
            httpx.Response(200, headers={"content-type": "text/html"}, text="not json"),
        ],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.ERROR
    assert result.error_message == (
        "Wikipedia stage=rich_query failed: status=200; content_type=text/html; "
        "response_shape=malformed_json; malformed JSON"
    )


async def test_valid_empty_opensearch_results_are_successful_and_skip_rich_query() -> None:
    handler, calls = _sequenced_handler([httpx.Response(200, json=["Ada", [], [], []])])

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert len(calls) == 1
    assert result.status is EngineStatus.OK
    assert result.results == []
    assert result.error_message is None


async def test_valid_empty_rich_query_results_are_successful() -> None:
    handler, _ = _sequenced_handler(
        [httpx.Response(200, json=_opensearch()), httpx.Response(200, json={"query": {"pages": {}}})],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.OK
    assert result.results == []
    assert result.error_message is None


async def test_user_agent_names_a_contact_url_on_both_stages() -> None:
    """Wikimedia's robot policy (https://w.wiki/4wJS) 403s agents without contact info (issue 674)."""
    handler, calls = _sequenced_handler(
        [httpx.Response(200, json=_opensearch()), httpx.Response(200, json=_rich_query())],
    )

    async with MockHTTP(handler):
        result = await _adapter().search("Ada")

    assert result.status is EngineStatus.OK
    assert len(calls) == 2
    for request in calls:
        user_agent = request.headers["User-Agent"]
        assert user_agent.startswith("SlopSearX/")
        assert "https://github.com/magnus919/SlopSearX" in user_agent


async def test_user_agent_can_be_overridden_by_config() -> None:
    handler, calls = _sequenced_handler(
        [httpx.Response(200, json=_opensearch()), httpx.Response(200, json=_rich_query())],
    )
    adapter = discover_engines(
        {"wikipedia": {"enabled": True, "user_agent": "MyDeploy/1.0 (ops@example.org)"}},
    )["wikipedia"]

    async with MockHTTP(handler):
        await adapter.search("Ada")

    assert [request.headers["User-Agent"] for request in calls] == ["MyDeploy/1.0 (ops@example.org)"] * 2


def test_user_agent_env_override_reaches_engine_config(monkeypatch) -> None:
    from slopsearx.config import load_config

    monkeypatch.setenv("ENGINE_WIKIPEDIA_USER_AGENT", "MyDeploy/1.0 (ops@example.org)")
    cfg = load_config("/nonexistent/slopsearx.yaml")

    assert cfg.engines["wikipedia"].user_agent == "MyDeploy/1.0 (ops@example.org)"
    assert dataclasses.asdict(cfg.engines["wikipedia"])["user_agent"] == "MyDeploy/1.0 (ops@example.org)"
