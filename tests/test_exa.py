"""Tests for the Exa Search API adapter (issue #272).

Beyond the ordinary parse/classify coverage, these pin the three boundaries
the adapter exists to hold: it never asks for page bodies, never asks for
model-authored prose, and ignores both if the upstream returns them anyway.
"""

from __future__ import annotations

import json

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from slopsearx.capabilities import REQUIRED_KEY_ENGINES
from slopsearx.filters import time_range_window
from slopsearx.service import DEFAULT_TIER1_ENGINES
from tests.test_adapters import MockHTTP

_SAMPLE = {
    "requestId": "req-1",
    "resolvedSearchType": "neural",
    "results": [
        {
            "id": "https://example.com/a",
            "url": "https://example.com/a",
            "title": "Alpha",
            "publishedDate": "2026-01-02T00:00:00.000Z",
            "author": "A. Author",
            "score": 0.42,
            "highlights": ["first passage", "second passage"],
            "highlightScores": [0.9, 0.8],
        },
        {
            "id": "https://example.com/b",
            "url": "https://example.com/b",
            "title": "Beta",
            "highlights": [],
        },
    ],
    "costDollars": {"total": 0.005},
}


def _capture(response: httpx.Response, sink: list[httpx.Request]):
    def _handler(request: httpx.Request) -> httpx.Response:
        sink.append(request)
        return response

    return _handler


def _body(request: httpx.Request) -> dict:
    return json.loads(request.content)


@pytest.fixture(autouse=True)
def _no_ambient_key(monkeypatch):
    """Never let a developer's real key leak into these tests."""
    monkeypatch.delenv("ENGINE_EXA_API_KEY", raising=False)


@pytest.fixture
def adapter():
    return discover_engines({"exa": {"enabled": True, "api_key": "test-key"}})["exa"]


class TestExaRegistration:
    def test_registered(self):
        assert "exa" in list_engines()

    def test_is_tier_two_by_default(self):
        assert "exa" not in DEFAULT_TIER1_ENGINES

    def test_requires_a_key_to_be_routable(self):
        assert "exa" in REQUIRED_KEY_ENGINES

    def test_declares_no_filter_enforcement(self, adapter):
        # Consumption is declared; enforcement is not, pending a live audit.
        assert adapter.supported_filters == {"date_from": True, "date_to": True, "time_range": True}
        assert adapter.enforced_filters == {}

    def test_declares_only_text_results(self, adapter):
        assert adapter.supported_result_types == ("text",)
        assert adapter.supported_media_types == ()
        assert adapter.cost_class == "freemium"


class TestExaRequestShape:
    async def test_posts_to_search_endpoint_with_api_key_header(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("test query")
        request = seen[0]
        assert request.method == "POST"
        assert str(request.url) == "https://api.exa.ai/search"
        assert request.headers["x-api-key"] == "test-key"
        assert _body(request)["query"] == "test query"

    async def test_requests_highlights_only_never_text_or_summary(self, adapter):
        """The search-only boundary, asserted on the wire."""
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("test query")
        contents = _body(seen[0])["contents"]
        assert set(contents) == {"highlights"}
        assert contents["highlights"]["maxCharacters"] == 500
        assert contents["highlights"]["query"] == "test query"

    async def test_pins_the_standard_search_tier(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["type"] == "auto"

    async def test_result_count_is_clamped_to_the_ceiling(self):
        """10 is the largest count with a documented basis; higher is clamped."""
        adapter = discover_engines({"exa": {"api_key": "k", "max_results": 5_000}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["numResults"] == 10

    async def test_unparseable_result_count_falls_back_to_the_default(self):
        adapter = discover_engines({"exa": {"api_key": "k", "max_results": "lots"}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["numResults"] == 10

    async def test_news_category_sets_the_exa_focus(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"categories": ["news"]})
        assert _body(seen[0])["category"] == "news"

    async def test_unmapped_category_sends_no_focus(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"categories": ["general"]})
        assert "category" not in _body(seen[0])


class TestExaCostBoundsAreNotConfigurable:
    """The cost-shaping choices are pinned, not tunable — assert that on the wire."""

    @pytest.mark.parametrize(
        "escalation",
        [
            {"search_type": "deep"},
            {"type": "deep-reasoning"},
            {"contents": {"text": True, "summary": True}},
            {"text": True},
            {"summary": {"query": "summarize"}},
            {"snippet_max_chars": 100_000},
        ],
    )
    async def test_config_cannot_escalate_the_request(self, escalation):
        adapter = discover_engines({"exa": {"api_key": "k", **escalation}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        body = _body(seen[0])
        assert body["type"] == "auto"
        assert set(body["contents"]) == {"highlights"}
        assert body["contents"]["highlights"]["maxCharacters"] == 500

    async def test_infinite_result_count_is_classified_not_raised(self):
        """YAML ``.inf`` reaches config as a float; int() would raise OverflowError."""
        adapter = discover_engines({"exa": {"api_key": "k", "max_results": float("inf")}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert _body(seen[0])["numResults"] == 10

    async def test_infinite_timeout_is_classified_not_raised(self):
        adapter = discover_engines({"exa": {"api_key": "k", "timeout_ms": float("inf")}})["exa"]
        async with MockHTTP(lambda r: httpx.Response(200, json=_SAMPLE)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK


class TestExaDateFilters:
    async def test_explicit_bounds_become_inclusive_publication_bounds(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"date_from": "2026-01-01", "date_to": "2026-02-01"})
        body = _body(seen[0])
        assert body["startPublishedDate"] == "2026-01-01T00:00:00.000Z"
        assert body["endPublishedDate"] == "2026-02-01T23:59:59.999Z"

    async def test_time_range_expands_to_absolute_bounds(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"time_range": "week"})
        window = time_range_window("week")
        assert window is not None
        start, end = window
        body = _body(seen[0])
        assert body["startPublishedDate"] == f"{start.isoformat()}T00:00:00.000Z"
        assert body["endPublishedDate"] == f"{end.isoformat()}T23:59:59.999Z"

    async def test_explicit_bounds_win_over_a_relative_window(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"date_from": "2026-01-01", "time_range": "day"})
        assert _body(seen[0])["startPublishedDate"] == "2026-01-01T00:00:00.000Z"

    async def test_unknown_time_range_fabricates_no_window(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"time_range": "fortnight"})
        body = _body(seen[0])
        assert "startPublishedDate" not in body
        assert "endPublishedDate" not in body

    async def test_malformed_date_is_classified_without_dispatching(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q", {"date_from": "01-01-2026"})
        assert result.status == EngineStatus.ERROR
        assert "date_from" in (result.error_message or "")
        assert seen == []


class TestExaParsing:
    async def test_parses_results_from_highlights(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=_SAMPLE)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert [r.url for r in result.results] == ["https://example.com/a", "https://example.com/b"]
        first = result.results[0]
        assert first.title == "Alpha"
        assert first.content == "first passage … second passage"
        assert first.engine == "exa"
        assert first.position == 1
        assert first.published_date == "2026-01-02T00:00:00.000Z"
        # No highlights means no snippet — never a fabricated one.
        assert result.results[1].content == ""
        assert result.results[1].published_date is None

    async def test_page_text_and_summary_are_ignored_if_returned_unasked(self, adapter):
        """The boundary holds even if the upstream volunteers content."""
        payload = {
            "results": [
                {
                    "url": "https://example.com/a",
                    "title": "Alpha",
                    "text": "THE ENTIRE PAGE BODY " * 100,
                    "summary": "A model-authored summary of the page.",
                    "highlights": ["a retrieved passage"],
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        content = result.results[0].content
        assert content == "a retrieved passage"
        assert "PAGE BODY" not in content
        assert "model-authored" not in content

    async def test_page_text_and_summary_are_ignored_when_there_is_no_highlight(self, adapter):
        """The shape the boundary exists for: a body/summary and no usable highlight.

        A fallback to ``text`` or ``summary`` must leave the snippet empty
        rather than smuggling a page body or generated prose into it.
        """
        payload = {
            "results": [
                {
                    "url": "https://example.com/a",
                    "title": "Alpha",
                    "text": "THE ENTIRE PAGE BODY " * 100,
                    "summary": "A model-authored summary of the page.",
                },
                {
                    "url": "https://example.com/b",
                    "title": "Beta",
                    "text": "ANOTHER PAGE BODY",
                    "highlights": [],
                },
                {
                    "url": "https://example.com/c",
                    "title": "Gamma",
                    "summary": "Another model-authored summary.",
                    # Blank and non-string entries are skipped, not stringified.
                    "highlights": ["   ", "", None, 42, {"nested": "object"}],
                },
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert [r.content for r in result.results] == ["", "", ""]

    async def test_snippet_is_bounded(self, adapter):
        payload = {"results": [{"url": "https://example.com/a", "highlights": ["x" * 5_000]}]}
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert len(result.results[0].content) == 500

    async def test_records_without_a_usable_url_are_dropped(self, adapter):
        payload = {
            "results": [
                {"title": "no url"},
                {"url": "", "title": "empty url"},
                {"url": 42, "title": "non-string url"},
                "not-a-dict",
                {"url": "https://example.com/keep", "title": "kept"},
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert [r.url for r in result.results] == ["https://example.com/keep"]
        assert result.results[0].position == 1

    async def test_result_count_is_bounded_by_the_request(self):
        adapter = discover_engines({"exa": {"api_key": "k", "max_results": 2}})["exa"]
        payload = {"results": [{"url": f"https://example.com/{i}"} for i in range(10)]}
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert len(result.results) == 2

    @pytest.mark.parametrize("body", ["[]", '"a string"', "{}", '{"results": "nope"}'])
    async def test_unexpected_body_shapes_yield_no_results(self, adapter, body):
        async with MockHTTP(lambda r: httpx.Response(200, content=body, headers={"content-type": "application/json"})):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert result.results == []


class TestExaCredentialSafety:
    async def test_a_key_with_a_control_character_never_reaches_the_wire(self):
        """h11 echoes the whole header value in its exception text."""
        adapter = discover_engines({"exa": {"api_key": "exa-a\nexa-SUPERSECRET"}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert "malformed" in (result.error_message or "")
        assert "SUPERSECRET" not in (result.error_message or "")
        assert seen == []

    @pytest.mark.parametrize("bad", ["k\nk2", "k\rk2", "k\x01k2", "k k2", "k\tk2"])
    async def test_malformed_keys_are_rejected_before_dispatch(self, bad):
        adapter = discover_engines({"exa": {"api_key": bad}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert seen == []

    async def test_exception_text_is_redacted(self):
        """Defence in depth: a key echoed by any transport error is scrubbed."""
        adapter = discover_engines({"exa": {"api_key": "exa-SUPERSECRET"}})["exa"]

        def _raise(request):
            raise httpx.ConnectError("boom exa-SUPERSECRET boom", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert "SUPERSECRET" not in (result.error_message or "")
        assert "<redacted>" in (result.error_message or "")


class TestExaFieldBounds:
    async def test_oversized_sibling_fields_are_bounded(self, adapter):
        payload = {
            "results": [
                {
                    "url": "https://example.com/a",
                    "title": "T" * 100_000,
                    "publishedDate": "D" * 100_000,
                    "highlights": ["h" * 100_000],
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        r = result.results[0]
        assert len(r.title) == 500
        assert len(r.published_date or "") == 64
        assert len(r.content) == 500

    async def test_absurdly_long_urls_are_dropped(self, adapter):
        payload = {
            "results": [
                {"url": "https://example.com/" + "x" * 5_000, "title": "too long"},
                {"url": "https://example.com/keep", "title": "kept"},
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert [r.url for r in result.results] == ["https://example.com/keep"]

    async def test_a_huge_highlight_array_does_not_build_the_whole_join(self, adapter):
        """Work is bounded by the snippet bound, not by the response size."""
        payload = {"results": [{"url": "https://example.com/a", "highlights": ["y" * 2_000] * 50_000}]}
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert len(result.results[0].content) == 500


class TestExaErrorClassification:
    async def test_missing_api_key_never_dispatches(self):
        adapter = discover_engines({"exa": {"api_key": ""}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")
        assert seen == []

    async def test_whitespace_only_key_is_not_a_credential(self):
        adapter = discover_engines({"exa": {"api_key": "   "}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")
        assert seen == []

    @pytest.mark.parametrize(
        ("code", "expected"),
        [
            (429, EngineStatus.RATE_LIMITED),
            # 402 is credit/spending-budget exhaustion: an account state an
            # operator must act on, not a throttle that clears on its own.
            (402, EngineStatus.BLOCKED),
            (401, EngineStatus.ERROR),
            (403, EngineStatus.BLOCKED),
            (400, EngineStatus.ERROR),
            (404, EngineStatus.ERROR),
            (500, EngineStatus.UNAVAILABLE),
            (503, EngineStatus.UNAVAILABLE),
        ],
    )
    async def test_status_codes_are_classified(self, adapter, code, expected):
        async with MockHTTP(lambda r: httpx.Response(code)):
            result = await adapter.search("q")
        assert result.status == expected
        assert result.results == []

    async def test_timeout_is_classified(self, adapter):
        def _raise(request):
            raise httpx.ReadTimeout("timed out", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert result.status == EngineStatus.TIMEOUT

    async def test_transport_failure_is_classified_not_raised(self, adapter):
        def _raise(request):
            raise httpx.ConnectError("connection refused", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR

    async def test_undecodable_body_is_classified_not_raised(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, content=b"<html>nope</html>")):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR

    async def test_error_messages_never_leak_the_api_key(self):
        adapter = discover_engines({"exa": {"api_key": "super-secret-key"}})["exa"]

        def _raise(request):
            raise httpx.ConnectError(f"failed talking to {request.url}", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert "super-secret-key" not in (result.error_message or "")


class TestExaCredentials:
    async def test_key_is_resolved_from_the_environment(self, monkeypatch):
        """The production path: the key arrives as an env var, not in config."""
        adapter = discover_engines({"exa": {"enabled": True}})["exa"]
        monkeypatch.setenv("ENGINE_EXA_API_KEY", "env-key")
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert seen[0].headers["x-api-key"] == "env-key"

    async def test_configured_key_wins_over_the_environment(self, monkeypatch):
        monkeypatch.setenv("ENGINE_EXA_API_KEY", "env-key")
        adapter = discover_engines({"exa": {"api_key": "config-key"}})["exa"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert seen[0].headers["x-api-key"] == "config-key"


class TestExaRateLimiting:
    async def test_denied_request_is_reported_without_dispatching(self, adapter):
        class _Denying:
            async def acquire(self, name: str) -> bool:
                return False

        adapter.rate_limiter = _Denying()
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.latency_ms >= 0
        assert seen == []

    async def test_allowed_request_dispatches(self, adapter):
        class _Allowing:
            async def acquire(self, name: str) -> bool:
                return True

        adapter.rate_limiter = _Allowing()
        async with MockHTTP(lambda r: httpx.Response(200, json=_SAMPLE)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
