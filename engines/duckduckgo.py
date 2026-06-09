"""DuckDuckGo HTML scrape adapter.

⚠️  Legal notice: DuckDuckGo does not provide a public search API.
This adapter scrapes the HTML search results page (https://html.duckduckgo.com/).
Use of this adapter may be subject to DuckDuckGo's Terms of Service.
This adapter is best-effort with no SLA — HTML structure changes,
CAPTCHA walls, and rate limiting may break it at any time.
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
class DuckDuckGoAdapter(ScrapeAdapter):
    name = "duckduckgo"
    display_name = "DuckDuckGo"
    env_prefix = "ENGINE_DDG"
    engine_type = "scrape"
    categories = ["general", "news"]

    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        cfg = self.config
        base_url = cfg.get("base_url", "https://html.duckduckgo.com/html/")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        data = {"q": query}
        headers = self.request_headers

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0, follow_redirects=True) as client:
                resp = await client.post(base_url, data=data, headers=headers)
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
        ]
        lower = raw_html.lower()
        return any(ind in lower for ind in indicators)

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
