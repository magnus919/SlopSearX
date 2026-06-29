"""DuckDuckGo HTML scrape adapter.

⚠️  Legal notice: DuckDuckGo does not provide a public search API.
This adapter scrapes the HTML search results page (https://html.duckduckgo.com/).
Use of this adapter may be subject to DuckDuckGo's Terms of Service.
This adapter is best-effort with no SLA — HTML structure changes,
CAPTCHA walls, and rate limiting may break it at any time.
"""

from __future__ import annotations

import time
import urllib.parse
from typing import Any

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
    categories = ["general", "news", "images"]

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://html.duckduckgo.com/html/")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        # Determine if this is an image search
        categories = (params or {}).get("categories", [])
        is_image_search = "images" in categories

        data: dict[str, str] = {"q": query}
        if is_image_search:
            data["iar"] = "images"
        headers = self.request_headers
        proxy = self._get_proxy()
        client_kwargs: dict[str, Any] = {
            "timeout": timeout_ms / 1000.0,
            "follow_redirects": True,
        }
        if proxy:
            client_kwargs["proxies"] = proxy

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(**client_kwargs) as client:
                resp = await client.post(base_url, data=data, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code in (403, 503):
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                if self._is_challenge_page(resp.text):
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)

                self._report_proxy_success(proxy)
                if is_image_search:
                    results = self._parse_image_html(resp.text, query, max_results)
                else:
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
                results.append(
                    SearchResult(
                        url=url,
                        title=title or query,
                        content=content,
                        img_src=img_src or None,
                        engine=self.name,
                        category="images",
                        position=i + 1,
                    ),
                )

        return results
