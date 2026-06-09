"""Result merging, deduplication and ranking (V1: presence-weighted)."""

from slopsearx.adapter import SearchResult


def merge_results(
    engine_results: dict[str, list[SearchResult]],
    strategy: str = "presence",
) -> list[SearchResult]:
    """Merge and deduplicate results from multiple engines.

    V1 uses presence-weighted ranking (honest baseline):
    1. Normalise URLs (strip tracking params)
    2. Deduplicate by normalised URL — keep highest-scored result
    3. Boost results that appear in multiple engines (presence signal)
    4. Sort by final score descending

    Args:
        engine_results: Engine name → list of SearchResult.
        strategy: Ranking strategy identifier.

    Returns:
        Ranked, deduplicated list of SearchResult.
    """
    if not engine_results:
        return []

    seen: dict[str, SearchResult] = {}
    for engine_name, results in engine_results.items():
        for result in results:
            norm_url = _normalise_url(result.url)
            if norm_url in seen:
                existing = seen[norm_url]
                existing.engines.add(engine_name)
                existing.score = max(existing.score, result.score) * len(existing.engines)
            else:
                result.engines = {engine_name}
                result.engine = engine_name
                result.score = 1.0
                seen[norm_url] = result

    ranked = sorted(seen.values(), key=lambda r: r.score, reverse=True)
    for i, r in enumerate(ranked):
        r.position = i + 1
    return ranked


def _normalise_url(url: str) -> str:
    """Strip tracking parameters and normalise for dedup.

    Handles: utm_*, fbclid, gclid, and known redirect domains.
    """
    import urllib.parse

    parsed = urllib.parse.urlparse(url)
    query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)

    strip_params = {"utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "fbclid", "gclid"}
    for param in strip_params:
        query.pop(param, None)

    # Rebuild without tracking params
    new_query = urllib.parse.urlencode(query, doseq=True) if query else ""
    return urllib.parse.urlunparse(parsed._replace(query=new_query))
