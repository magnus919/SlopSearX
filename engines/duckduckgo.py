"""DuckDuckGo HTML scrape adapter.

⚠️  Legal notice: DuckDuckGo does not provide a public search API.
This adapter scrapes the HTML search results pages (https://html.duckduckgo.com/
with https://lite.duckduckgo.com/lite/ as a fallback frontend).
Use of this adapter may be subject to DuckDuckGo's Terms of Service.
This adapter is best-effort with no SLA — HTML structure changes,
CAPTCHA walls, and rate limiting may break it at any time.

Resilience measures (best-effort mitigations, not guarantees):
- A realistic browser-like session (Chrome UA, Sec-Fetch headers, homepage
  bootstrap visit so cookie state looks organic) lowers the chance of an
  anonymous-scrape wall.
- When the primary ``html`` frontend is blocked or serves an unusable page,
  text searches fall back to the lightweight ``lite`` frontend, which runs
  separate infrastructure and is frequently still reachable.
- Blocked/CAPTCHA walls are classified as ``EngineStatus.BLOCKED`` (part of
  the adapter's declared ``failure_classes``) instead of masquerading as
  "success with zero results".
"""

from __future__ import annotations

import time
import urllib.parse
from typing import Any, NamedTuple

import httpx
from lxml import html

from slopsearx.adapter import (
    AdapterResponse,
    EngineStatus,
    ScrapeAdapter,
    SearchResult,
    build_media,
    register_engine,
)


@register_engine
class DuckDuckGoAdapter(ScrapeAdapter):
    name = "duckduckgo"
    display_name = "DuckDuckGo"
    env_prefix = "ENGINE_DDG"
    engine_type = "scrape"
    categories = ["general", "news", "images"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text", "media")
    supported_media_types = ("image",)
    failure_classes = ("rate_limited", "blocked", "error", "timeout")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://html.duckduckgo.com/html/")
        fallback_url = cfg.get("fallback_base_url", "https://lite.duckduckgo.com/lite/")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        # Determine if this is an image search
        categories = (params or {}).get("categories", [])
        is_image_search = "images" in categories

        data: dict[str, str] = {"q": query}
        if is_image_search:
            data["iar"] = "images"

        proxy = self._get_proxy()
        client_kwargs: dict[str, Any] = {
            "timeout": timeout_ms / 1000.0,
            "follow_redirects": True,
        }
        if proxy:
            client_kwargs["proxies"] = proxy

        start_time = time.monotonic()

        def _latency() -> float:
            return (time.monotonic() - start_time) * 1000

        try:
            async with httpx.AsyncClient(**client_kwargs) as client:
                # Warm the session (cookies / anti-bot state) before searching.
                await self._bootstrap_session(client)

                outcome = await self._search_via(
                    client,
                    base_url,
                    data,
                    query,
                    max_results,
                    is_image_search,
                    proxy,
                    parser="html",
                )
                if not outcome.retriable:
                    return AdapterResponse(
                        results=outcome.results,
                        status=outcome.status,
                        error_message=outcome.error_message,
                        latency_ms=_latency(),
                    )

                # The primary frontend could not serve the query. Only the
                # text layout has a lite twin (lite carries no image tiles).
                if is_image_search:
                    return AdapterResponse(
                        results=[],
                        status=outcome.status,
                        error_message=outcome.error_message,
                        latency_ms=_latency(),
                    )

                fb = await self._search_via(
                    client,
                    fallback_url,
                    data,
                    query,
                    max_results,
                    False,
                    proxy,
                    parser="lite",
                )
                if fb.status is EngineStatus.OK and fb.results:
                    return AdapterResponse(
                        results=fb.results,
                        status=EngineStatus.OK,
                        latency_ms=_latency(),
                    )

                # The fallback answered with a definitive non-OK state of its
                # own (rate limit / timeout / walled) — surface THAT honestly
                # instead of folding it into the primary's outcome.
                if fb.status in (EngineStatus.RATE_LIMITED, EngineStatus.TIMEOUT):
                    return AdapterResponse(
                        results=[],
                        status=fb.status,
                        error_message=(
                            f"{base_url} unusable ({outcome.error_message or 'unusable response'}); "
                            f"{fallback_url}: {fb.error_message}"
                        ),
                        latency_ms=_latency(),
                    )
                if fb.status is EngineStatus.BLOCKED:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        error_message=(
                            f"DuckDuckGo blocked or challenge-walled on all endpoints: "
                            f"{base_url} ({outcome.error_message or 'unusable response'}); "
                            f"{fallback_url}: {fb.error_message}"
                        ),
                        latency_ms=_latency(),
                    )
                if outcome.status is EngineStatus.BLOCKED and fb.status is EngineStatus.OK:
                    # Lite served a well-formed page but genuinely had no
                    # results for the query; the primary was walled. Report
                    # an honest empty result set with the wall detail.
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.BLOCKED,
                        error_message=(
                            f"{base_url} blocked/challenge-walled; {fallback_url} served no results for this query"
                        ),
                        latency_ms=_latency(),
                    )
                # Neither endpoint produced parsable results.
                return AdapterResponse(
                    results=[],
                    status=fb.status if fb.status is not EngineStatus.OK else EngineStatus.ERROR,
                    error_message=(
                        f"DuckDuckGo served no parsable results on any endpoint: "
                        f"{base_url} ({outcome.error_message or 'empty'}); "
                        f"{fallback_url} ({fb.error_message or 'empty'})"
                    ),
                    latency_ms=_latency(),
                )

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    async def _bootstrap_session(self, client: httpx.AsyncClient) -> None:
        """Visit the DuckDuckGo homepage so the session carries organic cookie state.

        Best-effort by design: any failure here is swallowed — the search
        itself must proceed even when the bootstrap visit is refused. The
        visit carries its own short timeout so a hung homepage can never
        consume the engine's dispatch budget needed for the actual search
        (and the lite fallback).
        """
        bootstrap_url = self.config.get("bootstrap_url", "https://duckduckgo.com/")
        bootstrap_budget = min(float(self.config.get("bootstrap_timeout_ms", 2_500)), 5.0) / 1000.0
        try:
            await client.get(bootstrap_url, headers=self._session_headers(), timeout=bootstrap_budget)
        except Exception:  # noqa: BLE001 — never fail the search for a warm-up visit
            pass

    def _session_headers(self) -> dict[str, str]:
        """Browser-realistic headers for the homepage bootstrap visit."""
        return {
            "User-Agent": self.request_headers["User-Agent"],
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "en-US,en;q=0.9",
            "Sec-Fetch-Dest": "document",
            "Sec-Fetch-Mode": "navigate",
            "Sec-Fetch-Site": "none",
            "Upgrade-Insecure-Requests": "1",
        }

    def _search_headers(self, url: str) -> dict[str, str]:
        """Browser-realistic headers for a search submission.

        ``Origin``/``Referer``/``Sec-Fetch-Site`` are derived from the actual
        target host so the fingerprint stays internally consistent: a form
        submission from duckduckgo.com to one of its own search frontends is
        same-site; anything else would be cross-site.
        """
        target = urllib.parse.urlparse(url)
        target_host = (target.hostname or "").lower()
        home_host = (
            urllib.parse.urlparse(self.config.get("bootstrap_url", "https://duckduckgo.com/")).hostname or ""
        ).lower()
        scheme = target.scheme or "https"
        if target_host == home_host:
            sec_fetch_site = "same-origin"
        elif target_host.endswith("." + home_host):
            # Another host of the same site (e.g. the html./lite. frontends).
            sec_fetch_site = "same-site"
        else:
            sec_fetch_site = "cross-site"
        return {
            "User-Agent": self.request_headers["User-Agent"],
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            "Accept-Language": "en-US,en;q=0.9",
            "Referer": f"{scheme}://{home_host}/",
            "Origin": f"{scheme}://{home_host}",
            "Sec-Fetch-Dest": "document",
            "Sec-Fetch-Mode": "navigate",
            # A browser navigating from the homepage to another host of the
            # same site sends "same-site", never "same-origin".
            "Sec-Fetch-Site": sec_fetch_site,
            "Upgrade-Insecure-Requests": "1",
        }

    async def _search_via(
        self,
        client: httpx.AsyncClient,
        endpoint_url: str,
        data: dict[str, str],
        query: str,
        max_results: int,
        is_image_search: bool,
        proxy: dict[str, str] | None,
        parser: str = "html",
    ) -> _EndpointOutcome:
        """Attempt one DuckDuckGo frontend and classify the outcome.

        ``parser`` selects the layout to parse (``"html"`` for the primary
        results markup, ``"lite"`` for the lite table layout).
        ``retriable=True`` marks outcomes where a second frontend may still
        serve the query (blocks, walls, unparsable bodies). Rate limits and
        timeouts are terminal for the request — retrying would only deepen
        the throttling or double the latency budget.
        """
        try:
            resp = await client.post(
                endpoint_url,
                data=data,
                headers=self._search_headers(endpoint_url),
            )

            if resp.status_code == 429:
                self._report_proxy_failure(proxy)
                return _EndpointOutcome(EngineStatus.RATE_LIMITED, [])
            if resp.status_code in (403, 503):
                self._report_proxy_failure(proxy)
                return _EndpointOutcome(
                    EngineStatus.BLOCKED,
                    [],
                    f"{endpoint_url} answered {resp.status_code}",
                    retriable=True,
                )
            resp.raise_for_status()

            body = resp.text
            if self._is_challenge_page(body):
                self._report_proxy_failure(proxy)
                return _EndpointOutcome(
                    EngineStatus.BLOCKED,
                    [],
                    f"{endpoint_url} served a CAPTCHA/challenge wall",
                    retriable=True,
                )

            if is_image_search:
                # Image tiles live only on the JS-heavy frontends; the lite
                # page mirrors none of that layout, so a tile-free page is
                # treated as a genuine zero-result answer, not a wall.
                results = self._parse_image_html(body, query, max_results)
                self._report_proxy_success(proxy)
                return _EndpointOutcome(EngineStatus.OK, results)

            results = (
                self._parse_lite_html(body, query, max_results)
                if parser == "lite"
                else self._parse_html(body, query, max_results)
            )
            if results:
                self._report_proxy_success(proxy)
                return _EndpointOutcome(EngineStatus.OK, results)

            if self._looks_like_legitimate_empty(body):
                self._report_proxy_success(proxy)
                return _EndpointOutcome(EngineStatus.OK, [])

            # Structurally empty: no wall markers, no results — likely a soft
            # block or a layout change. Another endpoint may still answer.
            return _EndpointOutcome(
                EngineStatus.ERROR,
                [],
                f"{endpoint_url} returned no parsable results",
                retriable=True,
            )

        except httpx.TimeoutException:
            self._report_proxy_failure(proxy)
            return _EndpointOutcome(EngineStatus.TIMEOUT, [])
        except Exception as exc:  # noqa: BLE001 — network/HTTP layer failure
            self._report_proxy_failure(proxy)
            return _EndpointOutcome(
                EngineStatus.ERROR,
                [],
                f"{endpoint_url}: {exc}",
                retriable=True,
            )

    def _is_challenge_page(self, raw_html: str) -> bool:
        """Detect CAPTCHA or challenge walls in the response HTML.

        Checks for known DDG challenge indicators.
        """
        indicators = [
            "challenge",
            "verify you're human",
            "hcaptcha",
            "cf-browser-verification",
            "ddg_sl_",
            "data-challenge",
            "bots use duckduckgo",
            "anomaly detected",
        ]
        lower = raw_html.lower()
        return any(ind in lower for ind in indicators)

    def _looks_like_legitimate_empty(self, raw_html: str) -> bool:
        """True when DDG served a real zero-results page rather than a wall."""
        markers = [
            'class="no-results"',
            "no results</div>",
            "no results for",
            "did not match any documents",
        ]
        lower = raw_html.lower()
        return any(marker in lower for marker in markers)

    @staticmethod
    def _strip_ddg_redirect(url: str) -> str:
        """Unwrap DuckDuckGo's ``/l/?uddg=<encoded-target>`` redirect wrapper."""
        if "//duckduckgo.com/l/" not in url:
            return url
        parsed = urllib.parse.urlparse(url)
        qs = urllib.parse.parse_qs(parsed.query)
        unwrapped = qs.get("uddg", [""])[0]
        return unwrapped or url

    def _parse_lite_html(self, raw_html: str, query: str, max_results: int) -> list[SearchResult]:
        """Parse the lite.duckduckgo.com/lite/ table layout.

        The lite frontend renders results as alternating ``<tr>`` rows: a
        row carrying ``<a class="result-link">``, followed by a snippet row
        and a domain/date row. Links are wrapped in DDG redirects.
        """
        if self._is_challenge_page(raw_html):
            return []

        results: list[SearchResult] = []
        doc = html.fromstring(raw_html)

        for link in doc.cssselect("a.result-link"):
            if len(results) >= max_results:
                break

            url = self._strip_ddg_redirect(link.get("href", ""))
            title = link.text_content().strip()
            if not url or not title:
                continue

            snippet_rows = link.xpath(
                './ancestor::tr[1]/following-sibling::tr[1]//td[contains(@class,"result-snippet")]',
            )
            content = snippet_rows[0].text_content().strip() if isinstance(snippet_rows, list) and snippet_rows else ""

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=content,
                    engine=self.name,
                    position=len(results) + 1,
                ),
            )

        return results

    def _parse_html(self, raw_html: str, query: str, max_results: int) -> list[SearchResult]:
        """Parse DuckDuckGo HTML search results.

        Detects CAPTCHA walls by checking for known challenge indicators
        in the response body. Returns empty results with status logged
        when a challenge is detected.
        """
        if self._is_challenge_page(raw_html):
            return []

        results: list[SearchResult] = []
        doc = html.fromstring(raw_html)

        # DDG HTML results are in .result elements
        for i, node in enumerate(doc.cssselect(".result")):
            if len(results) >= max_results:
                break

            link_el = node.cssselect(".result__a")
            if not link_el:
                continue
            link = link_el[0]

            snippet_el = node.cssselect(".result__snippet")
            snippet = snippet_el[0].text_content().strip() if snippet_el else ""

            url_el = link.cssselect(".result__url")
            if not url_el:
                url = link.get("href", "")
            else:
                url = url_el[0].text_content().strip()

            # Strip DDG redirect
            if "//duckduckgo.com/l/" in url:
                url = url.split("?uddg=")[-1] if "?uddg=" in url else url

            results.append(
                SearchResult(
                    url=url,
                    title=link.text_content().strip(),
                    content=snippet,
                    engine=self.name,
                    position=i + 1,
                ),
            )

        return results

    def _parse_image_html(self, raw_html: str, query: str, max_results: int) -> list[SearchResult]:
        """Parse DuckDuckGo HTML image search results.

        DDG image results appear as .tile--img or .result--image elements.
        Each tile contains a thumbnail image, source page link, and metadata.
        """
        if self._is_challenge_page(raw_html):
            return []

        results: list[SearchResult] = []
        doc = html.fromstring(raw_html)

        # Try modern tile-based image results first, fall back to result-based
        tiles = doc.cssselect(".tile--img, .result--image, .tile, .image-result")
        for i, node in enumerate(tiles):
            if len(results) >= max_results:
                break

            # Thumbnail URL from <img> tag
            img_el = node.cssselect("img")
            img_src = ""
            if img_el:
                img_src = img_el[0].get("src", "") or img_el[0].get("data-src", "")

            # Source page URL from <a> tag
            link_el = node.cssselect("a")
            url = ""
            if link_el:
                url = link_el[0].get("href", "")

            # Title from alt text or title attribute
            title = ""
            if img_el:
                title = img_el[0].get("alt", "") or img_el[0].get("title", "")
            if not title and link_el:
                title = link_el[0].text_content().strip()

            # Content / description
            desc_el = node.cssselect(".tile__caption, .result__content, .caption")
            content = desc_el[0].text_content().strip() if desc_el else ""

            # Strip DDG redirect from URL
            if "//duckduckgo.com/l/" in url:
                parsed = urllib.parse.urlparse(url)
                qs = urllib.parse.parse_qs(parsed.query)
                url = qs.get("uddg", [""])[0] or url

            # Clean up relative URLs for thumbnails
            if img_src and img_src.startswith("//"):
                img_src = "https:" + img_src

            if url:
                # Attach a media record only when the tile carries an image
                # source; a tile with a link but no <img> must never yield a
                # degenerate image record.
                media = None
                if img_src:
                    media = build_media(
                        "image",
                        url=img_src,
                        thumbnail=img_src,
                        source=url,
                    )
                results.append(
                    SearchResult(
                        url=url,
                        title=title or query,
                        content=content,
                        img_src=img_src or None,
                        engine=self.name,
                        category="images",
                        position=i + 1,
                        media=media,
                    ),
                )

        return results


class _EndpointOutcome(NamedTuple):
    """Classified result of a single-endpoint search attempt."""

    status: EngineStatus
    results: list[SearchResult]
    error_message: str | None = None
    # True when a second frontend may still serve the query (blocks, walls,
    # unparsable bodies). Rate limits and timeouts are terminal.
    retriable: bool = False
