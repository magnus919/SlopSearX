"""Bounded arXiv request/order baseline for triage issue #436.

The arXiv API User's Manual documents ``all:`` as an all-field search,
``ti:`` and ``abs:`` as fielded alternatives, and ``sortBy=relevance`` as
the API's upstream relevance ordering:
https://info.arxiv.org/help/api/user-manual.html

This fixture deliberately does not claim that one query form is more relevant
than another.  A deterministic Atom response can prove that the adapter sends
the intended encoded request and preserves the provider's order, but it cannot
measure the provider's topical relevance.  A live, labelled comparison of
``all:``, quoted, ``ti:``, and ``abs:`` variants remains unresolved and is the
follow-up benchmark required before changing query construction.
"""

from __future__ import annotations

from html import escape
from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import discover_engines


class MockHTTP:
    """Context manager that routes the adapter's HTTP call to a fixture."""

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


QUERY_CASES = (
    (
        "graph neural networks",
        (
            "Graph Neural Networks: Foundations and Applications",
            "A General Analysis of Randomized Numerical Algorithms",
        ),
    ),
    (
        "quantum error correction",
        (
            "Quantum Error Correction with Surface Codes",
            "Learning Representations for Scientific Data",
        ),
    ),
    (
        "climate change",
        (
            "Climate Change and Extreme Weather Events",
            "Efficient Optimization for Large-Scale Machine Learning",
        ),
    ),
)


def _atom_feed(titles: tuple[str, ...]) -> str:
    entries = []
    for index, title in enumerate(titles, start=1):
        entries.append(
            f"""
  <entry>
    <id>http://arxiv.org/abs/2501.{index:05d}</id>
    <title>{escape(title)}</title>
    <summary>Deterministic fixture abstract for {escape(title)}.</summary>
    <published>2025-01-{index:02d}T00:00:00Z</published>
  </entry>""",
        )
    return '<feed xmlns="http://www.w3.org/2005/Atom">' + "".join(entries) + "\n</feed>"


@pytest.fixture
def adapter():
    return discover_engines({"arxiv": {"enabled": True}})["arxiv"]


@pytest.mark.parametrize("query,titles", QUERY_CASES)
async def test_request_encoding_and_feed_order_are_preserved(adapter, query, titles):
    """The adapter is transparent to upstream relevance ordering."""

    requests = []

    def handler(request):
        requests.append(request)
        return httpx.Response(200, content=_atom_feed(titles).encode(), request=request)

    async with MockHTTP(handler):
        response = await adapter.search(query)

    assert response.status.value == "ok"
    assert len(requests) == 1
    request = requests[0]
    assert request.url.params["search_query"] == f"all:{query}"
    assert "search_query=" in str(request.url)
    assert "all%3A" in str(request.url)
    assert request.url.params["start"] == "0"
    assert request.url.params["max_results"] == "5"
    assert request.url.params["sortBy"] == "relevance"
    assert request.url.params["sortOrder"] == "descending"
    assert [result.title for result in response.results] == list(titles)
    assert [result.position for result in response.results] == [1, 2]


def test_relevance_benchmark_is_explicitly_unresolved():
    """Fixtures establish transport behavior, not provider topical quality."""

    assert len(QUERY_CASES) >= 3
    assert all(len(titles) >= 2 for _, titles in QUERY_CASES)
    assert "live, labelled comparison" in __doc__
