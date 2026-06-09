"""Pytest configuration for SlopSearX."""

from __future__ import annotations

import pytest


@pytest.fixture
def sample_query() -> str:
    return "test search query"


@pytest.fixture
def sample_params() -> dict:
    return {"language": "en", "safesearch": 1, "pageno": 1, "categories": ["general"], "time_range": None}
