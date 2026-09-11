"""Output formatters — SearXNG JSON and YAML+Markdown.

The internal SearchResult dataclass is decoupled from the wire format.
Formatters map between the internal model and output serialization.
"""

# The self-contained portal stylesheet is intentionally readable as CSS. Its
# long lines are not Python formatting problems and are covered by browser
# rendering tests instead.
# ruff: noqa: E501

from __future__ import annotations

import csv
import datetime
import html as html_lib
import io
import re
from typing import Any
from urllib.parse import urlencode, urlparse
from xml.etree import ElementTree

import yaml

from slopsearx.adapter import SearchResult, media_to_dict
from slopsearx.payload import (
    PAYLOAD_INLINE_BYTES,
    payload_max_persist_bytes,
    payload_serialized_size,
    payload_to_dict,
)
from slopsearx.retrieval_url import RETRIEVAL_URL_STATUS_OK, classify_retrieval_url

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
        "parsed_url": _parsed_url(result.url),
        "open_group": False,
        "close_group": False,
        "priority": "",
    }


def _parsed_url(url: str | None) -> list[str] | None:
    """Return SearXNG's JSON-safe six-part URL parse result.

    Search results should normally contain absolute URLs.  Keep malformed or
    absent values from breaking the whole response and make the tuple-like
    ``ParseResult`` explicit as a JSON array at this boundary.
    """
    if not url:
        return None
    try:
        parsed = urlparse(url)
    except (TypeError, ValueError):
        return None
    if not parsed.scheme or not parsed.netloc:
        return None
    return [parsed.scheme, parsed.netloc, parsed.path, parsed.params, parsed.query, parsed.fragment]


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


_PORTAL_CSS = """
:root {
  color-scheme: dark;
  --ink: #f4f0e8;
  --muted: #a49e91;
  --quiet: #6e6a61;
  --paper: #191a18;
  --paper-raised: #22231f;
  --paper-lift: #2a2b26;
  --line: rgba(244, 240, 232, .13);
  --accent: #e5b567;
  --accent-strong: #f1c979;
  --signal: #a7d7c5;
  --shadow: rgba(0, 0, 0, .32);
}
:root[data-theme="darker"] {
  --ink: #f8f3e9;
  --muted: #a9a398;
  --quiet: #716c62;
  --paper: #0b0c0b;
  --paper-raised: #121311;
  --paper-lift: #1a1b18;
  --line: rgba(248, 243, 233, .15);
  --accent: #f0bd65;
  --accent-strong: #ffd88b;
  --signal: #b8e4d2;
  --shadow: rgba(0, 0, 0, .5);
}
* { box-sizing: border-box; }
html { min-width: 320px; background: var(--paper); }
body {
  margin: 0; min-height: 100vh; color: var(--ink); background:
    radial-gradient(circle at 8% -10%, rgba(229,181,103,.14), transparent 32rem),
    radial-gradient(circle at 92% 20%, rgba(167,215,197,.07), transparent 28rem),
    var(--paper);
  font-family: "Avenir Next", "Segoe UI", sans-serif; line-height: 1.5;
}
body::before { content: ""; position: fixed; inset: 0; pointer-events: none; opacity: .22;
  background-image: linear-gradient(var(--line) 1px, transparent 1px), linear-gradient(90deg, var(--line) 1px, transparent 1px);
  background-size: 5rem 5rem; mask-image: linear-gradient(to bottom, black, transparent 70%);
}
a { color: inherit; }
.portal { position: relative; width: min(1180px, calc(100% - 2.5rem)); margin: 0 auto; }
.masthead { display: flex; justify-content: space-between; align-items: center; padding: 1.6rem 0; border-bottom: 1px solid var(--line); }
.brand { display: inline-flex; align-items: center; gap: .7rem; text-decoration: none; letter-spacing: .09em; font: 700 .78rem/1 "SFMono-Regular", Consolas, monospace; }
.brand-mark { display: grid; place-items: center; width: 2rem; height: 2rem; border: 1px solid var(--accent); color: var(--accent); transform: rotate(45deg); }
.brand-mark span { transform: rotate(-45deg); }
.theme-toggle { border: 1px solid var(--line); border-radius: 99px; color: var(--muted); background: transparent; cursor: pointer; padding: .6rem .85rem; font: 600 .7rem/1 "SFMono-Regular", Consolas, monospace; transition: border-color .2s, color .2s, background .2s; }
.theme-toggle:hover, .theme-toggle:focus-visible { color: var(--ink); border-color: var(--accent); background: var(--paper-lift); }
.theme-toggle:focus-visible, .search-input:focus-visible, .search-button:focus-visible, .result-link:focus-visible, .result-explanation summary:focus-visible, .explanation-link:focus-visible { outline: 3px solid var(--signal); outline-offset: 3px; }
.eyebrow { color: var(--accent); text-transform: uppercase; letter-spacing: .16em; font: 700 .7rem/1.2 "SFMono-Regular", Consolas, monospace; }
.hero { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(18rem, .8fr); gap: 5rem; align-items: end; padding: clamp(5rem, 12vw, 9rem) 0 5rem; }
.hero h1 { max-width: 12ch; margin: .8rem 0 1.25rem; font: 500 clamp(3.3rem, 8vw, 7rem)/.9 Georgia, "Times New Roman", serif; letter-spacing: -.065em; }
.hero h1 em { color: var(--accent); font-style: normal; }
.hero-copy { max-width: 31rem; margin: 0; color: var(--muted); font-size: 1.05rem; }
.hero-note { border-left: 1px solid var(--accent); padding: 0 0 0 1.35rem; color: var(--muted); font-size: .92rem; }
.hero-note strong { display: block; margin-bottom: .45rem; color: var(--ink); font: 600 1rem Georgia, serif; }
.search-panel { padding: .45rem; background: var(--paper-raised); border: 1px solid var(--line); box-shadow: 0 1.2rem 3rem var(--shadow); }
.search-form { display: flex; gap: .45rem; }
.search-input { min-width: 0; flex: 1; border: 0; color: var(--ink); background: transparent; padding: 1rem 1.1rem; font: 1rem "Avenir Next", "Segoe UI", sans-serif; }
.search-input::placeholder { color: var(--quiet); }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
.search-button { border: 0; color: #171710; background: var(--accent); cursor: pointer; padding: 0 1.35rem; font: 700 .75rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .08em; transition: background .2s, transform .2s; }
.search-button:hover { background: var(--accent-strong); transform: translateY(-1px); }
.search-hint { display: flex; justify-content: space-between; gap: 1rem; padding: .65rem 1rem .35rem; color: var(--quiet); font: .66rem "SFMono-Regular", Consolas, monospace; }
.search-hint kbd { border: 1px solid var(--line); border-radius: .2rem; padding: .12rem .3rem; color: var(--muted); }
.portal-footer { display: flex; justify-content: space-between; gap: 1rem; padding: 2.5rem 0 2rem; color: var(--quiet); font: .68rem "SFMono-Regular", Consolas, monospace; }
.results-shell { padding: 3rem 0 1rem; }
.results-shell > .search-panel { position: sticky; top: .5rem; z-index: 3; }
.results-top { display: flex; justify-content: space-between; align-items: end; gap: 2rem; margin-bottom: 2.3rem; }
.results-top h1 { max-width: 18ch; margin: .5rem 0 0; font: 500 clamp(2rem, 4vw, 3.8rem)/.95 Georgia, serif; letter-spacing: -.05em; overflow-wrap: anywhere; }
.results-top .summary { max-width: 21rem; margin: 0; color: var(--muted); text-align: right; font-size: .85rem; }
.results-top .summary strong { color: var(--ink); font-weight: 600; }
.results-layout { display: grid; grid-template-columns: minmax(0, 1fr) 15.5rem; gap: 2.2rem; align-items: start; }
.results-main { min-width: 0; }
.results-list { display: grid; gap: .85rem; counter-reset: result; }
.result { position: relative; min-width: 0; padding: 1.25rem 1.3rem 1.1rem 4.2rem; border: 1px solid var(--line); border-radius: .18rem; background: linear-gradient(135deg, rgba(42,43,38,.72), rgba(34,35,31,.38)); box-shadow: 0 .9rem 2.2rem rgba(0,0,0,.12); animation: rise .5s both; animation-delay: calc(var(--i, 0) * 55ms); transition: border-color .2s, transform .2s, background .2s; }
.result:hover { border-color: rgba(229,181,103,.48); background: linear-gradient(135deg, rgba(48,47,39,.82), rgba(34,35,31,.52)); transform: translateY(-2px); }
.result.is-active { border-color: var(--signal); box-shadow: 0 0 0 2px rgba(167,215,197,.18), 0 1rem 2.6rem rgba(0,0,0,.2); }
.result::before { content: counter(result, decimal-leading-zero); counter-increment: result; position: absolute; left: 1.25rem; top: 1.25rem; width: 2rem; height: 2rem; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 50%; color: var(--accent); background: var(--paper-lift); font: .68rem "SFMono-Regular", Consolas, monospace; }
.result-kicker { display: flex; align-items: center; flex-wrap: wrap; gap: .45rem .65rem; margin-bottom: .55rem; color: var(--muted); font: .66rem "SFMono-Regular", Consolas, monospace; }
.result-source-line { display: inline-flex; align-items: center; min-width: 0; max-width: 100%; color: var(--signal); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.result-source-line::before { content: ""; width: .4rem; height: .4rem; flex: 0 0 auto; margin-right: .45rem; border-radius: 50%; background: var(--signal); box-shadow: 0 0 .55rem rgba(167,215,197,.5); }
.result-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--quiet); }
.result-pill { display: inline-flex; align-items: center; border: 1px solid var(--line); border-radius: 99px; padding: .24rem .5rem; color: var(--muted); background: rgba(11,12,11,.18); font: 600 .64rem/1 "SFMono-Regular", Consolas, monospace; letter-spacing: .04em; text-transform: uppercase; }
.result-pill.type { color: var(--accent); border-color: rgba(229,181,103,.34); }
.result-pill.consensus { color: var(--signal); border-color: rgba(167,215,197,.35); }
.result-link { display: inline; color: var(--ink); text-decoration: none; font: 500 clamp(1.18rem, 2.1vw, 1.58rem)/1.12 Georgia, serif; letter-spacing: -.025em; }
.result-link:hover { color: var(--accent-strong); }
.result-content { display: -webkit-box; max-width: 72ch; margin: .65rem 0 0; overflow: hidden; color: var(--muted); font-size: .91rem; line-height: 1.62; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }
.result-meta { display: flex; flex-wrap: wrap; gap: .45rem .9rem; margin-top: .9rem; color: var(--muted); font: .7rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .04em; }
.result-meta span + span::before { content: "•"; margin-right: .9rem; color: var(--accent); }
.result-actions { display: flex; flex-wrap: wrap; gap: .55rem; margin-top: 1rem; padding-top: .8rem; border-top: 1px solid rgba(244,240,232,.08); }
.result-action { color: var(--muted); text-decoration: none; font: 700 .62rem "SFMono-Regular", Consolas, monospace; letter-spacing: .06em; text-transform: uppercase; }
.result-action:hover, .result-action:focus-visible { color: var(--accent-strong); }
.result-explanation { margin-top: .9rem; border-top: 1px solid rgba(244,240,232,.08); color: var(--muted); font-size: .78rem; }
.result-explanation summary { cursor: pointer; padding: .8rem 0 .15rem; color: var(--accent); font: 700 .64rem "SFMono-Regular", Consolas, monospace; letter-spacing: .06em; text-transform: uppercase; }
.explanation-list { display: grid; gap: .55rem; margin: .7rem 0 .2rem; padding: 0; list-style: none; }
.explanation-list strong { color: var(--ink); font-weight: 600; }
.explanation-sources { display: inline-flex; flex-wrap: wrap; gap: .3rem; margin-left: .25rem; }
.explanation-link { color: var(--signal); }
.explanation-status { color: var(--quiet); }
.explanation-status[data-status="available"], .explanation-status[data-status="eligible"] { color: var(--signal); }
.explanation-status[data-status="unavailable"], .explanation-status[data-status="expired"], .explanation-status[data-status="ineligible"] { color: #e8b0a8; }
.results-rail { display: grid; gap: .8rem; position: sticky; top: 1rem; }
.rail-card { padding: 1rem; border: 1px solid var(--line); background: rgba(34,35,31,.58); }
.rail-label { margin: 0 0 .8rem; color: var(--accent); font: 700 .63rem "SFMono-Regular", Consolas, monospace; letter-spacing: .12em; text-transform: uppercase; }
.rail-count { display: block; color: var(--ink); font: 500 3.4rem/.9 Georgia, serif; letter-spacing: -.07em; }
.rail-copy { margin: .55rem 0 0; color: var(--muted); font-size: .78rem; }
.rail-detail { display: flex; justify-content: space-between; gap: .5rem; margin-top: .9rem; padding-top: .75rem; border-top: 1px solid var(--line); color: var(--muted); font: .68rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; }
.rail-sources { display: grid; gap: .55rem; margin: 0; padding: 0; list-style: none; }
.rail-source { display: flex; align-items: center; justify-content: space-between; gap: .7rem; color: var(--muted); font: .7rem "SFMono-Regular", Consolas, monospace; }
.rail-source-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rail-source-count { color: var(--signal); }
.rail-source-unavailable { color: #e8b0a8; }
.rail-source-unavailable .rail-source-count { color: #e8b0a8; font-size: .62rem; text-align: right; }
.rail-source-empty { color: var(--muted); }
.rail-source-empty .rail-source-count { color: var(--muted); font-size: .62rem; text-align: right; }
.rail-link { display: inline-flex; margin-top: 1rem; color: var(--accent); text-decoration: none; font: 700 .62rem "SFMono-Regular", Consolas, monospace; letter-spacing: .06em; text-transform: uppercase; }
.rail-link:hover { color: var(--accent-strong); }
.notice { margin: 1.5rem 0; padding: .8rem 1rem; border: 1px solid rgba(229,181,103,.45); color: var(--muted); background: rgba(229,181,103,.07); font-size: .82rem; }
.notice strong { color: var(--ink); }
.notice[data-kind="error"] { border-color: rgba(226,126,126,.65); background: rgba(226,126,126,.08); }
.scope-panel { margin: 1rem 0 2.2rem; padding: 1rem; border: 1px solid var(--line); background: rgba(34,35,31,.58); }
.scope-panel summary { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; cursor: pointer; color: var(--ink); font: 700 .72rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .08em; }
.scope-summary { color: var(--muted); font-size: .65rem; font-weight: 500; letter-spacing: .03em; text-transform: none; }
.scope-grid { display: grid; grid-template-columns: minmax(0, 1.2fr) repeat(2, minmax(0, 1fr)) auto; gap: .8rem; margin-top: 1rem; }
.scope-field { display: grid; gap: .35rem; color: var(--muted); font: .68rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .05em; }
.scope-field select { width: 100%; border: 1px solid var(--line); border-radius: .2rem; color: var(--ink); background: var(--paper-lift); padding: .7rem .75rem; font: .85rem "Avenir Next", "Segoe UI", sans-serif; }
.scope-field select:focus-visible, .scope-apply:focus-visible, .page-link:focus-visible { outline: 3px solid var(--signal); outline-offset: 3px; }
.scope-apply { align-self: end; min-height: 2.55rem; border: 1px solid var(--accent); color: var(--accent); background: transparent; cursor: pointer; padding: 0 .9rem; font: 700 .68rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .06em; }
.scope-apply:hover { color: #171710; background: var(--accent); }
.scope-note { margin: .9rem 0 0; color: var(--quiet); font-size: .75rem; }
.scope-status { display: flex; flex-wrap: wrap; gap: .45rem .9rem; margin: .9rem 0 0; color: var(--muted); font: .67rem "SFMono-Regular", Consolas, monospace; }
.scope-status strong { color: var(--ink); }
.enforcement { color: var(--quiet); }
.enforcement[data-status="enforced"] { color: var(--signal); }
.enforcement[data-status="rejected"] { color: #e29a9a; }
.result-special { display: flex; flex-wrap: wrap; gap: .45rem; margin-top: .9rem; }
.result-special span { border: 1px solid var(--line); color: var(--accent); padding: .22rem .45rem; font: .62rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .05em; }
.result-media { max-width: 22rem; margin: 1rem 0 0; border: 1px solid var(--line); background: var(--paper-raised); }
.result-media img { display: block; width: 100%; max-height: 14rem; object-fit: cover; }
.result-media figcaption { padding: .45rem .6rem; color: var(--quiet); font: .65rem "SFMono-Regular", Consolas, monospace; }
.suggestions { display: flex; flex-wrap: wrap; gap: .45rem; margin-top: 1rem; }
.suggestions a { border-bottom: 1px solid var(--accent); color: var(--accent); text-decoration: none; font-size: .82rem; }
.pagination { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: 1.5rem 0; border-top: 1px solid var(--line); }
.page-current { color: var(--muted); font: .68rem "SFMono-Regular", Consolas, monospace; letter-spacing: .05em; text-transform: uppercase; }
.page-spacer { min-width: 5rem; }
.page-link { border: 1px solid var(--line); color: var(--muted); background: transparent; padding: .6rem .8rem; text-decoration: none; font: .68rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .05em; }
.page-link:hover { color: var(--ink); border-color: var(--accent); }
.empty { padding: 3rem 0; color: var(--muted); border-top: 1px solid var(--line); }
@keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@media (max-width: 900px) { .results-layout { grid-template-columns: 1fr; } .results-rail { position: static; grid-template-columns: repeat(2, minmax(0, 1fr)); } }
@media (max-width: 720px) { .portal { width: min(100% - 1.5rem, 42rem); } .hero { display: block; padding: 4.5rem 0 3.5rem; } .hero h1 { font-size: clamp(3.2rem, 16vw, 5rem); } .hero-note { margin-top: 3rem; } .results-top { display: block; } .results-top .summary { margin-top: 1rem; text-align: left; } .result { padding: 1.1rem 1rem 1rem 3.7rem; } .result::before { left: .9rem; top: 1.1rem; width: 2rem; height: 2rem; } .result-kicker { display: block; } .result-kicker > * { margin: 0 .45rem .35rem 0; } .search-form { display: block; } .search-button { width: 100%; min-height: 3rem; } .search-hint { display: none; } .portal-footer { display: block; } .portal-footer span { display: block; margin-top: .5rem; } .scope-grid { grid-template-columns: 1fr; } .scope-apply { width: 100%; } .pagination { display: grid; grid-template-columns: 1fr 1fr 1fr; } .page-link { text-align: center; } .page-current { text-align: center; } .page-spacer { min-width: 0; } .results-rail { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation-duration: .01ms !important; transition-duration: .01ms !important; } }
"""


_PORTAL_SCRIPT = """
(function () {
  const root = document.documentElement;
  let stored = null;
  try { stored = window.localStorage.getItem("slopsearx-theme"); } catch (_) { /* storage is optional */ }
  const configured = root.dataset.defaultTheme || "dark";
  const allowed = ["dark", "darker"];
  const theme = allowed.includes(stored) ? stored : (allowed.includes(configured) ? configured : "dark");
  root.dataset.theme = theme;
  const button = document.querySelector("[data-theme-toggle]");
  const update = function () {
    const darker = root.dataset.theme === "darker";
    if (button) {
      button.textContent = darker ? "Dark mode" : "Darker mode";
      button.setAttribute("aria-label", darker ? "Use dark mode" : "Use darker mode");
      button.setAttribute("aria-pressed", String(darker));
    }
  };
  update();
  if (button) button.addEventListener("click", function () {
    root.dataset.theme = root.dataset.theme === "darker" ? "dark" : "darker";
    try { window.localStorage.setItem("slopsearx-theme", root.dataset.theme); } catch (_) { /* storage is optional */ }
    update();
  });
  const scopePanel = document.querySelector(".scope-panel");
  if (scopePanel && window.matchMedia && window.matchMedia("(max-width: 900px)").matches) scopePanel.removeAttribute("open");
  const input = document.querySelector(".search-input");
  const resultCards = Array.from(document.querySelectorAll("[data-result-card]"));
  let activeResult = -1;
  const setActiveResult = function (next) {
    if (!resultCards.length) return;
    activeResult = Math.max(0, Math.min(resultCards.length - 1, next));
    resultCards.forEach(function (card, index) {
      card.classList.toggle("is-active", index === activeResult);
    });
    resultCards[activeResult].scrollIntoView({ block: "nearest", behavior: "smooth" });
  };
  document.addEventListener("keydown", function (event) {
    const target = event.target;
    const typing = target && (target.matches("input, textarea, select") || target.isContentEditable);
    if ((event.key === "/" || (event.key === "k" && (event.metaKey || event.ctrlKey))) && document.activeElement !== input) {
      event.preventDefault(); if (input) input.focus();
    } else if (!typing && event.key === "j" && resultCards.length) {
      event.preventDefault(); setActiveResult(activeResult + 1);
    } else if (!typing && event.key === "k" && resultCards.length) {
      event.preventDefault(); setActiveResult(activeResult - 1);
    } else if (!typing && event.key === "Enter" && activeResult >= 0) {
      const link = resultCards[activeResult].querySelector(".result-link");
      if (link) { event.preventDefault(); window.open(link.href, "_blank", "noopener,noreferrer"); }
    }
  });
}());
"""


def _safe_href(url: str | None) -> str:
    """Return only browser-safe absolute HTTP(S) URLs for result links."""
    if not url:
        return "#"
    try:
        parsed = urlparse(url)
    except (TypeError, ValueError):
        return "#"
    return url if parsed.scheme in {"http", "https"} and parsed.netloc else "#"


def _portal_content_markup(content: str | None) -> str:
    """Escape result snippets while preserving the engines' plain highlights.

    Scrape adapters commonly return ``<strong>`` around query matches.  The
    portal must not trust arbitrary upstream markup, but showing those tags as
    literal text makes otherwise valid snippets look broken.  Escape first and
    then restore only exact, attribute-free highlight tags from the tiny
    allowlist used by the portal.
    """
    source = content or ""
    allowed = re.compile(r"<(strong|em|mark)>(.*?)</\1>", re.IGNORECASE | re.DOTALL)
    pieces: list[str] = []
    cursor = 0
    for match in allowed.finditer(source):
        pieces.append(html_lib.escape(source[cursor : match.start()]))
        tag = match.group(1).lower()
        pieces.append(f"<{tag}>{html_lib.escape(match.group(2))}</{tag}>")
        cursor = match.end()
    pieces.append(html_lib.escape(source[cursor:]))
    return "".join(pieces)


def _portal_document(*, body: str, title: str, default_theme: str = "dark") -> str:
    """Build the browser document with the portal's self-contained shell."""
    safe_theme = default_theme if default_theme in {"dark", "darker"} else "dark"
    return (
        '<!doctype html>\n<html lang="en" data-default-theme="'
        + safe_theme
        + '"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">'
        + f'<meta name="description" content="SlopSearX — search across the open web and specialist sources."><title>{html_lib.escape(title)}</title>'
        + "<style>"
        + _PORTAL_CSS
        + "</style></head><body>"
        + body
        + "<script>"
        + _PORTAL_SCRIPT
        + "</script></body></html>"
    )


def format_landing_page(*, default_theme: str = "dark") -> str:
    """Render the human-facing portal landing page for a bare root visit."""
    body = """
<main class="portal">
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker mode</button></header>
  <section class="hero" aria-labelledby="hero-title">
    <div><div class="eyebrow">Search</div><h1 id="hero-title">Search <em>SlopSearX.</em></h1><p class="hero-copy">Search across the web and specialist sources configured for this instance.</p></div>
    <aside class="hero-note"><strong>Configured sources</strong>Results identify their source. If an engine cannot answer, the page says so.</aside>
  </section>
  <section class="search-panel" aria-label="Search"><form class="search-form" action="/search" method="get"><label class="sr-only" for="portal-query">Search SlopSearX</label><input class="search-input" id="portal-query" name="q" type="search" placeholder="Search" autocomplete="off" autofocus required><button class="search-button" type="submit">Search ↗</button></form><div class="search-hint"><span>Enter a query</span><span><kbd>/</kbd> focus search</span></div></section>
  <footer class="portal-footer"><span>Sources are configured by the operator.</span><span>Press / to focus search.</span></footer>
</main>
"""
    return _portal_document(body=body, title="SlopSearX — search", default_theme=default_theme)


def _portal_state_value(state: dict[str, Any] | None, key: str, default: Any = "") -> Any:
    """Read a trusted, server-built portal state value."""
    if not state:
        return default
    return state.get(key, default)


def _portal_hidden_inputs(
    state: dict[str, Any] | None,
    *,
    exclude: set[str] | frozenset[str] = frozenset(),
    page: int = 1,
) -> str:
    """Render the allow-listed search state as hidden form controls."""
    values = {
        "q": _portal_state_value(state, "query"),
        "categories": _portal_state_value(state, "categories"),
        "engines": _portal_state_value(state, "engines"),
        "language": _portal_state_value(state, "language", "en"),
        "time_range": _portal_state_value(state, "time_range"),
        "safesearch": str(_portal_state_value(state, "safesearch", 0)),
        "pageno": str(page),
    }
    return "".join(
        f'<input type="hidden" name="{html_lib.escape(name, quote=True)}" value="{html_lib.escape(str(value), quote=True)}">'
        for name, value in values.items()
        if name not in exclude and value not in (None, "")
    )


def _portal_page_url(state: dict[str, Any] | None, page: int) -> str:
    """Build a reproducible browser URL for a numbered result page."""
    values = {
        "q": _portal_state_value(state, "query"),
        "categories": _portal_state_value(state, "categories"),
        "engines": _portal_state_value(state, "engines"),
        "language": _portal_state_value(state, "language", "en"),
        "time_range": _portal_state_value(state, "time_range"),
        "safesearch": str(_portal_state_value(state, "safesearch", 0)),
        "pageno": str(page),
    }
    return "/search?" + urlencode({key: value for key, value in values.items() if value not in (None, "")})


def _portal_format_url(state: dict[str, Any] | None, output_format: str) -> str:
    """Build a machine-view link that preserves the visible search state."""
    values = {
        "q": _portal_state_value(state, "query"),
        "categories": _portal_state_value(state, "categories"),
        "engines": _portal_state_value(state, "engines"),
        "language": _portal_state_value(state, "language", "en"),
        "time_range": _portal_state_value(state, "time_range"),
        "safesearch": str(_portal_state_value(state, "safesearch", 0)),
        "pageno": str(_portal_state_value(state, "page", 1)),
        "format": output_format,
    }
    return "/search?" + urlencode({key: value for key, value in values.items() if value not in (None, "")})


def _portal_engine_label(engine: str) -> str:
    """Turn an internal engine slug into a compact, human-readable label."""
    return str(engine).replace("_", " ").replace("-", " ").title()


def _portal_result_type(
    result: SearchResult, category: str, media: dict[str, Any] | None, payload: dict[str, Any] | None
) -> str:
    """Return an honest, low-signal result label from declared source metadata."""
    if media:
        return str(media.get("media_type", "media")).title()
    if payload:
        return "Structured"
    root = category.split(":", 1)[0].strip().lower()
    labels = {
        "science": "Research",
        "github": "Developer",
        "packages": "Package",
        "security": "Security",
        "jobs": "Jobs",
        "news": "News",
        "reference": "Reference",
        "media": "Media",
    }
    return labels.get(root, "General")


def _portal_source_counts(results: list[SearchResult]) -> list[tuple[str, int]]:
    """Count the source appearances represented on the current result page."""
    counts: dict[str, int] = {}
    for result in results:
        engines = result.engines or {result.engine}
        for engine in engines:
            if engine:
                counts[str(engine)] = counts.get(str(engine), 0) + 1
    return sorted(counts.items(), key=lambda item: (-item[1], item[0]))


def _portal_ranking_explanation(strategy: str, tier: int) -> str:
    """Describe ordering as ranking arithmetic, never confidence."""
    if strategy == "tier_then_reciprocal_rank_fusion_k60":
        method = "Reciprocal rank fusion (k=60) combines each source's result order"
    else:
        method = "Cross-source presence counts configured sources returning the same normalized URL"
    return f"{method}, after configured tier {tier}. The score controls ordering; it is not confidence."


def _portal_identifier(identifier: Any) -> str:
    if not isinstance(identifier, dict):
        return ""
    return ", ".join(
        f"{str(key).replace('_', ' ')}: {value}" for key, value in sorted(identifier.items()) if value is not None
    )


def _portal_result_explanation(result: SearchResult, index: int, state: dict[str, Any]) -> str:
    """Render bounded provenance, identity, ranking, and handoff facts."""
    engines = sorted(str(engine) for engine in (result.engines or {result.engine}) if engine)
    source_labels = "".join(
        f'<span class="result-pill">{html_lib.escape(_portal_engine_label(engine), quote=True)}</span>'
        for engine in engines
    )
    strategy = str(_portal_state_value(state, "ranking_explanation", "tier_then_cross_engine_presence"))
    ranking = html_lib.escape(_portal_ranking_explanation(strategy, int(result.tier or 2)), quote=True)

    grouping_status = str(_portal_state_value(state, "grouping_status", "unavailable"))
    groups = _portal_state_value(state, "result_groups", {})
    group = groups.get(str(index)) if isinstance(groups, dict) else None
    entity_markup: str
    if isinstance(group, dict) and group.get("entity_id"):
        namespace = html_lib.escape(str(group.get("namespace") or "entity"), quote=True)
        identifier = html_lib.escape(_portal_identifier(group.get("identifier")), quote=True)
        member_indices = [value for value in group.get("result_indices", []) if type(value) is int and value >= 0]
        member_links = " ".join(
            f'<a class="explanation-link" href="#result-{member + 1}">#{member + 1}</a>' for member in member_indices
        )
        conflicts = [str(field) for field in group.get("conflicting_fields", []) if str(field)]
        conflict_markup = ""
        if conflicts:
            conflict_names = html_lib.escape(", ".join(sorted(conflicts)), quote=True)
            conflict_markup = (
                f"<li><strong>Source-reported conflicts:</strong> {conflict_names}. "
                f"Inspect contributing results: {member_links}</li>"
            )
        entity_markup = (
            f'<li><strong>Entity:</strong> <span class="explanation-status" data-status="available">'
            f"{namespace} — {identifier}</span> · {len(member_indices)} result"
            f"{'s' if len(member_indices) != 1 else ''} in this canonical response</li>{conflict_markup}"
        )
    elif isinstance(group, dict):
        group_reason = html_lib.escape(str(group.get("reason") or "unsupported result metadata"), quote=True)
        entity_markup = f"<li><strong>Entity grouping:</strong> unsupported for this result ({group_reason.replace('_', ' ')}).</li>"
    else:
        status = grouping_status if grouping_status in {"unavailable", "partial", "empty", "expired"} else "unavailable"
        descriptions = {
            "unavailable": "unavailable for this response",
            "partial": "partial; no supported identity was available for this result",
            "empty": "empty; no supported identities were found",
            "expired": "expired with its source snapshot",
        }
        entity_markup = (
            f'<li><strong>Entity grouping:</strong> <span class="explanation-status" data-status="{status}">'
            f"{descriptions[status]}</span>.</li>"
        )

    url_status, retrieval_reason, _scheme, _url = classify_retrieval_url(result.url)
    if url_status == RETRIEVAL_URL_STATUS_OK:
        retrieval = (
            '<span class="explanation-status" data-status="eligible">structurally eligible</span> for downstream '
            "retrieval. The page has not been fetched or verified; a retriever must check the resolved destination."
        )
    else:
        reason_text = html_lib.escape(str(retrieval_reason or url_status).replace("_", " "), quote=True)
        retrieval = (
            '<span class="explanation-status" data-status="ineligible">not eligible</span> for the technical '
            f"handoff ({reason_text})."
        )
    return (
        '<details class="result-explanation"><summary>Why this result appeared</summary>'
        '<ul class="explanation-list">'
        f'<li><strong>Contributors:</strong><span class="explanation-sources">{source_labels}</span> '
        f"({len(engines)} configured source{'s' if len(engines) != 1 else ''})</li>"
        f"<li><strong>Ranking:</strong> {ranking}</li>{entity_markup}"
        f"<li><strong>Retrieval handoff:</strong> {retrieval}</li>"
        "</ul></details>"
    )


def _portal_scope_panel(state: dict[str, Any] | None) -> str:
    """Render capability-aware scope and filter controls."""
    state = state or {}
    categories = str(_portal_state_value(state, "categories"))
    selected_time_range = str(_portal_state_value(state, "time_range"))
    selected_safesearch = str(_portal_state_value(state, "safesearch", 0))
    category_options = [str(value) for value in _portal_state_value(state, "category_options", [])]

    category_markup = [f'<option value=""{" selected" if not categories else ""}>All sources</option>']
    for value in category_options:
        selected = " selected" if value == categories else ""
        label = value.replace(":", " / ").replace("_", " ").title()
        category_markup.append(
            f'<option value="{html_lib.escape(value, quote=True)}"{selected}>{html_lib.escape(label)}</option>'
        )
    time_options = [
        ("", "Any time"),
        ("day", "Past day"),
        ("week", "Past week"),
        ("month", "Past month"),
        ("year", "Past year"),
    ]
    time_markup = [
        f'<option value="{html_lib.escape(value, quote=True)}"{" selected" if value == selected_time_range else ""}>{label}</option>'
        for value, label in time_options
    ]
    safe_options = [("0", "Off"), ("1", "Moderate"), ("2", "Strict")]
    safe_markup = [
        f'<option value="{value}"{" selected" if value == selected_safesearch else ""}>{label}</option>'
        for value, label in safe_options
    ]

    enforcement = _portal_state_value(state, "filter_enforcement", {})
    status_bits: list[str] = []
    for key, label in (("time_range", "time"), ("safesearch", "SafeSearch")):
        entry = enforcement.get(key) if isinstance(enforcement, dict) else None
        if not isinstance(entry, dict) or not entry.get("requested"):
            continue
        status = str(entry.get("status", "unsupported"))
        reason = html_lib.escape(str(entry.get("reason", "")))
        status_bits.append(
            f'<span class="enforcement" data-status="{html_lib.escape(status, quote=True)}">{label}: {status.replace("_", " ")} — {reason}</span>'
        )
    status_markup = f'<div class="scope-status" aria-live="polite">{"".join(status_bits)}</div>' if status_bits else ""
    note = html_lib.escape(str(_portal_state_value(state, "scope_note", "")))
    note_markup = f'<p class="scope-note">{note}</p>' if note else ""
    scope_summary = " · ".join(
        [
            categories.replace(":", " / ").replace("_", " ").title() if categories else "All sources",
            next((label for value, label in time_options if value == selected_time_range), "Any time"),
            f"SafeSearch: {next((label for value, label in safe_options if value == selected_safesearch), 'Off')}",
        ]
    )
    return f"""
<details class="scope-panel" open>
  <summary><span>Scope and filters</span><span class="scope-summary">{html_lib.escape(scope_summary)}</span></summary>
  <form action="/search" method="get">
    {_portal_hidden_inputs(state, exclude={"categories", "time_range", "safesearch", "pageno"}, page=1)}
    <div class="scope-grid">
      <label class="scope-field">Scope<select name="categories">{"".join(category_markup)}</select></label>
      <label class="scope-field">Time range<select name="time_range">{"".join(time_markup)}</select></label>
      <label class="scope-field">SafeSearch<select name="safesearch">{"".join(safe_markup)}</select></label>
      <button class="scope-apply" type="submit">Apply filters</button>
    </div>
    {note_markup}{status_markup}
  </form>
</details>"""


def format_error_html(
    error: str,
    message: str,
    *,
    field: str | None = None,
    default_theme: str = "dark",
) -> str:
    """Render a safe, useful browser error without changing machine errors."""
    escaped_error = html_lib.escape(error, quote=True)
    escaped_message = html_lib.escape(message, quote=True)
    field_hint = f'<p class="scope-note">Field: {html_lib.escape(field, quote=True)}</p>' if field else ""
    body = f"""
<main class="portal">
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker mode</button></header>
  <section class="results-shell" aria-labelledby="error-title">
    <div class="eyebrow">Search request</div><h1 id="error-title">{escaped_error}</h1>
    <div class="notice" data-kind="error" role="alert"><strong>{escaped_message}</strong>{field_hint}</div>
    <p><a class="page-link" href="/">Return to search</a></p>
  </section>
  <footer class="portal-footer"><span>Sources are configured by the operator.</span><span>Press / to focus search.</span></footer>
</main>"""
    return _portal_document(body=body, title=f"SlopSearX — {error}", default_theme=default_theme)


def format_html(
    results: list[SearchResult],
    query: str,
    *,
    meta: dict[str, Any] | None = None,
    unresponsive_engines: list[list[str]] | None = None,
    portal_state: dict[str, Any] | None = None,
    default_theme: str = "dark",
) -> str:
    """Format the human-facing, safe HTML search-results page."""
    escaped_query = html_lib.escape(query, quote=True)
    state = portal_state or {"query": query}
    json_enabled = bool(_portal_state_value(state, "json_enabled", True))
    result_items: list[str] = []
    for index, result in enumerate(results):
        title = html_lib.escape(str(result.title or result.url or "Untitled result"), quote=True)
        raw_url = _safe_href(result.url)
        url = html_lib.escape(raw_url, quote=True)
        parsed_result_url = urlparse(raw_url) if raw_url != "#" else None
        domain = html_lib.escape(parsed_result_url.netloc if parsed_result_url else "unknown source", quote=True)
        result_path = html_lib.escape((parsed_result_url.path or "/") if parsed_result_url else "", quote=True)
        content = _portal_content_markup(result.content)
        category_value = str(result.category or "web")
        category = html_lib.escape(category_value, quote=True)
        published = html_lib.escape((result.published_date or "").split("T", 1)[0], quote=True)
        engines = sorted(result.engines) if result.engines else [result.engine]
        media = media_to_dict(result.media)
        payload = _payload_for_output(result.payload)
        source_pills = "".join(
            f'<span class="result-pill">{html_lib.escape(_portal_engine_label(str(engine)), quote=True)}</span>'
            for engine in engines
            if engine
        )
        consensus_count = len(engines)
        consensus = (
            f'<span class="result-pill consensus" title="Same URL returned by {consensus_count} configured engines" '
            f'aria-label="Same URL returned by {consensus_count} configured engines">Matched {consensus_count} sources</span>'
            if consensus_count > 1
            else ""
        )
        result_type = html_lib.escape(_portal_result_type(result, category_value, media, payload), quote=True)
        special_bits: list[str] = []
        if media:
            media_type = html_lib.escape(str(media.get("media_type", "media")), quote=True)
            special_bits.append(f"<span>{media_type} result</span>")
        if payload:
            domain_name = html_lib.escape(str(payload.get("domain", "specialist")), quote=True)
            payload_type = html_lib.escape(str(payload.get("type", "result")), quote=True)
            special_bits.append(f"<span>{domain_name} / {payload_type}</span>")
        special = f'<div class="result-special">{"".join(special_bits)}</div>' if special_bits else ""
        media_markup = ""
        if media:
            thumbnail = _safe_href(media.get("thumbnail"))
            if thumbnail != "#":
                caption = html_lib.escape(str(media.get("source") or domain), quote=True)
                media_markup = (
                    f'<figure class="result-media"><img src="{html_lib.escape(thumbnail, quote=True)}" '
                    f'alt="{title}" loading="lazy"><figcaption>{caption}</figcaption></figure>'
                )
        path_markup = f'<span class="result-path">{result_path}</span>' if result_path else ""
        metadata = f"<span>{category}</span>" + (f"<span>{published}</span>" if published else "")
        actions = (
            '<div class="result-actions"><a class="result-action" href="{}" target="_blank" rel="noopener noreferrer">'
            "Open result ↗</a></div>"
        ).format(url)
        explanation = _portal_result_explanation(result, index, state)
        result_items.append(
            f'<article class="result" id="result-{index + 1}" data-result-card style="--i:{index}">'
            f'<div class="result-kicker"><span class="result-source-line">{domain}</span>{path_markup}'
            f'<span class="result-pill type">{result_type}</span>{consensus}</div>'
            f'<h2><a class="result-link" href="{url}" target="_blank" rel="noopener noreferrer">{title}</a></h2>'
            f'<p class="result-content">{content}</p>'
            f'{special}{media_markup}<div class="result-meta">{source_pills}{metadata}</div>{explanation}{actions}'
            "</article>"
        )

    suggestions = [str(item) for item in _portal_state_value(portal_state, "suggestions", []) if str(item).strip()]
    if result_items:
        body = "\n".join(result_items)
    else:
        body = f'<div class="empty"><p>No results found for <strong>{escaped_query}</strong>.</p>'
        if suggestions:
            suggestion_links = "".join(
                f'<a href="/search?{urlencode({"q": suggestion})}">{html_lib.escape(suggestion, quote=True)}</a>'
                for suggestion in suggestions[:5]
            )
            body += f'<div class="suggestions" aria-label="Suggestions">{suggestion_links}</div>'
        body += "</div>"
    source_failures = [
        (str(engine), str(reason))
        for entry in (unresponsive_engines or [])
        if len(entry) >= 2
        for engine, reason in [entry[:2]]
    ]
    empty_sources = [
        (str(engine), str(reason))
        for entry in (meta or {}).get("empty_engines", [])
        if len(entry) >= 2
        for engine, reason in [entry[:2]]
    ]
    unavailable_labels = ", ".join(
        f"{html_lib.escape(_portal_engine_label(engine), quote=True)} ({html_lib.escape(reason, quote=True)})"
        for engine, reason in source_failures
    )
    empty_labels = ", ".join(
        f"{html_lib.escape(_portal_engine_label(engine), quote=True)} ({html_lib.escape(reason, quote=True)})"
        for engine, reason in empty_sources
    )
    elapsed = int((meta or {}).get("response_time_ms", 0))
    all_unresponsive = bool(_portal_state_value(portal_state, "all_unresponsive", False))
    partial = bool((meta or {}).get("partial", False)) or bool(source_failures)
    if all_unresponsive or (source_failures and not results):
        detail = f" Unavailable: {unavailable_labels}." if unavailable_labels else ""
        unavailable = f'<div class="notice" data-kind="error" role="alert"><strong>No configured source returned a result.</strong> You can retry this search.{detail}</div>'
    elif partial:
        details = []
        if unavailable_labels:
            details.append(f"Unavailable: {unavailable_labels}.")
        if empty_labels:
            details.append(f"No results: {empty_labels}.")
        detail = f" {' '.join(details)}" if details else ""
        unavailable = f'<div class="notice" role="status"><strong>Some sources could not answer this search.</strong>{detail} The results below are still usable.</div>'
    elif empty_labels:
        unavailable = f'<div class="notice" role="status"><strong>No results from some sources.</strong> No results: {empty_labels}. Other results remain usable.</div>'
    else:
        unavailable = ""
    scope_label = html_lib.escape(str(_portal_state_value(state, "scope_label", "All sources")), quote=True)
    selected_count = int(_portal_state_value(state, "selected_engine_count", 0) or 0)
    responded_count = int(_portal_state_value(state, "responsive_engine_count", 0) or 0)
    scope_note = (
        f"{scope_label} · {responded_count} of {selected_count} sources answered" if selected_count else scope_label
    )
    current_page = max(1, int(_portal_state_value(state, "page", 1) or 1))
    previous = current_page > 1
    # Most upstream engines do not expose an authoritative total.  Callers
    # may provide ``has_more`` when they know it; otherwise label the action
    # as an attempt instead of presenting availability as a fact.
    has_more = _portal_state_value(state, "has_more", None)
    next_page = bool(results) if has_more is None else bool(has_more)
    next_label = "Try next page →" if has_more is None else "Next page →"
    next_aria_label = "Try next result page" if has_more is None else "Next result page"
    page_context = f"Page {current_page}"
    if has_more is None and next_page:
        page_context += " · more may be available"
    pagination = ""
    if previous or next_page:
        pagination_links: list[str] = []
        if previous:
            pagination_links.append(
                f'<a class="page-link" aria-label="Previous result page" href="{html_lib.escape(_portal_page_url(state, current_page - 1), quote=True)}">← Previous</a>'
            )
        else:
            pagination_links.append('<span class="page-spacer" aria-hidden="true"></span>')
        pagination_links.append(f'<span class="page-current" aria-current="page">{page_context}</span>')
        if next_page:
            pagination_links.append(
                f'<a class="page-link" aria-label="{next_aria_label}" href="{html_lib.escape(_portal_page_url(state, current_page + 1), quote=True)}">{next_label}</a>'
            )
        pagination = f'<nav class="pagination" aria-label="Pagination">{"".join(pagination_links)}</nav>'
    search_hidden = _portal_hidden_inputs(state, exclude={"q", "pageno"}, page=1)
    source_counts = _portal_source_counts(results)
    source_rows = "".join(
        f'<li class="rail-source"><span class="rail-source-name">{html_lib.escape(_portal_engine_label(engine), quote=True)}</span>'
        f'<span class="rail-source-count">{count}</span></li>'
        for engine, count in source_counts[:7]
    )
    unavailable_rows = "".join(
        f'<li class="rail-source rail-source-unavailable"><span class="rail-source-name">{html_lib.escape(_portal_engine_label(engine), quote=True)}</span>'
        f'<span class="rail-source-count">{html_lib.escape(reason, quote=True)}</span></li>'
        for engine, reason in source_failures
    )
    empty_rows = "".join(
        f'<li class="rail-source rail-source-empty"><span class="rail-source-name">{html_lib.escape(_portal_engine_label(engine), quote=True)}</span>'
        f'<span class="rail-source-count">no results</span></li>'
        for engine, _reason in empty_sources
    )
    if not source_rows:
        source_rows = '<li class="rail-source"><span class="rail-source-name">No responding sources</span></li>'
    source_rows += unavailable_rows + empty_rows
    json_link = (
        f'<a class="rail-link" href="{html_lib.escape(_portal_format_url(state, "json"), quote=True)}">Open JSON view ↗</a>'
        if json_enabled
        else ""
    )
    rail = f"""
<aside class="results-rail" aria-label="Search summary">
  <section class="rail-card">
    <p class="rail-label">Result signal</p>
    <strong class="rail-count">{len(results)}</strong>
    <p class="rail-copy">merged results on this page</p>
    <div class="rail-detail"><span>{responded_count} sources answered</span><span>{elapsed} ms</span></div>
  </section>
  <section class="rail-card">
  <p class="rail-label">Source status</p>
    <ul class="rail-sources">{source_rows}</ul>
    {json_link}
  </section>
</aside>"""
    content = f"""
<main class="portal">
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker mode</button></header>
  <section class="results-shell" aria-labelledby="results-title">
    <div class="results-top"><div><div class="eyebrow">Search results</div><h1 id="results-title">{escaped_query}</h1></div><p class="summary">{len(results)} results · {elapsed} ms<br>{scope_note}</p></div>
    <section class="search-panel" aria-label="Refine search"><form class="search-form" action="/search" method="get"><label class="sr-only" for="portal-query">Search SlopSearX</label><input class="search-input" id="portal-query" name="q" type="search" value="{escaped_query}" required>{search_hidden}<button class="search-button" type="submit">Search ↗</button></form></section>
    {_portal_scope_panel(state)}
    {unavailable}
    <div class="results-layout"><div class="results-main"><div class="results-list">{body}</div>{pagination}</div>{rail}</div>
  </section>
  <footer class="portal-footer"><span>Sources are configured by the operator.</span><span>Press / to focus search.</span></footer>
</main>
"""
    return _portal_document(body=content, title=f"Search results for {query}", default_theme=default_theme)


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
