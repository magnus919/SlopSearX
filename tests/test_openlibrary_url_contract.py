"""Regression tests for Open Library result identity URLs."""

from __future__ import annotations

from unittest.mock import patch

import httpx

from engines.openlibrary import OpenLibraryAdapter
from slopsearx.formatter import format_json
from slopsearx.merger import PresenceRanker


class MockHTTP:
    """Patch the adapter's HTTP client with a deterministic JSON response."""

    def __init__(self, payload: dict):
        self.transport = httpx.MockTransport(lambda request: httpx.Response(200, json=payload))

    async def __aenter__(self):
        self.mock_client = httpx.AsyncClient(transport=self.transport)
        self.patcher = patch("httpx.AsyncClient")
        mock_class = self.patcher.start()
        mock_class.return_value.__aenter__.return_value = self.mock_client
        return self

    async def __aexit__(self, *args):
        self.patcher.stop()
        await self.mock_client.aclose()


async def search_docs(docs: list[dict]) -> list:
    adapter = OpenLibraryAdapter({"max_results": 10})
    async with MockHTTP({"docs": docs}):
        response = await adapter.search("Dune Frank Herbert")
    assert response.status.value == "ok"
    return response.results


async def test_identifier_urls_prefer_work_then_edition_then_isbn_and_omit_unknown() -> None:
    results = await search_docs(
        [
            {
                "title": "Dune",
                "key": "/works/OL166894W",
                "edition_key": ["OL24370548M"],
                "isbn": ["9780441172719"],
            },
            {"title": "Dune Messiah", "edition_key": ["OL7353617M"]},
            {"title": "Unknown identity", "author_name": ["Unknown author"]},
            {"title": "God Emperor of Dune", "isbn": ["9780441103250"]},
        ],
    )

    assert [result.url for result in results] == [
        "https://openlibrary.org/works/OL166894W",
        "https://openlibrary.org/books/OL7353617M",
        "https://openlibrary.org/isbn/9780441103250",
    ]


async def test_identifier_free_records_are_omitted() -> None:
    results = await search_docs(
        [
            {
                "title": "The Left Hand of Darkness",
                "author_name": ["Ursula K. Le Guin"],
                "first_publish_year": 1969,
            },
            {
                "title": "The Dispossessed",
                "author_name": ["Ursula K. Le Guin"],
                "first_publish_year": 1974,
            },
        ],
    )

    assert results == []


async def test_distinct_urls_survive_merger_and_json_formatting() -> None:
    results = await search_docs(
        [
            {"title": "Dune", "key": "/works/OL166894W"},
            {"title": "Dune Messiah", "key": "/works/OL321123W"},
            {"title": "Unidentified book", "author_name": ["Unknown author"]},
        ],
    )

    merged = PresenceRanker().rank({"openlibrary": results}, "Dune Frank Herbert")
    document = format_json(merged, "Dune Frank Herbert")

    assert len(merged) == 2
    assert [result["url"] for result in document["results"]] == [
        "https://openlibrary.org/works/OL166894W",
        "https://openlibrary.org/works/OL321123W",
    ]
    assert [result["parsed_url"] for result in document["results"]] == [
        ["https", "openlibrary.org", "/works/OL166894W", "", "", ""],
        ["https", "openlibrary.org", "/works/OL321123W", "", "", ""],
    ]
