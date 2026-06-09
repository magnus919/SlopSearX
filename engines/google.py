"""Google HTML scrape adapter.

⚠️  Legal notice: Google search scraping may be subject to Google's
Terms of Service. This adapter sends HTTP GET requests to the public
search page (https://www.google.com/search) and parses the HTML
response. Use of this adapter may carry Terms of Service risk.
This adapter is best-effort with no SLA — HTML structure changes,
reCAPTCHA walls, and rate limiting may break it at any time.
"""

from __future__ import annotations

import time

import httpx
from lxml import html

from slopsearx.adapter import (
    AdapterResponse,
    EngineStatus,
    ScrapeAdapter,
    SearchResult,
    register_engine,
)


@register_engine
class GoogleAdapter(ScrapeAdapter):
    name = "google"
    display_name = "Google Search"
    env_prefix = "ENGINE_GOOGLE"
    engine_type = "scrape"

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        base_url = cfg.get("base_url", "https://www.google.com/search")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        headers = self.request_headers
        params_dict = {"q": query, "hl": "en", "num": str(max_results)}

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0, follow_redirects=True) as client:
                resp = await client.get(base_url, params=params_dict, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code in (403, 503):
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                results = self._parse_html(resp.text, query, max_results)
                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

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

    def _is_challenge_page(self, raw_html: str) -> bool:
        """Detect CAPTCHA/reCAPTCHA or challenge walls in the response."""
        indicators = [
            "challenge",
            "recaptcha",
            "cf-browser-verification",
            "unusual traffic",
            "verify you're not a robot",
            "g-recaptcha",
        ]
        lower = raw_html.lower()
        return any(ind in lower for ind in indicators)

    def _parse_html(self, raw_html: str, query: str, max_results: int) -> list[SearchResult]:
        """Parse Google HTML search results (organic results only).

        Detects CAPTCHA walls before attempting to parse.
        """
        if self._is_challenge_page(raw_html):
            return []

        results: list[SearchResult] = []
        doc = html.fromstring(raw_html)

        # Google search result containers
        seen_urls: set[str] = set()
        for i, node in enumerate(doc.cssselect("div.g")):
            if len(results) >= max_results:
                break

            # Extract link
            link_el = node.cssselect("a[href^='http']")
            if not link_el:
                continue
            url = link_el[0].get("href", "")

            if not url or url in seen_urls:
                continue
            seen_urls.add(url)

            # Title
            h3 = node.cssselect("h3")
            title = h3[0].text_content().strip() if h3 else ""

            # Snippet
            span_el = node.cssselect("span.aCOpRe, div.VwiC3b, span.st")
            snippet = span_el[0].text_content().strip() if span_el else ""

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=snippet,
                    engine=self.name,
                    position=i + 1,
                ),
            )

        return results
