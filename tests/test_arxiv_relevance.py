"""Deterministic request contract for arXiv phrase relevance."""

from __future__ import annotations

from unittest.mock import patch

import httpx
import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx.adapter import discover_engines


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


ATOM = """<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/2501.00001</id>
    <title>Graph Neural Networks</title>
    <summary>Deterministic fixture.</summary>
    <published>2025-01-01T00:00:00Z</published>
  </entry>
</feed>"""


@pytest.fixture
def adapter():
    return discover_engines({"arxiv": {"enabled": True}})["arxiv"]


@pytest.mark.parametrize(
    ("query", "expected"),
    (
        ("graph neural networks", 'all:"graph neural networks"'),
        ("graph", "all:graph"),
        ('ti:"graph neural networks"', 'all:ti:"graph neural networks"'),
    ),
)
async def test_plain_multiword_queries_use_phrase_search(adapter, query, expected):
    requests = []

    def handler(request):
        requests.append(request)
        return httpx.Response(200, text=ATOM, request=request)

    async with MockHTTP(handler):
        response = await adapter.search(query)

    assert response.status.value == "ok"
    assert len(requests) == 1
    assert requests[0].url.params["search_query"] == expected
    assert requests[0].url.params["max_results"] == "5"
    assert requests[0].url.params["sortBy"] == "relevance"
    assert [result.title for result in response.results] == ["Graph Neural Networks"]
