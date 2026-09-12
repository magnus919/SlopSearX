"""Tests for the Tavily Search API adapter.

Beyond the ordinary parse/classify coverage, these pin the three boundaries
the adapter exists to hold: it never asks for page bodies, never asks for
Tavily's generated answer, and ignores both if the upstream returns them
anyway.
"""

from __future__ import annotations

import json

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from slopsearx.capabilities import REQUIRED_KEY_ENGINES
from slopsearx.service import DEFAULT_TIER1_ENGINES
from tests.test_adapters import MockHTTP

_SAMPLE = {
    "query": "test query",
    "follow_up_questions": None,
    "answer": None,
    "images": [],
    "results": [
        {
            "title": "Alpha",
            "url": "https://example.com/a",
            "content": "an extracted passage",
            "score": 0.99,
            "raw_content": None,
            "published_date": "2026-01-02",
        },
        {
            "title": "Beta",
            "url": "https://example.com/b",
            "content": "another extracted passage",
            "score": 0.7,
            "raw_content": None,
        },
    ],
    "response_time": 1.5,
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
    monkeypatch.delenv("ENGINE_TAVILY_API_KEY", raising=False)


@pytest.fixture
def adapter():
    return discover_engines({"tavily": {"enabled": True, "api_key": "test-key"}})["tavily"]


class TestTavilyRegistration:
    def test_registered(self):
        assert "tavily" in list_engines()

    def test_is_tier_two_by_default(self):
        assert "tavily" not in DEFAULT_TIER1_ENGINES

    def test_requires_a_key_to_be_routable(self):
        assert "tavily" in REQUIRED_KEY_ENGINES

    def test_declares_no_filter_enforcement(self, adapter):
        # Consumption is declared; enforcement is not, pending a live audit.
        assert adapter.supported_filters == {"date_from": True, "date_to": True, "time_range": True}
        assert adapter.enforced_filters == {}

    def test_declares_only_text_results(self, adapter):
        # The generated answer is never requested, so no answers channel is
        # advertised and none is populated.
        assert adapter.supported_result_types == ("text",)
        assert adapter.supported_media_types == ()
        assert adapter.cost_class == "freemium"


class TestTavilyRequestShape:
    async def test_posts_to_search_endpoint_with_bearer_auth(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("test query")
        request = seen[0]
        assert request.method == "POST"
        assert str(request.url) == "https://api.tavily.com/search"
        assert request.headers["authorization"] == "Bearer test-key"
        assert _body(request)["query"] == "test query"

    async def test_credit_bearing_extras_are_pinned_off(self, adapter):
        """The search-only and no-synthesis boundaries, asserted on the wire."""
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        body = _body(seen[0])
        assert body["include_answer"] is False
        assert body["include_raw_content"] is False
        assert body["include_images"] is False

    async def test_pins_the_basic_search_depth(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["search_depth"] == "basic"

    async def test_result_count_is_clamped_to_the_ceiling(self):
        adapter = discover_engines({"tavily": {"api_key": "k", "max_results": 5_000}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["max_results"] == 20

    async def test_unparseable_result_count_falls_back_to_the_default(self):
        adapter = discover_engines({"tavily": {"api_key": "k", "max_results": "lots"}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert _body(seen[0])["max_results"] == 10

    @pytest.mark.parametrize(("category", "topic"), [("news", "news"), ("finance", "finance")])
    async def test_mapped_categories_set_the_tavily_topic(self, adapter, category, topic):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"categories": [category]})
        assert _body(seen[0])["topic"] == topic

    async def test_unmapped_category_sends_no_topic(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"categories": ["general"]})
        assert "topic" not in _body(seen[0])


class TestTavilyCostBoundsAreNotConfigurable:
    """The cost-shaping choices are pinned, not tunable — assert that on the wire."""

    @pytest.mark.parametrize(
        "escalation",
        [
            {"search_depth": "advanced"},
            {"include_answer": True},
            {"include_answer": "advanced"},
            {"include_raw_content": True},
            {"include_images": True},
            {"auto_parameters": True},
            {"snippet_max_chars": 100_000},
        ],
    )
    async def test_config_cannot_escalate_the_request(self, escalation):
        adapter = discover_engines({"tavily": {"api_key": "k", **escalation}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        body = _body(seen[0])
        assert body["search_depth"] == "basic"
        assert body["include_answer"] is False
        assert body["include_raw_content"] is False
        assert body["include_images"] is False
        # auto_parameters is never sent: it can silently promote search_depth.
        assert "auto_parameters" not in body

    async def test_infinite_result_count_is_classified_not_raised(self):
        """YAML ``.inf`` reaches config as a float; int() would raise OverflowError."""
        adapter = discover_engines({"tavily": {"api_key": "k", "max_results": float("inf")}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert _body(seen[0])["max_results"] == 10

    async def test_infinite_timeout_is_classified_not_raised(self):
        adapter = discover_engines({"tavily": {"api_key": "k", "timeout_ms": float("inf")}})["tavily"]
        async with MockHTTP(lambda r: httpx.Response(200, json=_SAMPLE)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK


class TestTavilyDateFilters:
    async def test_time_range_maps_directly(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"time_range": "month"})
        assert _body(seen[0])["time_range"] == "month"

    async def test_unknown_time_range_is_not_forwarded(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"time_range": "fortnight"})
        assert "time_range" not in _body(seen[0])

    async def test_explicit_bounds_become_calendar_dates(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q", {"date_from": "2026-01-01", "date_to": "2026-02-01"})
        body = _body(seen[0])
        assert body["start_date"] == "2026-01-01"
        assert body["end_date"] == "2026-02-01"

    async def test_malformed_date_is_classified_without_dispatching(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q", {"date_to": "2026-13-45"})
        assert result.status == EngineStatus.ERROR
        assert "date_to" in (result.error_message or "")
        assert seen == []

    async def test_inverted_bounds_are_rejected_without_dispatching(self, adapter):
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q", {"date_from": "2026-02-01", "date_to": "2026-01-01"})
        assert result.status == EngineStatus.ERROR
        assert seen == []


class TestTavilyParsing:
    async def test_parses_results(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=_SAMPLE)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert [r.url for r in result.results] == ["https://example.com/a", "https://example.com/b"]
        first = result.results[0]
        assert first.title == "Alpha"
        assert first.content == "an extracted passage"
        assert first.engine == "tavily"
        assert first.position == 1
        assert first.published_date == "2026-01-02"
        assert result.results[1].published_date is None

    async def test_generated_answer_is_never_surfaced(self, adapter):
        """A model-authored answer must not reach results or the answers channel."""
        payload = dict(_SAMPLE, answer="Tavily's synthesized answer to the question.")
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert result.answers == []
        assert all("synthesized answer" not in r.content for r in result.results)

    async def test_raw_page_content_is_ignored_if_returned_unasked(self, adapter):
        payload = {
            "results": [
                {
                    "url": "https://example.com/a",
                    "title": "Alpha",
                    "content": "an extracted passage",
                    "raw_content": "THE ENTIRE PAGE BODY " * 100,
                }
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        content = result.results[0].content
        assert content == "an extracted passage"
        assert "PAGE BODY" not in content

    async def test_raw_page_content_is_ignored_when_there_is_no_passage(self, adapter):
        """The shape the boundary exists for: a page body and no extracted passage.

        A fallback to ``raw_content`` must leave the snippet empty rather
        than smuggling a page body into it.
        """
        payload = {
            "results": [
                {"url": "https://example.com/a", "title": "Alpha", "raw_content": "THE ENTIRE PAGE BODY " * 100},
                {"url": "https://example.com/b", "title": "Beta", "content": None, "raw_content": "ANOTHER BODY"},
                {"url": "https://example.com/c", "title": "Gamma", "content": "", "raw_content": "A THIRD BODY"},
            ]
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=payload)):
            result = await adapter.search("q")
        assert [r.content for r in result.results] == ["", "", ""]

    async def test_snippet_is_bounded(self, adapter):
        payload = {"results": [{"url": "https://example.com/a", "content": "x" * 5_000}]}
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
        adapter = discover_engines({"tavily": {"api_key": "k", "max_results": 2}})["tavily"]
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


class TestTavilyCredentialSafety:
    async def test_a_key_with_a_control_character_never_reaches_the_wire(self):
        """h11 echoes the whole header value in its exception text."""
        adapter = discover_engines({"tavily": {"api_key": "tvly-a\ntvly-SUPERSECRET"}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert "malformed" in (result.error_message or "")
        assert "SUPERSECRET" not in (result.error_message or "")
        assert seen == []

    @pytest.mark.parametrize("bad", ["k\nk2", "k\rk2", "k\x01k2", "k k2", "k\tk2"])
    async def test_malformed_keys_are_rejected_before_dispatch(self, bad):
        adapter = discover_engines({"tavily": {"api_key": bad}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert seen == []

    async def test_exception_text_is_redacted(self):
        """Defence in depth: a key echoed by any transport error is scrubbed."""
        adapter = discover_engines({"tavily": {"api_key": "tvly-SUPERSECRET"}})["tavily"]

        def _raise(request):
            raise httpx.ConnectError("boom tvly-SUPERSECRET boom", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert "SUPERSECRET" not in (result.error_message or "")
        assert "<redacted>" in (result.error_message or "")


class TestTavilyFieldBounds:
    async def test_oversized_sibling_fields_are_bounded(self, adapter):
        payload = {
            "results": [
                {
                    "url": "https://example.com/a",
                    "title": "T" * 100_000,
                    "published_date": "D" * 100_000,
                    "content": "c" * 100_000,
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


class TestTavilyErrorClassification:
    async def test_missing_api_key_never_dispatches(self):
        adapter = discover_engines({"tavily": {"api_key": ""}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.ERROR
        assert "API key not configured" in (result.error_message or "")
        assert seen == []

    async def test_whitespace_only_key_is_not_a_credential(self):
        adapter = discover_engines({"tavily": {"api_key": "   "}})["tavily"]
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
            # Plan/usage-limit exhaustion: the vendor SDK groups these with
            # 403, and they need operator action rather than waiting.
            (432, EngineStatus.BLOCKED),
            (433, EngineStatus.BLOCKED),
            (401, EngineStatus.ERROR),
            (403, EngineStatus.BLOCKED),
            (400, EngineStatus.ERROR),
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
        adapter = discover_engines({"tavily": {"api_key": "super-secret-key"}})["tavily"]

        def _raise(request):
            raise httpx.ConnectError(f"failed talking to {request.url}", request=request)

        async with MockHTTP(_raise):
            result = await adapter.search("q")
        assert "super-secret-key" not in (result.error_message or "")


class TestTavilyCredentials:
    async def test_key_is_resolved_from_the_environment(self, monkeypatch):
        """The production path: the key arrives as an env var, not in config."""
        adapter = discover_engines({"tavily": {"enabled": True}})["tavily"]
        monkeypatch.setenv("ENGINE_TAVILY_API_KEY", "env-key")
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            result = await adapter.search("q")
        assert result.status == EngineStatus.OK
        assert seen[0].headers["authorization"] == "Bearer env-key"

    async def test_configured_key_wins_over_the_environment(self, monkeypatch):
        monkeypatch.setenv("ENGINE_TAVILY_API_KEY", "env-key")
        adapter = discover_engines({"tavily": {"api_key": "config-key"}})["tavily"]
        seen: list[httpx.Request] = []
        async with MockHTTP(_capture(httpx.Response(200, json=_SAMPLE), seen)):
            await adapter.search("q")
        assert seen[0].headers["authorization"] == "Bearer config-key"


class TestTavilyRateLimiting:
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
