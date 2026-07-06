"""Tests for jobs ATS adapters and routing topic."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import EngineStatus, discover_engines
from slopsearx.jobs_utils import extract_company
from slopsearx.router import QueryRouter

# ---------------------------------------------------------------------------
# Helper: mock HTTP transport (replicated from test_adapters.py)
# ---------------------------------------------------------------------------


class MockHTTP:
    """Context manager that patches httpx.AsyncClient to return a mock client."""

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


# ---------------------------------------------------------------------------
# Company extraction
# ---------------------------------------------------------------------------


class TestCompanyExtraction:
    def test_extract_at_trigger(self):
        slug, name = extract_company("Senior AI Engineer at Anthropic")
        assert slug == "anthropic"
        assert name == "Anthropic"

    def test_extract_for_trigger(self):
        slug, name = extract_company("looking for engineer for Stripe")
        assert slug == "stripe"
        assert name == "Stripe"

    def test_extract_at_symbol_trigger(self):
        slug, name = extract_company("PM role @ Notion")
        assert slug == "notion"
        assert name == "Notion"

    def test_extract_multi_word_company(self):
        slug, name = extract_company("Engineer at Red Hat")
        assert slug == "red-hat"
        assert name == "Red Hat"

    def test_extract_no_trigger_returns_none(self):
        slug, name = extract_company("python developer jobs")
        assert slug is None
        assert name is None

    def test_extract_at_end_no_company(self):
        slug, name = extract_company("hiring at")
        assert slug is None
        assert name is None

    def test_extract_special_characters(self):
        slug, name = extract_company("dev at Hello! World Inc.")
        assert slug == "hello-world-inc"
        assert name == "Hello! World Inc."

    def test_extract_trigger_in_url(self):
        slug, name = extract_company("site:jobs.lever.co at Stripe")
        assert slug == "stripe"
        assert name == "Stripe"

    def test_extract_case_insensitive(self):
        slug, name = extract_company("HIRING AT MICROSOFT")
        assert slug == "microsoft"
        assert name == "MICROSOFT"


# ---------------------------------------------------------------------------
# Greenhouse adapter
# ---------------------------------------------------------------------------


class TestGreenhouseAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"greenhouse": {"enabled": True}})
        return instances["greenhouse"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "jobs": [
                {
                    "id": 123,
                    "title": "Senior AI Engineer",
                    "absolute_url": "https://boards.greenhouse.io/anthropic/jobs/123",
                    "offices": [{"name": "San Francisco, CA"}],
                    "metadata": [{"name": "Salary", "value": "$200k-$280k"}],
                    "updated_at": "2026-07-01T12:00:00Z",
                },
                {
                    "id": 456,
                    "title": "ML Research Scientist",
                    "absolute_url": "https://boards.greenhouse.io/anthropic/jobs/456",
                    "offices": [{"name": "New York, NY"}],
                    "metadata": [],
                    "updated_at": "2026-07-02T12:00:00Z",
                },
            ],
            "meta": {"total_count": 2},
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("Senior AI Engineer at Anthropic")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Senior AI Engineer"
        assert "anthropic" in result.results[0].url.lower()
        assert "San Francisco, CA" in result.results[0].content
        assert "$200k-$280k" in result.results[0].content
        assert result.results[0].published_date == "2026-07-01T12:00:00Z"
        assert result.results[0].category == "jobs"
        assert result.results[0].tier == 2

    async def test_search_no_company_returns_empty(self, adapter):
        result = await adapter.search("python jobs")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_404_returns_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("Engineer at NonexistentCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_empty_jobs(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"jobs": []})):
            result = await adapter.search("Engineer at SomeCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("Engineer at RateLimitedCo")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_http_error(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(500)):
            result = await adapter.search("Engineer at ErrorCo")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("Engineer at TimeoutCo")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_url_contains_company_slug(self, adapter, sample_response):
        captured_url: list[str] = []

        def _handler(r):
            captured_url.append(str(r.url))
            return httpx.Response(200, json=sample_response)

        async with MockHTTP(_handler):
            await adapter.search("Engineer at Anthropic")
        assert "/anthropic/jobs" in captured_url[0]


# ---------------------------------------------------------------------------
# Ashby adapter
# ---------------------------------------------------------------------------


class TestAshbyAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"ashby": {"enabled": True}})
        return instances["ashby"]

    @pytest.fixture
    def sample_response(self) -> dict:
        return {
            "data": {
                "jobBoard": {
                    "jobPostings": [
                        {
                            "id": "abc-123",
                            "title": "AI Engineer",
                            "locationName": "Remote",
                            "salaryRange": "$180k-$250k",
                            "departmentName": "Engineering",
                            "updatedAt": "2026-07-01T12:00:00.000Z",
                        },
                        {
                            "id": "def-456",
                            "title": "Product Designer",
                            "locationName": "New York, NY",
                            "salaryRange": "",
                            "departmentName": "Design",
                            "updatedAt": "2026-06-30T12:00:00.000Z",
                        },
                    ],
                },
            },
        }

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("AI Engineer at Temporal")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "AI Engineer"
        assert "jobs.ashbyhq.com/temporal/abc-123" in result.results[0].url
        assert "Remote" in result.results[0].content
        assert "$180k-$250k" in result.results[0].content
        assert result.results[0].category == "jobs"
        assert result.results[0].tier == 2

    async def test_search_no_company_returns_empty(self, adapter):
        result = await adapter.search("python jobs")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_404_returns_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("Engineer at NonexistentCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_empty_postings(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json={"data": {"jobBoard": {"jobPostings": []}}})):
            result = await adapter.search("Engineer at SomeCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("Engineer at RateLimitedCo")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_http_error(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(500)):
            result = await adapter.search("Engineer at ErrorCo")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("Engineer at TimeoutCo")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_uses_graphql_post(self, adapter):
        captured: dict = {}

        def _handler(r):
            captured["method"] = r.method
            captured["url"] = str(r.url)
            captured["body"] = r.content
            return httpx.Response(200, json={"data": {"jobBoard": {"jobPostings": []}}})

        async with MockHTTP(_handler):
            await adapter.search("Engineer at Temporal")
        assert captured["method"] == "POST"
        assert "graphql" in captured["url"].lower()
        assert b"ApiJobBoardWithTeams" in captured["body"]

    async def test_search_null_safe_fields(self, adapter):
        """Null fields in GraphQL response should not cause errors."""
        response = {
            "data": {
                "jobBoard": {
                    "jobPostings": [
                        {
                            "id": None,
                            "title": None,
                            "locationName": None,
                            "salaryRange": None,
                            "departmentName": None,
                            "updatedAt": None,
                        },
                    ],
                },
            },
        }
        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("Engineer at TestCo")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 1


# ---------------------------------------------------------------------------
# Lever adapter
# ---------------------------------------------------------------------------


class TestLeverAdapter:
    @pytest.fixture
    def adapter(self):
        instances = discover_engines({"lever": {"enabled": True}})
        return instances["lever"]

    @pytest.fixture
    def sample_response(self) -> list[dict]:
        return [
            {
                "id": "abc123",
                "text": "Software Engineer",
                "categories": {
                    "location": "San Francisco, CA",
                    "commitment": "Full-time",
                    "team": "Engineering",
                },
                "hostedUrl": "https://jobs.lever.co/stripe/abc123",
                "createdAt": 1720000000000,
            },
            {
                "id": "def456",
                "text": "Product Manager",
                "categories": {
                    "location": "Remote",
                    "commitment": "Full-time",
                    "team": "Product",
                },
                "hostedUrl": "https://jobs.lever.co/stripe/def456",
                "createdAt": 1719900000000,
            },
        ]

    async def test_search_returns_results(self, adapter, sample_response):
        async with MockHTTP(lambda r: httpx.Response(200, json=sample_response)):
            result = await adapter.search("Engineer at Stripe")
        assert result.status == EngineStatus.OK
        assert len(result.results) == 2
        assert result.results[0].title == "Software Engineer"
        assert "jobs.lever.co/stripe/abc123" in result.results[0].url
        assert "San Francisco, CA" in result.results[0].content
        assert "Full-time" in result.results[0].content
        assert result.results[0].published_date is not None
        assert result.results[0].category == "jobs"
        assert result.results[0].tier == 2

    async def test_search_no_company_returns_empty(self, adapter):
        result = await adapter.search("python jobs")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_404_returns_empty(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(404)):
            result = await adapter.search("Engineer at NonexistentCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_empty_array(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(200, json=[])):
            result = await adapter.search("Engineer at SomeCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_rate_limited(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(429)):
            result = await adapter.search("Engineer at RateLimitedCo")
        assert result.status == EngineStatus.RATE_LIMITED
        assert result.results == []

    async def test_search_http_error(self, adapter):
        async with MockHTTP(lambda r: httpx.Response(500)):
            result = await adapter.search("Engineer at ErrorCo")
        assert result.status == EngineStatus.ERROR
        assert result.results == []

    async def test_search_timeout(self, adapter):
        async with MockHTTP(lambda r: (_ for _ in ()).throw(httpx.TimeoutException("timeout"))):
            with patch("httpx.AsyncClient", side_effect=httpx.TimeoutException("timeout")):
                result = await adapter.search("Engineer at TimeoutCo")
        assert result.status == EngineStatus.TIMEOUT
        assert result.results == []

    async def test_search_non_list_response(self, adapter):
        """Non-list JSON response should return empty results."""
        async with MockHTTP(lambda r: httpx.Response(200, json={"error": "not found"})):
            result = await adapter.search("Engineer at BadCo")
        assert result.status == EngineStatus.OK
        assert result.results == []

    async def test_search_missing_published_date(self, adapter):
        """Missing createdAt should default published_date to None."""
        response = [
            {
                "id": "xyz",
                "text": "DevOps Engineer",
                "categories": {"location": "Remote"},
                "hostedUrl": "https://jobs.lever.co/testco/xyz",
            },
        ]
        async with MockHTTP(lambda r: httpx.Response(200, json=response)):
            result = await adapter.search("Engineer at TestCo")
        assert result.status == EngineStatus.OK
        assert result.results[0].published_date is None

    async def test_search_url_contains_company_slug(self, adapter, sample_response):
        captured_url: list[str] = []

        def _handler(r):
            captured_url.append(str(r.url))
            return httpx.Response(200, json=sample_response)

        async with MockHTTP(_handler):
            await adapter.search("Engineer at Stripe")
        assert "/stripe" in captured_url[0]
        assert "mode=json" in captured_url[0]


# ---------------------------------------------------------------------------
# Router topic tests
# ---------------------------------------------------------------------------


class TestJobsRouting:
    @pytest.fixture
    def router(self):
        return QueryRouter()

    def test_senior_at_company_matches(self, router):
        result = router.route("Senior AI Engineer at Anthropic")
        assert result is not None
        assert "greenhouse" in result
        assert "ashby" in result
        assert "lever" in result

    def test_hiring_keyword_matches(self, router):
        result = router.route("hiring software engineers")
        assert result is not None
        assert "greenhouse" in result

    def test_careers_keyword_matches(self, router):
        result = router.route("careers at google")
        assert result is not None

    def test_openings_keyword_matches(self, router):
        result = router.route("job openings remote")
        assert result is not None

    def test_full_time_keyword_matches(self, router):
        result = router.route("full-time engineering positions")
        assert result is not None

    def test_workday_keyword_matches(self, router):
        result = router.route("workday integration developer")
        assert result is not None

    def test_non_job_query_no_jobs_engines(self, router):
        """Non-job query may match another topic but must not include jobs engines."""
        result = router.route("latest agentic frameworks")
        if result is not None:
            assert "greenhouse" not in result
            assert "ashby" not in result
            assert "lever" not in result

    def test_categories_skips_routing(self, router):
        result = router.route("Senior AI Engineer at Anthropic", categories=["jobs"])
        assert result is None

    def test_case_insensitive_matching(self, router):
        result = router.route("HIRING ENGINEERS")
        assert result is not None

    def test_code_topic_matches_first(self, router):
        """Job-adjacent queries like 'principal engineer api architecture' should match code first."""
        result = router.route("principal engineer api architecture")
        assert result is not None
        assert "github" in result  # code topic engines

    @pytest.mark.parametrize(
        "query",
        [
            "latest agentic frameworks",
            "python async tutorial",
            "quantum computing breakthrough",
            "breaking news today",
            "reddit discussion thread",
            "wikipedia reference guide",
            "wayback machine archive",
            "how to write rust macros",
            "deep learning paper 2026",
            "show hn my new project",
            "docker compose tutorial",
            "neural network architecture",
            "github actions workflow",
            "npm package publishing",
            "cargo build optimization",
            "react hooks best practices",
            "kubernetes deployment guide",
            "api design patterns",
            "javascript event loop",
            "sql query optimization",
        ],
    )
    def test_non_job_corpus_no_false_positives(self, router, query):
        """AC-001.4: 20 non-job queries should never match the jobs topic."""
        result = router.route(query)
        if result is not None:
            assert "greenhouse" not in result, f"'{query}' falsely matched jobs topic"
            assert "ashby" not in result, f"'{query}' falsely matched jobs topic"
            assert "lever" not in result, f"'{query}' falsely matched jobs topic"


# ---------------------------------------------------------------------------
# Engine registration
# ---------------------------------------------------------------------------


class TestJobsEngineRegistration:
    def test_greenhouse_registered(self):
        from slopsearx.adapter import _ENGINE_REGISTRY

        assert "greenhouse" in _ENGINE_REGISTRY

    def test_ashby_registered(self):
        from slopsearx.adapter import _ENGINE_REGISTRY

        assert "ashby" in _ENGINE_REGISTRY

    def test_lever_registered(self):
        from slopsearx.adapter import _ENGINE_REGISTRY

        assert "lever" in _ENGINE_REGISTRY

    def test_all_jobs_engines_have_jobs_category(self):
        from slopsearx.adapter import _ENGINE_REGISTRY

        for name in ("greenhouse", "ashby", "lever"):
            cls = _ENGINE_REGISTRY[name]
            assert "jobs" in cls.categories, f"{name} should have 'jobs' category"
            assert "general" not in cls.categories, f"{name} should NOT have 'general' category"
