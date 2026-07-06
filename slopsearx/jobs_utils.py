"""Jobs utility — shared company-name extraction for ATS adapters.

Each ATS adapter (Greenhouse, Ashby, Lever) requires a company
identifier to construct a valid API request. This module provides
a greedy multi-token extraction heuristic that all three adapters use.
"""

from __future__ import annotations

import re
from typing import Optional

_TRIGGER_WORDS = [" at ", " for ", " @ "]
_NON_ALPHANUM = re.compile(r"[^a-z0-9-]")


def extract_company(query: str) -> tuple[Optional[str], Optional[str]]:
    """Extract a company name from a job-shaped query string.

    Uses a greedy multi-token heuristic: finds the first trigger word
    (`` at ``, `` for ``, `` @ ``) and tries the full remaining text
    as the company name. If no trigger word is found, returns ``(None, None)``.

    Args:
        query: The raw search query, e.g. ``"Senior AI Engineer at Anthropic"``.

    Returns:
        A ``(slug, display_name)`` tuple:
        - ``slug``: Lowercased, slugified company name for URL construction.
        - ``display_name``: Original-case company name for display in results.
        - ``(None, None)`` if no company can be extracted.
    """
    query_lower = query.lower()

    # Find the last occurrence of any trigger word to handle queries
    # where trigger words appear multiple times (e.g. "looking for engineer for Stripe").
    best_idx = -1
    best_trigger_len = 0
    for trigger in _TRIGGER_WORDS:
        idx = query_lower.rfind(trigger)
        if idx > best_idx:
            best_idx = idx
            best_trigger_len = len(trigger)

    if best_idx != -1:
        candidate = query[best_idx + best_trigger_len :].strip()
        if candidate:
            slug = _to_slug(candidate)
            return (slug, candidate)
        return (None, None)

    return (None, None)


def _to_slug(name: str) -> str:
    """Convert a company name to a URL-safe slug.

    Lowercases, replaces non-alphanumeric characters with hyphens,
    and collapses multiple hyphens.
    """
    slug = name.lower().strip()
    slug = _NON_ALPHANUM.sub("-", slug)
    slug = re.sub(r"-+", "-", slug)
    return slug.strip("-")
