"""Output formatters — SearXNG JSON and YAML+Markdown.

The internal SearchResult dataclass is decoupled from the wire format.
Formatters map between the internal model and output serialization.
"""

from __future__ import annotations

import csv
import datetime
import html as html_lib
import io
from typing import Any
from xml.etree import ElementTree

import yaml

from slopsearx.adapter import SearchResult, media_to_dict
from slopsearx.payload import (
    PAYLOAD_INLINE_BYTES,
    payload_max_persist_bytes,
    payload_serialized_size,
    payload_to_dict,
)

# ---------------------------------------------------------------------------
# JSON Formatter — SearXNG-compatible
# ---------------------------------------------------------------------------


def _payload_for_output(payload: dict[str, Any] | None) -> dict[str, Any] | None:
    """Inline a payload only when it is small enough for compact output.

    The public, unauthenticated HTTP ``/search`` surface mirrors the MCP
    compact-card disclosure model: payloads are omitted unless their
    serialized form is ``<= PAYLOAD_INLINE_BYTES`` **and** within the
    configured persistence bound (``payload_max_persist_bytes()``). Larger
    payloads are not emitted here (there is no HTTP equivalent of
    ``slopsearx_read_result``), and an unserializable payload is always
    omitted. The gate measures and emits the **same canonical form** the
    persistence boundary stores (``payload_to_dict``), so the HTTP output
    never disagrees with the cached/record form of the payload (finding 4) —
    in particular, a payload above the persistence bound is omitted on both
    the fresh and the cache-hit response (finding 3).
    """
    if payload is None:
        return None
    canonical = payload_to_dict(payload)
    if canonical is None:
        return None
    size = payload_serialized_size(canonical)
    if size is None or size > PAYLOAD_INLINE_BYTES or size > payload_max_persist_bytes():
        return None
    return canonical


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
        "tier": result.tier,
        "length": None,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "media": media_to_dict(result.media),
        "payload": _payload_for_output(result.payload),
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

    # Build response sequentially so engines sits between results and
    # number_of_results in the output dict (SearXNG-compatible ordering).
    response: dict[str, Any] = {}
    response["query"] = query
    response["results"] = search_results

    # engines array — SearXNG-compatible top-level field
    if meta and "engine_status" in meta:
        response["engines"] = [
            {"engine": name, "results": info.get("results", 0)} for name, info in meta["engine_status"].items()
        ]

    response["number_of_results"] = number_of_results if number_of_results is not None else len(search_results)
    response["answers"] = answers or []
    response["corrections"] = corrections or []
    response["infoboxes"] = infoboxes or []
    response["suggestions"] = suggestions or []
    response["unresponsive_engines"] = unresponsive_engines or []

    # meta is injected at the top level (not nested) to extend
    # the SearXNG response without breaking existing consumers.
    if meta:
        response["meta"] = meta

    return response


def format_html(
    results: list[SearchResult],
    query: str,
    *,
    meta: dict[str, Any] | None = None,
    unresponsive_engines: list[list[str]] | None = None,
) -> str:
    """Format a small, safe HTML search-results page.

    This is intentionally theme-neutral: the compatibility contract requires
    HTML output, while the JSON and YAML formatters remain the richer machine
    interfaces for clients that do not need a browser page.
    """
    escaped_query = html_lib.escape(query, quote=True)
    result_items: list[str] = []
    for result in results:
        title = html_lib.escape(result.title or result.url, quote=True)
        url = html_lib.escape(result.url, quote=True)
        content = html_lib.escape(result.content or "")
        result_items.append(
            '<article class="result">'
            f'<h2><a href="{url}">{title}</a></h2>'
            f'<p class="url">{url}</p>'
            f"<p>{content}</p>"
            f'<p class="engine">{html_lib.escape(result.engine, quote=True)}</p>'
            "</article>"
        )

    body = "\n".join(result_items) or '<p class="no-results">No results found.</p>'
    if unresponsive_engines:
        failures = ", ".join(html_lib.escape(str(engine), quote=True) for engine, _ in unresponsive_engines)
        body += f'<p class="unresponsive">Unavailable engines: {failures}</p>'
    elapsed = int((meta or {}).get("response_time_ms", 0))
    return (
        "<!doctype html>\n"
        '<html lang="en"><head><meta charset="utf-8">'
        f"<title>Search results for {escaped_query}</title></head>\n"
        "<body>\n"
        '<form action="/search" method="get">'
        f'<input name="q" value="{escaped_query}">'
        '<button type="submit">Search</button></form>\n'
        f"<h1>Search results for {escaped_query}</h1>\n"
        f'<p class="summary">{len(results)} results in {elapsed} ms.</p>\n'
        f"{body}\n"
        "</body></html>"
    )


def format_csv(results: list[SearchResult]) -> str:
    """Format results as a deterministic CSV document."""
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["title", "url", "content", "engine", "publishedDate"])
    for result in results:
        writer.writerow([result.title, result.url, result.content, result.engine, result.published_date or ""])
    return output.getvalue()


def format_rss(results: list[SearchResult], query: str) -> str:
    """Format results as an RSS 2.0 document."""
    rss = ElementTree.Element("rss", {"version": "2.0"})
    channel = ElementTree.SubElement(rss, "channel")
    ElementTree.SubElement(channel, "title").text = f"SlopSearX results for {query}"
    ElementTree.SubElement(channel, "description").text = f"Search results for {query}"
    for result in results:
        item = ElementTree.SubElement(channel, "item")
        ElementTree.SubElement(item, "title").text = result.title
        ElementTree.SubElement(item, "link").text = result.url
        ElementTree.SubElement(item, "description").text = result.content or ""
        if result.published_date:
            ElementTree.SubElement(item, "pubDate").text = result.published_date
    return ElementTree.tostring(rss, encoding="unicode", xml_declaration=True)


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
            "partial": meta.get("partial", False),
            "deadline_exceeded": meta.get("deadline_exceeded", False),
            "query_id": meta.get("query_id", ""),
        }
        # Count responsive engines from engine_status
        if "engine_status" in meta:
            yaml_section["meta"]["engine_count"] = len(meta["engine_status"])
            ok_count = sum(1 for s in meta["engine_status"].values() if s.get("status") == "ok")
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
            "tier": r.tier,
            "media": media_to_dict(r.media),
            "payload": _payload_for_output(r.payload),
        }
        for r in results
    ]

    # Build the Markdown section
    total_engines = engine_count or (len(meta["engine_status"]) if meta and "engine_status" in meta else 0)
    ok_engines = responsive_count
    if ok_engines is None and meta and "engine_status" in meta:
        ok_engines = sum(1 for s in meta["engine_status"].values() if s.get("status") == "ok")

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
