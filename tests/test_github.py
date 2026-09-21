"""Tests for the GitHub API adapter."""

from __future__ import annotations

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines, list_engines
from tests.test_adapters import MockHTTP


@pytest.fixture
def public_adapter():
    return discover_engines({"github": {"enabled": True, "api_key": ""}})["github"]


@pytest.fixture
def authenticated_adapter():
    return discover_engines({"github": {"enabled": True, "api_key": "fixture-token"}})["github"]


class TestGitHubAdapter:
    def test_adapter_registered(self):
        assert "github" in list_engines()

    @pytest.mark.parametrize(
        ("category", "expected_title"),
        [("reference", "octocat/Hello-World"), ("github:issues", "Example issue")],
    )
    async def test_public_search_does_not_require_token(self, public_adapter, category, expected_title):
        captured_headers: dict[str, str] = {}

        def _handler(request):
            captured_headers.update(dict(request.headers))
            return httpx.Response(
                200,
                json={
                    "items": [
                        {
                            "full_name": "octocat/Hello-World",
                            "html_url": "https://github.com/octocat/Hello-World",
                            "description": "Example repository",
                            "stargazers_count": 42,
                            "title": "Example issue",
                            "state": "open",
                        }
                    ]
                },
            )

        async with MockHTTP(_handler):
            result = await public_adapter.search("hello world", {"categories": [category]})

        assert result.status == EngineStatus.OK
        assert result.results[0].title == expected_title
        assert "authorization" not in captured_headers

    async def test_code_search_requires_token(self, public_adapter):
        result = await public_adapter.search("language:python", {"categories": ["github:code"]})

        assert result.status == EngineStatus.UNAVAILABLE
        assert result.error_message == "GitHub token required for code search (set ENGINE_GITHUB_API_KEY)"
        assert "unavailable" in public_adapter.failure_classes

    @pytest.mark.parametrize(
        ("category", "endpoint"),
        [
            ("github:code", "/search/code"),
            ("github:issues", "/search/issues"),
            ("github:prs", "/search/issues"),
            ("reference", "/search/repositories"),
        ],
    )
    async def test_authenticated_search_uses_expected_endpoint(self, authenticated_adapter, category, endpoint):
        captured: dict[str, object] = {}

        def _handler(request):
            captured["url"] = request.url
            captured["authorization"] = request.headers.get("authorization")
            return httpx.Response(200, json={"items": []})

        async with MockHTTP(_handler):
            result = await authenticated_adapter.search("python", {"categories": [category]})

        assert result.status == EngineStatus.OK
        assert str(captured["url"]).startswith(f"https://api.github.com{endpoint}")
        assert captured["authorization"] == "Bearer fixture-token"

    async def test_unauthorized_upstream_response_is_unavailable(self, public_adapter):
        async with MockHTTP(lambda request: httpx.Response(401)):
            result = await public_adapter.search("private repository")

        assert result.status == EngineStatus.UNAVAILABLE
        assert result.error_message == "GitHub authentication required for this request"

    async def test_invalid_search_query_is_error(self, authenticated_adapter):
        async with MockHTTP(lambda request: httpx.Response(422, json={"message": "Validation Failed"})):
            result = await authenticated_adapter.search("invalid query")

        assert result.status == EngineStatus.ERROR
        assert result.error_message == "GitHub rejected the search query (validation failed)"

    async def test_missing_token_is_ready_for_public_health(self, public_adapter):
        assert await public_adapter.health() == EngineStatus.OK
