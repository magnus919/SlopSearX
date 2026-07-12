"""Result merging, deduplication and ranking.

V1: Presence-weighted ranking (honest baseline).
V2 (future): Weighted-fusion with per-engine trust scores.
"""

from __future__ import annotations

from typing import Any, Optional

from slopsearx.adapter import AdapterResponse, EngineStatus, SearchResult

# ---------------------------------------------------------------------------
# Presence-weighted ranking
# ---------------------------------------------------------------------------


class PresenceRanker:
    """V1: Presence-weighted ranking.

    Strategy:
    1. Normalise URLs (strip tracking params).
    2. Deduplicate by normalised URL — keep highest-scored result.
    3. Boost results that appear in multiple engines (presence signal).
    4. Sort by final score descending, then assign positions.

    Documented quality ceiling: V1 ranking is not better than any
    individual engine's ranking. It provides breadth (coverage gain)
    at the cost of precision (less accurate ordering). The ranking is
    presence-weighted — a result appearing in N engine feeds is
    preferred over one appearing in 1, regardless of which engine.
    """

    def __init__(self, per_engine_budget: Optional[dict[str, int]] = None) -> None:
        self.per_engine_budget = per_engine_budget or {}

    def rank(
        self,
        engine_results: dict[str, list[SearchResult]],
        query: str,
        params: dict[str, Any] | None = None,
    ) -> list[SearchResult]:
        if not engine_results:
            return []

        seen: dict[str, SearchResult] = {}

        for engine_name, results in engine_results.items():
            budget = self.per_engine_budget.get(engine_name, 0)
            taken = 0

            for result in results:
                # Per-engine result budget enforcement
                budget_ok = True
                if budget > 0:
                    if taken >= budget:
                        budget_ok = False
                    else:
                        taken += 1

                norm_url = _normalise_url(result.url)

                if norm_url in seen:
                    existing = seen[norm_url]
                    existing.engines.add(engine_name)
                    # Presence-weighted: boost score by engine count
                    existing.score = 1.0 * len(existing.engines)
                    # Preserve the higher-priority tier (lower number)
                    existing.tier = min(existing.tier, result.tier)
                elif budget_ok:
                    result.engines = {engine_name}
                    result.engine = engine_name
                    result.score = 1.0
                    seen[norm_url] = result
                # else: skipped due to per-engine budget

        # Sort by tier first (1 before 2), then by score descending
        ranked = sorted(seen.values(), key=lambda r: (r.tier, -r.score))

        for i, r in enumerate(ranked):
            r.position = i + 1

        return ranked


# ---------------------------------------------------------------------------
# Convenience function (backward compat with existing stub)
# ---------------------------------------------------------------------------


def merge_results(
    engine_results: dict[str, list[SearchResult]],
    strategy: str = "presence",
) -> list[SearchResult]:
    """Merge and deduplicate results from multiple engines.

    Convenience wrapper around presence-weighted ranking. This function
    exists for backward compatibility with the M1 stub.

    Args:
        engine_results: Engine name → list of SearchResult.
        strategy: Deprecated ranking strategy identifier. Presence ranking
            is the only supported strategy.

    Returns:
        Ranked, deduplicated list of SearchResult.
    """
    del strategy
    return PresenceRanker().rank(engine_results, "")


# ---------------------------------------------------------------------------
# Metadata helpers
# ---------------------------------------------------------------------------


def build_engine_status(
    responses: dict[str, AdapterResponse],
    elapsed_ms: float,
) -> dict[str, dict[str, Any]]:
    """Build per-engine status map from adapter responses.

    Args:
        responses: Engine name → AdapterResponse.
        elapsed_ms: Total wall-clock time for the fan-out.

    Returns:
        Dict mapping engine name → {results, latency_ms, status}.
    """
    status: dict[str, dict[str, Any]] = {}
    for name, resp in responses.items():
        status[name] = {
            "results": len(resp.results),
            "latency_ms": round(resp.latency_ms, 1),
            "status": resp.status.value,
        }
    return status


def extract_unresponsive(
    responses: dict[str, AdapterResponse],
) -> list[list[str]]:
    """Extract unresponsive engine list for SearXNG-compatible response.

    Returns list of [engine_name, reason] pairs.
    """
    unresponsive: list[list[str]] = []
    for name, resp in responses.items():
        if resp.status != EngineStatus.OK:
            reason = resp.error_message or resp.status.value
            unresponsive.append([name, reason])
    return unresponsive


def extract_empty_scrape_engines(
    responses: dict[str, AdapterResponse],
    scrape_engine_names: set[str],
) -> list[list[str]]:
    """Report successful scrape responses that contained no parsed results.

    This is diagnostic-only: an empty result set can be legitimate, so it does
    not alter the engine status or circuit-breaker behavior.
    """
    return [
        [name, "successful scrape returned no results"]
        for name, response in responses.items()
        if name in scrape_engine_names and response.status == EngineStatus.OK and not response.results
    ]


def build_meta(
    responses: dict[str, AdapterResponse],
    elapsed_ms: float,
    query_id: str,
    cached: bool = False,
    empty_engines: list[list[str]] | None = None,
) -> dict[str, Any]:
    """Build the meta.* extension field.

    Args:
        responses: Engine name → AdapterResponse.
        elapsed_ms: Total wall-clock time.
        query_id: Traceable query identifier.
        cached: Whether the response was served from cache.

    Returns:
        Meta dict with response_time_ms, cached, query_id, engine_status.
    """
    meta = {
        "response_time_ms": round(elapsed_ms),
        "cached": cached,
        "query_id": query_id,
        "engine_status": build_engine_status(responses, elapsed_ms),
    }
    if empty_engines:
        meta["empty_engines"] = empty_engines
    return meta


# ---------------------------------------------------------------------------
# URL normalisation
# ---------------------------------------------------------------------------


def _normalise_url(url: str) -> str:
    """Strip tracking parameters and normalise for dedup.

    Handles: utm_*, fbclid, gclid.
    """
    import urllib.parse

    parsed = urllib.parse.urlparse(url)
    query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)

    strip_params = {
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
    }
    for param in strip_params:
        query.pop(param, None)

    new_query = urllib.parse.urlencode(query, doseq=True) if query else ""
    return urllib.parse.urlunparse(parsed._replace(query=new_query))
