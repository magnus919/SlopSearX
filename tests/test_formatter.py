"""Tests for response formatters — JSON and YAML+Markdown."""

from __future__ import annotations

import yaml

from slopsearx.adapter import SearchResult
from slopsearx.formatter import (
    format_json,
    format_yaml,
    format_yaml_markdown,
)


def _make_result(
    url: str,
    title: str,
    content: str = "",
    engine: str = "brave",
    score: float = 1.0,
    position: int = 1,
    engines: set[str] | None = None,
    published_date: str | None = None,
    thumbnail: str | None = None,
    img_src: str | None = None,
) -> SearchResult:
    return SearchResult(
        url=url,
        title=title,
        content=content,
        engine=engine,
        engines=engines or {engine},
        score=score,
        position=position,
        published_date=published_date,
        thumbnail=thumbnail,
        img_src=img_src,
    )


# ---------------------------------------------------------------------------
# JSON Formatter
# ---------------------------------------------------------------------------


class TestFormatJson:
    """SearXNG-compatible JSON output."""

    def test_empty_results(self) -> None:
        """Empty results produce valid response structure."""
        response = format_json(results=[], query="test")

        assert response["query"] == "test"
        assert response["results"] == []
        assert response["number_of_results"] == 0
        assert response["answers"] == []
        assert response["corrections"] == []
        assert response["infoboxes"] == []
        assert response["suggestions"] == []
        assert response["unresponsive_engines"] == []

    def test_single_result_all_fields(self) -> None:
        """Single result with all 23 SearXNG fields present."""
        result = _make_result(
            "https://example.com",
            "Example Title",
            content="This is a test result.",
            engine="brave",
            score=0.92,
            position=1,
            published_date="2025-11-15T00:00:00Z",
            thumbnail="https://example.com/thumb.jpg",
            img_src="https://example.com/img.jpg",
        )

        response = format_json(results=[result], query="test")

        assert response["number_of_results"] == 1
        r = response["results"][0]

        # All 23 SearXNG fields
        assert r["url"] == "https://example.com"
        assert r["title"] == "Example Title"
        assert r["content"] == "This is a test result."
        assert r["engine"] == "brave"
        assert r["engines"] == ["brave"]
        assert r["score"] == 0.92
        assert r["positions"] == [1]
        assert r["category"] == "general"
        assert r["publishedDate"] == "2025-11-15T00:00:00Z"
        assert r["pubdate"] == 1763164800  # epoch for 2025-11-15T00:00:00Z
        assert r["thumbnail"] == "https://example.com/thumb.jpg"
        assert r["img_src"] == "https://example.com/img.jpg"
        # Fields that should be null for basic results
        assert r["length"] is None
        assert r["iframe_src"] is None
        assert r["audio_src"] is None
        assert r["views"] is None
        assert r["author"] is None
        assert r["metadata"] is None
        assert r["template"] == "default.html"
        assert r["parsed_url"] is None
        assert r["open_group"] is False
        assert r["close_group"] is False
        assert r["priority"] == ""

    def test_multi_engine_result(self) -> None:
        """Multi-engine result shows all engines in 'engines' array."""
        result = _make_result(
            "https://example.com",
            "Shared",
            engine="brave",
            engines={"brave", "wikipedia"},
        )

        response = format_json(results=[result], query="test")
        r = response["results"][0]

        assert set(r["engines"]) == {"brave", "wikipedia"}

    def test_multiple_results_ordered(self) -> None:
        """Results preserve their position ordering."""
        r1 = _make_result("https://a.com", "A", position=1, score=2.0)
        r2 = _make_result("https://b.com", "B", position=2, score=1.0)

        response = format_json(results=[r1, r2], query="test")

        assert response["number_of_results"] == 2
        assert response["results"][0]["url"] == "https://a.com"
        assert response["results"][1]["url"] == "https://b.com"

    def test_unresponsive_engines(self) -> None:
        """unresponsive_engines list is passed through."""
        unresponsive = [["duckduckgo", "CAPTCHA detected"]]

        response = format_json(
            results=[],
            query="test",
            unresponsive_engines=unresponsive,
        )

        assert response["unresponsive_engines"] == unresponsive

    def test_meta_extension(self) -> None:
        """meta.* extension fields are injected."""
        meta = {
            "response_time_ms": 1420,
            "cached": False,
            "query_id": "ssx-abc12345",
            "engine_status": {
                "brave": {"results": 10, "latency_ms": 340, "status": "ok"},
            },
        }

        response = format_json(
            results=[],
            query="test",
            meta=meta,
        )

        assert response["meta"] == meta

    def test_engines_array_from_meta(self) -> None:
        """SearXNG-compatible engines array built from meta.engine_status."""
        meta = {
            "engine_status": {
                "brave": {"results": 10, "latency_ms": 340.0, "status": "ok"},
                "duckduckgo": {"results": 0, "latency_ms": 0.0, "status": "error"},
            },
        }

        response = format_json(results=[], query="test", meta=meta)

        assert "engines" in response
        assert response["engines"] == [
            {"engine": "brave", "results": 10},
            {"engine": "duckduckgo", "results": 0},
        ]
        # Each entry has exactly engine and results keys
        for entry in response["engines"]:
            assert set(entry.keys()) == {"engine", "results"}

    def test_engines_array_omitted_without_meta(self) -> None:
        """engines key is absent when meta is None or lacks engine_status."""
        # No meta at all
        response = format_json(results=[], query="test")
        assert "engines" not in response

        # Meta without engine_status
        response = format_json(results=[], query="test", meta={"version": "1.0"})
        assert "engines" not in response

    def test_suggestions(self) -> None:
        """Query suggestions are included."""
        suggestions = ["better query", "alternative search"]
        response = format_json(results=[], query="test", suggestions=suggestions)
        assert response["suggestions"] == suggestions

    def test_answers_and_corrections(self) -> None:
        """Answers and corrections arrays are passed through."""
        answers = [{"answer": "42", "url": "https://example.com"}]
        corrections = [{"original": "pythn", "correction": "python"}]

        response = format_json(
            results=[],
            query="test",
            answers=answers,
            corrections=corrections,
        )

        assert response["answers"] == answers
        assert response["corrections"] == corrections

    def test_published_date_iso_to_epoch(self) -> None:
        """ISO date is correctly converted to Unix epoch."""
        result = _make_result(
            "https://example.com",
            "Test",
            published_date="2024-01-01T00:00:00Z",
        )
        response = format_json(results=[result], query="test")
        # Jan 1 2024 UTC = 1704067200
        assert response["results"][0]["pubdate"] == 1704067200

    def test_null_published_date(self) -> None:
        """Null published_date produces null pubdate."""
        result = _make_result("https://example.com", "Test", published_date=None)
        response = format_json(results=[result], query="test")
        assert response["results"][0]["pubdate"] is None

    def test_number_of_results_explicit(self) -> None:
        """Explicit number_of_results overrides len(results)."""
        response = format_json(
            results=[_make_result("https://a.com", "A")],
            query="test",
            number_of_results=42,
        )
        assert response["number_of_results"] == 42


# ---------------------------------------------------------------------------
# YAML+Markdown Formatter
# ---------------------------------------------------------------------------


class TestFormatYamlMarkdown:
    """Agent-native YAML+Markdown output."""

    def test_basic_structure(self) -> None:
        """Output is valid YAML with Markdown body."""
        result = _make_result(
            "https://example.com",
            "Example",
            content="A comprehensive guide.",
            engine="brave",
            score=0.92,
            position=1,
        )

        meta = {
            "response_time_ms": 1420,
            "cached": False,
            "query_id": "ssx-test",
            "engine_status": {
                "brave": {"results": 1, "latency_ms": 340, "status": "ok"},
            },
        }

        output = format_yaml_markdown(
            [result],
            "test query",
            meta=meta,
            engine_count=4,
            responsive_count=1,
        )

        # Split on YAML/Markdown separator
        parts = output.split("---\n", 1)
        assert len(parts) == 2, f"Expected YAML + --- + Markdown, got: {output[:200]}"

        yaml_part, md_part = parts

        # YAML is valid
        parsed = yaml.safe_load(yaml_part)
        assert parsed["query"] == "test query"
        assert len(parsed["results"]) == 1
        assert parsed["results"][0]["url"] == "https://example.com"
        assert parsed["results"][0]["title"] == "Example"
        assert parsed["meta"]["response_time_ms"] == 1420

        # Markdown has summary
        assert "## Results Summary" in md_part
        assert "test query" in md_part

    def test_empty_results(self) -> None:
        """Empty results produce valid YAML+Markdown."""
        meta = {
            "response_time_ms": 50,
            "cached": False,
            "query_id": "ssx-empty",
            "engine_status": {},
        }

        output = format_yaml_markdown(
            [],
            "no results",
            meta=meta,
            engine_count=4,
            responsive_count=0,
        )

        parts = output.split("---\n", 1)
        assert len(parts) == 2

    def test_unresponsive_engines_in_markdown(self) -> None:
        """Blocked engines are noted in the Markdown body."""
        meta = {
            "response_time_ms": 100,
            "cached": False,
            "query_id": "ssx-test",
            "engine_status": {
                "brave": {"results": 1, "latency_ms": 100, "status": "ok"},
                "duckduckgo": {"results": 0, "latency_ms": 0, "status": "blocked"},
            },
        }

        unresponsive = [["duckduckgo", "CAPTCHA detected"]]

        output = format_yaml_markdown(
            [_make_result("https://a.com", "A")],
            "test",
            meta=meta,
            engine_count=2,
            responsive_count=1,
            unresponsive_engines=unresponsive,
        )

        assert "duckduckgo" in output
        assert "CAPTCHA" in output

    def test_content_snippet_truncation(self) -> None:
        """Long content is truncated in markdown summary."""
        long_content = "x" * 200
        result = _make_result("https://a.com", "Long", content=long_content)

        meta = {
            "response_time_ms": 100,
            "cached": False,
            "query_id": "ssx-test",
            "engine_status": {"brave": {"results": 1, "latency_ms": 100, "status": "ok"}},
        }

        output = format_yaml_markdown([result], "test", meta=meta, engine_count=1, responsive_count=1)

        # The markdown should contain a snippet <= ~120 chars
        assert long_content[:120] in output


# ---------------------------------------------------------------------------
# Backward-compat format_yaml stub
# ---------------------------------------------------------------------------


class TestFormatYamlStub:
    """Legacy format_yaml() wrapper."""

    def test_dict_to_search_result(self) -> None:
        """Dict results are converted to SearchResult and formatted."""
        output = format_yaml(
            results=[
                {"url": "https://a.com", "title": "A", "content": "test content", "engine": "brave"},
            ],
            query="test",
            response_time_ms=100,
            engine_count=4,
            responsive_count=2,
        )

        # Should be valid YAML+Markdown
        assert "test" in output
        parts = output.split("---\n", 1)
        assert len(parts) == 2
