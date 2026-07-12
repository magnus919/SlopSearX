"""Tests for result merger, ranking, and metadata helpers."""

from __future__ import annotations

from slopsearx.adapter import AdapterResponse, EngineStatus, SearchResult
from slopsearx.merger import (
    PresenceRanker,
    build_engine_status,
    build_meta,
    extract_empty_scrape_engines,
    extract_unresponsive,
    merge_results,
)

# ---------------------------------------------------------------------------
# PresenceRanker — ranking behavior
# ---------------------------------------------------------------------------


class TestPresenceRanker:
    """V1 presence-weighted ranking."""

    def _make_result(self, url: str, title: str, engine: str, content: str = "") -> SearchResult:
        return SearchResult(url=url, title=title, content=content, engine=engine)

    def test_empty_results(self) -> None:
        """Empty input produces empty output."""
        ranker = PresenceRanker()
        assert ranker.rank({}, "test") == []

    def test_single_engine(self) -> None:
        """Single engine results pass through with positions assigned."""
        ranker = PresenceRanker()
        results = [
            self._make_result("https://a.com", "A", "brave"),
            self._make_result("https://b.com", "B", "brave"),
            self._make_result("https://c.com", "C", "brave"),
        ]
        engine_results = {"brave": results}
        ranked = ranker.rank(engine_results, "test")

        assert len(ranked) == 3
        assert ranked[0].position == 1
        assert ranked[1].position == 2
        assert ranked[2].position == 3

    def test_cross_engine_dedup(self) -> None:
        """Same URL across engines — deduplicated, presence boost."""
        ranker = PresenceRanker()
        engine_results = {
            "brave": [self._make_result("https://a.com", "A", "brave")],
            "wikipedia": [self._make_result("https://a.com", "A", "wikipedia")],
        }
        ranked = ranker.rank(engine_results, "test")

        assert len(ranked) == 1
        # Presence-weighted: score = 1.0 * 2 engines = 2.0
        assert ranked[0].score == 2.0
        assert ranked[0].engines == {"brave", "wikipedia"}

    def test_presence_boost(self) -> None:
        """Results in more engines rank higher."""
        ranker = PresenceRanker()
        engine_results = {
            "brave": [
                self._make_result("https://a.com", "Shared", "brave"),
                self._make_result("https://c.com", "Only Brave", "brave"),
            ],
            "wikipedia": [
                self._make_result("https://a.com", "Shared", "wikipedia"),
                self._make_result("https://d.com", "Only Wiki", "wikipedia"),
            ],
        }
        ranked = ranker.rank(engine_results, "test")

        assert len(ranked) == 3
        # Shared result (score=2.0) should be first
        assert ranked[0].url == "https://a.com"
        assert ranked[0].score == 2.0

    def test_multiple_engines_boost(self) -> None:
        """Result in 3 engines gets 3x boost."""
        ranker = PresenceRanker()
        engine_results = {
            "brave": [self._make_result("https://a.com", "Triple", "brave")],
            "wikipedia": [self._make_result("https://a.com", "Triple", "wikipedia")],
            "duckduckgo": [self._make_result("https://a.com", "Triple", "duckduckgo")],
        }
        ranked = ranker.rank(engine_results, "test")

        assert len(ranked) == 1
        assert ranked[0].score == 3.0  # 1.0 * 3 engines

    def test_tracking_params_stripped(self) -> None:
        """UTM params and fbclid are stripped for dedup."""
        ranker = PresenceRanker()
        engine_results = {
            "brave": [
                self._make_result(
                    "https://example.com/page?utm_source=twitter&fbclid=123",
                    "Page",
                    "brave",
                ),
            ],
            "google": [
                self._make_result(
                    "https://example.com/page?gclid=456",
                    "Page",
                    "google",
                ),
            ],
        }
        ranked = ranker.rank(engine_results, "test")

        assert len(ranked) == 1
        assert ranked[0].score == 2.0
        assert ranked[0].engines == {"brave", "google"}


# ---------------------------------------------------------------------------
# Per-engine result budget
# ---------------------------------------------------------------------------


class TestEngineBudget:
    """Per-engine result budget enforcement."""

    def _make_result(self, url: str, title: str, engine: str) -> SearchResult:
        return SearchResult(url=url, title=title, content="", engine=engine)

    def test_budget_limits_results(self) -> None:
        """Per-engine budget caps results from that engine."""
        ranker = PresenceRanker(per_engine_budget={"brave": 2})
        results = [self._make_result(f"https://brave-only-{i}.com", f"B{i}", "brave") for i in range(5)]
        ranked = ranker.rank({"brave": results}, "test")

        assert len(ranked) == 2

    def test_budget_zero_means_no_limit(self) -> None:
        """Budget of 0 (or not set) means no limit."""
        ranker = PresenceRanker(per_engine_budget={})
        results = [self._make_result(f"https://a-{i}.com", f"A{i}", "brave") for i in range(10)]
        ranked = ranker.rank({"brave": results}, "test")

        assert len(ranked) == 10

    def test_budget_per_engine_different(self) -> None:
        """Different engines can have different budgets."""
        ranker = PresenceRanker(per_engine_budget={"brave": 1, "wikipedia": 3})
        engine_results = {
            "brave": [
                self._make_result("https://b1.com", "B1", "brave"),
                self._make_result("https://b2.com", "B2", "brave"),
            ],
            "wikipedia": [
                self._make_result("https://w1.com", "W1", "wikipedia"),
                self._make_result("https://w2.com", "W2", "wikipedia"),
                self._make_result("https://w3.com", "W3", "wikipedia"),
                self._make_result("https://w4.com", "W4", "wikipedia"),
            ],
        }
        ranked = ranker.rank(engine_results, "test")

        # brave: capped at 1, wikipedia: capped at 3 = 4 total
        assert len(ranked) == 4


# ---------------------------------------------------------------------------
# Metadata helpers
# ---------------------------------------------------------------------------


class TestBuildEngineStatus:
    """build_engine_status() from adapter responses."""

    def _make_response(self, count: int, status: EngineStatus, latency: float = 100.0) -> AdapterResponse:
        results = [
            SearchResult(url=f"https://x{i}.com", title=f"X{i}", content="", engine="test") for i in range(count)
        ]
        return AdapterResponse(results=results, status=status, latency_ms=latency)

    def test_ok_engine(self) -> None:
        """OK engine reports correct result count and status."""
        responses = {
            "brave": self._make_response(5, EngineStatus.OK, 340.0),
        }
        status = build_engine_status(responses, 500.0)

        assert status["brave"]["results"] == 5
        assert status["brave"]["status"] == "ok"
        assert status["brave"]["latency_ms"] == 340.0

    def test_multiple_engines(self) -> None:
        """Multiple engines produce per-engine status map."""
        responses = {
            "brave": self._make_response(10, EngineStatus.OK),
            "duckduckgo": self._make_response(0, EngineStatus.BLOCKED),
        }
        status = build_engine_status(responses, 500.0)

        assert len(status) == 2
        assert status["duckduckgo"]["status"] == "blocked"
        assert status["duckduckgo"]["results"] == 0


class TestExtractUnresponsive:
    """extract_unresponsive() from adapter responses."""

    def _make_response(self, status: EngineStatus, error: str | None = None) -> AdapterResponse:
        return AdapterResponse(results=[], status=status, error_message=error)

    def test_all_ok_produces_empty(self) -> None:
        """No unresponsive engines when all are OK."""
        responses = {
            "brave": self._make_response(EngineStatus.OK),
            "wikipedia": self._make_response(EngineStatus.OK),
        }
        assert extract_unresponsive(responses) == []

    def test_blocked_engine_reported(self) -> None:
        """Blocked engine appears in unresponsive list."""
        responses = {
            "brave": self._make_response(EngineStatus.OK),
            "duckduckgo": self._make_response(EngineStatus.BLOCKED, "CAPTCHA detected"),
        }
        unresponsive = extract_unresponsive(responses)

        assert len(unresponsive) == 1
        assert unresponsive[0][0] == "duckduckgo"
        assert "CAPTCHA" in unresponsive[0][1]

    def test_multiple_unresponsive(self) -> None:
        """Multiple unresponsive engines all reported."""
        responses = {
            "duckduckgo": self._make_response(EngineStatus.BLOCKED),
            "google": self._make_response(EngineStatus.RATE_LIMITED),
            "brave": self._make_response(EngineStatus.OK),
        }
        unresponsive = extract_unresponsive(responses)

        assert len(unresponsive) == 2


class TestExtractEmptyScrapeEngines:
    """Opt-in diagnostics for anomalous successful scrape responses."""

    def test_only_successful_empty_scrape_engines_are_reported(self) -> None:
        responses = {
            "google": AdapterResponse(results=[], status=EngineStatus.OK),
            "duckduckgo": AdapterResponse(results=[], status=EngineStatus.BLOCKED),
            "brave": AdapterResponse(results=[], status=EngineStatus.OK),
            "wikipedia": AdapterResponse(
                results=[SearchResult(url="https://example.com", title="Result", content="", engine="wikipedia")],
                status=EngineStatus.OK,
            ),
        }

        assert extract_empty_scrape_engines(responses, {"google", "duckduckgo"}) == [
            [
                "google",
                "successful scrape returned no results",
            ]
        ]


class TestBuildMeta:
    """build_meta() extension field."""

    def _make_response(self, count: int, status: EngineStatus, latency: float = 100.0) -> AdapterResponse:
        results = [
            SearchResult(url=f"https://x{i}.com", title=f"X{i}", content="", engine="test") for i in range(count)
        ]
        return AdapterResponse(results=results, status=status, latency_ms=latency)

    def test_meta_structure(self) -> None:
        """Meta dict has expected fields."""
        responses = {
            "brave": self._make_response(10, EngineStatus.OK, 340.0),
        }
        meta = build_meta(responses, 1420.0, "ssx-abc12345")

        assert meta["response_time_ms"] == 1420
        assert meta["cached"] is False
        assert meta["query_id"] == "ssx-abc12345"
        assert "engine_status" in meta
        assert meta["engine_status"]["brave"]["results"] == 10

    def test_cached_flag(self) -> None:
        """Cached flag is respected."""
        responses: dict[str, AdapterResponse] = {}
        meta = build_meta(responses, 0, "ssx-test", cached=True)

        assert meta["cached"] is True

    def test_empty_engines_are_included_when_supplied(self) -> None:
        meta = build_meta(
            {},
            0,
            "ssx-test",
            empty_engines=[["google", "successful scrape returned no results"]],
        )

        assert meta["empty_engines"] == [["google", "successful scrape returned no results"]]


# ---------------------------------------------------------------------------
# Backward-compat merge_results wrapper
# ---------------------------------------------------------------------------


class TestMergeResultsWrapper:
    """Backward-compat merge_results() convenience function."""

    def _make_result(self, url: str, title: str, engine: str) -> SearchResult:
        return SearchResult(url=url, title=title, content="", engine=engine)

    def test_wraps_presence_ranker(self) -> None:
        """merge_results with strategy='presence' delegates to PresenceRanker."""
        engine_results = {
            "brave": [self._make_result("https://a.com", "A", "brave")],
            "wikipedia": [self._make_result("https://a.com", "A", "wikipedia")],
        }
        ranked = merge_results(engine_results, "presence")

        assert len(ranked) == 1
        assert ranked[0].score == 2.0  # presence-weighted boost

    def test_unknown_strategy_falls_back(self) -> None:
        """Unknown strategy string falls back to presence."""
        engine_results = {
            "brave": [self._make_result("https://a.com", "A", "brave")],
        }
        ranked = merge_results(engine_results, "cosmic_vibes")

        assert len(ranked) == 1
