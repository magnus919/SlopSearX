"""Output formatters — SearXNG JSON and YAML+Markdown.

The internal SearchResult dataclass is decoupled from the wire format.
Formatters map between the internal model and output serialization.
"""

from __future__ import annotations

import datetime
from typing import Any

import yaml

from slopsearx.adapter import SearchResult

# ---------------------------------------------------------------------------
# JSON Formatter — SearXNG-compatible
# ---------------------------------------------------------------------------


def _result_to_searxng(result: SearchResult) -> dict[str, Any]:
    """Map a SearchResult to SearXNG-compatible MainResult dict.

    All 23 SearXNG fields are present. Null fields are preserved as
    None for JSON serialization.
    """
    return {
        "url": result.url,
        "title": result.title,
        "content": result.content,
        "engine": result.engine,
        "engines": sorted(result.engines) if result.engines else [result.engine],
        "score": result.score,
        "positions": [result.position] if result.position else [],
        "category": result.category,
        "publishedDate": result.published_date,
        "pubdate": _iso_to_epoch(result.published_date),
        "length": None,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "iframe_src": None,
        "audio_src": None,
        "views": None,
        "author": None,
        "metadata": None,
        "template": "default.html",
        "parsed_url": None,
        "open_group": False,
        "close_group": False,
        "priority": "",
    }


def _iso_to_epoch(iso_str: str | None) -> int | None:
    """Convert ISO 8601 datetime to Unix epoch (seconds), or None."""
    if not iso_str:
        return None
    try:
        # Handle ISO formats with or without timezone
        for fmt in ("%Y-%m-%dT%H:%M:%SZ", "%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%d"):
            try:
                dt = datetime.datetime.strptime(iso_str, fmt)
                # Make timezone-aware if naive
                if dt.tzinfo is None:
                    dt = dt.replace(tzinfo=datetime.timezone.utc)
                return int(dt.timestamp())
            except ValueError:
                continue
        return None
    except Exception:
        return None


def format_json(
    results: list[SearchResult],
    query: str,
    *,
    answers: list[dict[str, Any]] | None = None,
    corrections: list[str] | None = None,
    infoboxes: list[dict[str, Any]] | None = None,
    suggestions: list[str] | None = None,
    unresponsive_engines: list[list[str]] | None = None,
    meta: dict[str, Any] | None = None,
    number_of_results: int | None = None,
) -> dict[str, Any]:
    """Format results as a SearXNG-compatible JSON response.

    Args:
        results: Ranked, deduplicated SearchResult list.
        query: Original search query.
        answers: Optional answer box results.
        corrections: Optional spelling corrections.
        infoboxes: Optional info boxes.
        suggestions: Optional query suggestions.
        unresponsive_engines: [[engine_name, reason], ...].
        meta: Meta extension dict (response_time_ms, cached, etc.).
        number_of_results: Override for result count.

    Returns:
        SearXNG-compatible response dict.
    """
    search_results = [_result_to_searxng(r) for r in results]

    response: dict[str, Any] = {
        "query": query,
        "results": search_results,
        "number_of_results": (
            number_of_results if number_of_results is not None else len(search_results)
        ),
        "answers": answers or [],
        "corrections": corrections or [],
        "infoboxes": infoboxes or [],
        "suggestions": suggestions or [],
        "unresponsive_engines": unresponsive_engines or [],
    }

    # meta is injected at the top level (not nested) to extend
    # the SearXNG response without breaking existing consumers.
    if meta:
        response["meta"] = meta

    return response


# ---------------------------------------------------------------------------
# YAML+Markdown Formatter — agent-native
# ---------------------------------------------------------------------------


def format_yaml_markdown(
    results: list[SearchResult],
    query: str,
    *,
    meta: dict[str, Any] | None = None,
    engine_count: int | None = None,
    responsive_count: int | None = None,
    unresponsive_engines: list[list[str]] | None = None,
) -> str:
    """Format results as YAML+Markdown (agent-native output).

    The response is a YAML document with an embedded Markdown section,
    separated by ``---``. Designed for AI agent consumption where
    structured data + readable prose is more useful than raw JSON.

    Args:
        results: Ranked SearchResult list.
        query: Original search query.
        meta: Meta extension dict.
        engine_count: Total active engines.
        responsive_count: How many engines returned results.
        unresponsive_engines: [[engine_name, reason], ...].

    Returns:
        YAML document string with embedded Markdown.
    """
    # Build the YAML header
    yaml_section: dict[str, Any] = {
        "query": query,
    }

    if meta:
        yaml_section["meta"] = {
            "response_time_ms": meta.get("response_time_ms", 0),
            "cached": meta.get("cached", False),
            "query_id": meta.get("query_id", ""),
        }
        # Count responsive engines from engine_status
        if "engine_status" in meta:
            yaml_section["meta"]["engine_count"] = len(meta["engine_status"])
            ok_count = sum(
                1 for s in meta["engine_status"].values() if s.get("status") == "ok"
            )
            yaml_section["meta"]["responsive"] = ok_count

    yaml_section["results"] = [
        {
            "url": r.url,
            "title": r.title,
            "engine": r.engine,
            "engines": sorted(r.engines) if r.engines else [r.engine],
            "content": r.content if r.content else None,
            "score": r.score,
            "position": r.position,
            "published": r.published_date,
        }
        for r in results
    ]

    # Build the Markdown section
    total_engines = engine_count or (len(meta["engine_status"]) if meta and "engine_status" in meta else 0)
    ok_engines = responsive_count
    if ok_engines is None and meta and "engine_status" in meta:
        ok_engines = sum(
            1 for s in meta["engine_status"].values() if s.get("status") == "ok"
        )

    elapsed = meta.get("response_time_ms", 0) if meta else 0

    lines: list[str] = []
    lines.append("## Results Summary")
    lines.append("")
    lines.append(
        f"**{ok_engines or '?'} engines responded** "
        f"of {total_engines or '?'} active. "
        f"{len(results)} results returned in {elapsed / 1000:.1f}s."
    )
    lines.append("")
    lines.append(f"For **{query}**, the top results cover:")

    for r in results[:5]:
        snippet = (r.content or "")[:120].strip().replace("\n", " ")
        lines.append(f"- {snippet or r.title}")

    lines.append("")

    if unresponsive_engines:
        blocked_list = ", ".join(f"{e[0]} ({e[1]})" for e in unresponsive_engines)
        lines.append(f"> {blocked_list}.")
        lines.append("")

    markdown_body = "\n".join(lines)

    # Combine: YAML header + separator + Markdown body
    yaml_str = yaml.dump(yaml_section, allow_unicode=True, default_flow_style=False, sort_keys=False)
    return f"{yaml_str}---\n{markdown_body}"


# ---------------------------------------------------------------------------
# Backward-compat stub wrapper
# ---------------------------------------------------------------------------


def format_yaml(results: list[dict[str, Any]], query: str, **meta: Any) -> str:
    """Format results as YAML+Markdown (legacy stub wrapper).

    Prefer ``format_yaml_markdown`` for production use — this exists
    for backward compatibility with the M1 stub signature.
    """
    # Convert dict results to SearchResult for the real formatter
    search_results = [
        SearchResult(
            url=r.get("url", ""),
            title=r.get("title", ""),
            content=r.get("content", ""),
            engine=r.get("engine", ""),
        )
        for r in results
    ]
    return format_yaml_markdown(
        search_results,
        query,
        meta={"response_time_ms": meta.get("response_time_ms", 0)},
        engine_count=meta.get("engine_count", 0),
        responsive_count=meta.get("responsive_count", 0),
    )
