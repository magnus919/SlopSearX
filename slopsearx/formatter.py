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
.theme-toggle:focus-visible, .search-input:focus-visible, .search-button:focus-visible, .result-link:focus-visible { outline: 3px solid var(--signal); outline-offset: 3px; }
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
.results-top { display: flex; justify-content: space-between; align-items: end; gap: 2rem; margin-bottom: 2.3rem; }
.results-top h1 { margin: .5rem 0 0; font: 500 clamp(2rem, 4vw, 3.8rem)/.95 Georgia, serif; letter-spacing: -.05em; }
.results-top .summary { max-width: 18rem; margin: 0; color: var(--muted); text-align: right; font-size: .85rem; }
.results-list { max-width: 850px; }
.result { position: relative; padding: 1.35rem 0 1.6rem; border-top: 1px solid var(--line); animation: rise .5s both; animation-delay: calc(var(--i, 0) * 55ms); }
.result::before { content: counter(result); counter-increment: result; position: absolute; left: -2.5rem; top: 1.45rem; color: var(--quiet); font: .68rem "SFMono-Regular", Consolas, monospace; }
.results-list { counter-reset: result; }
.result-link { display: inline; color: var(--ink); text-decoration: none; font: 500 clamp(1.25rem, 2.5vw, 1.8rem)/1.1 Georgia, serif; letter-spacing: -.025em; }
.result-link:hover { color: var(--accent-strong); }
.result-url { margin: .45rem 0 .6rem; color: var(--signal); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: .68rem "SFMono-Regular", Consolas, monospace; }
.result-content { max-width: 70ch; margin: 0; color: var(--muted); font-size: .91rem; }
.result-meta { display: flex; flex-wrap: wrap; gap: .5rem 1rem; margin-top: .9rem; color: var(--quiet); font: .65rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .04em; }
.result-meta span + span::before { content: "•"; margin-right: 1rem; color: var(--accent); }
.notice { margin: 1.5rem 0; padding: .8rem 1rem; border: 1px solid rgba(229,181,103,.45); color: var(--muted); background: rgba(229,181,103,.07); font-size: .82rem; }
.notice[data-kind="error"] { border-color: rgba(226,126,126,.65); background: rgba(226,126,126,.08); }
.scope-panel { margin: 1rem 0 2.2rem; padding: 1rem; border: 1px solid var(--line); background: rgba(34,35,31,.58); }
.scope-panel summary { cursor: pointer; color: var(--ink); font: 700 .72rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .08em; }
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
.pagination { display: flex; justify-content: space-between; gap: 1rem; max-width: 850px; padding: 1.5rem 0; border-top: 1px solid var(--line); }
.page-link { border: 1px solid var(--line); color: var(--muted); background: transparent; padding: .6rem .8rem; text-decoration: none; font: .68rem "SFMono-Regular", Consolas, monospace; text-transform: uppercase; letter-spacing: .05em; }
.page-link:hover { color: var(--ink); border-color: var(--accent); }
.empty { padding: 3rem 0; color: var(--muted); border-top: 1px solid var(--line); }
@keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@media (max-width: 720px) { .portal { width: min(100% - 1.5rem, 42rem); } .hero { display: block; padding: 4.5rem 0 3.5rem; } .hero h1 { font-size: clamp(3.2rem, 16vw, 5rem); } .hero-note { margin-top: 3rem; } .results-top { display: block; } .results-top .summary { margin-top: 1rem; text-align: left; } .result::before { display: none; } .search-form { display: block; } .search-button { width: 100%; min-height: 3rem; } .search-hint { display: none; } .portal-footer { display: block; } .portal-footer span { display: block; margin-top: .5rem; } .scope-grid { grid-template-columns: 1fr; } .scope-apply { width: 100%; } .pagination { display: grid; grid-template-columns: 1fr 1fr; } .page-link { text-align: center; } }
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
    if (button) { button.textContent = darker ? "Darker · on" : "Darker · off"; button.setAttribute("aria-pressed", String(darker)); }
  };
  update();
  if (button) button.addEventListener("click", function () {
    root.dataset.theme = root.dataset.theme === "darker" ? "dark" : "darker";
    try { window.localStorage.setItem("slopsearx-theme", root.dataset.theme); } catch (_) { /* storage is optional */ }
    update();
  });
  const input = document.querySelector(".search-input");
  document.addEventListener("keydown", function (event) {
    if ((event.key === "/" || (event.key === "k" && (event.metaKey || event.ctrlKey))) && document.activeElement !== input) {
      event.preventDefault(); if (input) input.focus();
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
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker · off</button></header>
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
    return f"""
<details class="scope-panel">
  <summary>Scope and filters</summary>
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
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker · off</button></header>
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
    result_items: list[str] = []
    for index, result in enumerate(results):
        title = html_lib.escape(str(result.title or result.url or "Untitled result"), quote=True)
        raw_url = _safe_href(result.url)
        url = html_lib.escape(raw_url, quote=True)
        domain = html_lib.escape(urlparse(raw_url).netloc if raw_url != "#" else "unknown source", quote=True)
        content = html_lib.escape(result.content or "")
        category = html_lib.escape(result.category or "web", quote=True)
        published = html_lib.escape((result.published_date or "").split("T", 1)[0], quote=True)
        engines = sorted(result.engines) if result.engines else [result.engine]
        sources = ", ".join(html_lib.escape(str(engine), quote=True) for engine in engines if engine)
        metadata = f"<span>{category}</span>" + (f"<span>{published}</span>" if published else "")
        metadata += f"<span>Source{('s' if len(engines) != 1 else '')}: {sources or 'unknown'}</span>"
        special_bits: list[str] = []
        media = media_to_dict(result.media)
        if media:
            media_type = html_lib.escape(str(media.get("media_type", "media")), quote=True)
            special_bits.append(f"<span>{media_type} result</span>")
        payload = _payload_for_output(result.payload)
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
        result_items.append(
            f'<article class="result" style="--i:{index}">'
            f'<h2><a class="result-link" href="{url}" target="_blank" rel="noopener noreferrer">{title}</a></h2>'
            f'<p class="result-url">{domain}</p>'
            f'<p class="result-content">{content}</p>'
            f'{special}{media_markup}<div class="result-meta">{metadata}</div>'
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
    if unresponsive_engines:
        failures = ", ".join(
            f"{html_lib.escape(str(engine), quote=True)} ({html_lib.escape(str(reason), quote=True)})"
            for engine, reason in unresponsive_engines
        )
        body += f'<p class="scope-note">Unavailable sources: {failures}</p>'
    elapsed = int((meta or {}).get("response_time_ms", 0))
    all_unresponsive = bool(_portal_state_value(portal_state, "all_unresponsive", False))
    partial = bool((meta or {}).get("partial", False)) or bool(unresponsive_engines)
    if all_unresponsive or (unresponsive_engines and not results):
        unavailable = '<div class="notice" data-kind="error" role="alert">No configured source returned a result. You can retry this search.</div>'
    elif partial:
        unavailable = '<div class="notice" role="status">Some sources could not answer this search. The results below are still usable.</div>'
    else:
        unavailable = ""
    state = portal_state or {}
    scope_label = html_lib.escape(str(_portal_state_value(state, "scope_label", "All sources")), quote=True)
    selected_count = int(_portal_state_value(state, "selected_engine_count", 0) or 0)
    responded_count = int(_portal_state_value(state, "responsive_engine_count", 0) or 0)
    scope_note = (
        f"{scope_label} · {responded_count} of {selected_count} sources answered" if selected_count else scope_label
    )
    previous = int(_portal_state_value(state, "page", 1) or 1) > 1
    next_page = bool(results)
    pagination = ""
    if previous or next_page:
        pagination_links: list[str] = []
        if previous:
            pagination_links.append(
                f'<a class="page-link" href="{html_lib.escape(_portal_page_url(state, max(1, int(_portal_state_value(state, "page", 1)) - 1)), quote=True)}">← Previous</a>'
            )
        else:
            pagination_links.append("<span></span>")
        if next_page:
            pagination_links.append(
                f'<a class="page-link" href="{html_lib.escape(_portal_page_url(state, int(_portal_state_value(state, "page", 1)) + 1), quote=True)}">Next →</a>'
            )
        pagination = f'<nav class="pagination" aria-label="Pagination">{"".join(pagination_links)}</nav>'
    search_hidden = _portal_hidden_inputs(state, exclude={"q", "pageno"}, page=1)
    content = f"""
<main class="portal">
  <header class="masthead"><a class="brand" href="/"><span class="brand-mark"><span>⌁</span></span><span>SLOPSEARX</span></a><button class="theme-toggle" type="button" data-theme-toggle aria-pressed="false">Darker · off</button></header>
  <section class="results-shell" aria-labelledby="results-title">
    <div class="results-top"><div><div class="eyebrow">Search results</div><h1 id="results-title">{escaped_query}</h1></div><p class="summary">{len(results)} results · {elapsed} ms<br>{scope_note}</p></div>
    <section class="search-panel" aria-label="Refine search"><form class="search-form" action="/search" method="get"><label class="sr-only" for="portal-query">Search SlopSearX</label><input class="search-input" id="portal-query" name="q" type="search" value="{escaped_query}" required>{search_hidden}<button class="search-button" type="submit">Search ↗</button></form></section>
    {_portal_scope_panel(state)}
    {unavailable}
    <div class="results-list">{body}</div>
    {pagination}
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
