"""Bing HTML scrape adapter.

Legal notice: Bing scraping may be subject to Microsoft's Terms of Use.
This adapter sends HTTP GET requests to the public results page
(https://www.bing.com/search) and parses the HTML response. It is
best-effort with no SLA: HTML structure changes, CAPTCHA walls, and rate
limiting may break it at any time.

Bing's results page renders its organic results (``li.b_algo``) without
JavaScript, which keeps it usable where the Google page is a JS wall.
Result links point at Bing's click tracker (``/ck/a?...&u=a1<base64url>``);
the target URL is decoded from the ``u`` parameter.
"""

from __future__ import annotations

import base64
import binascii
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

# Only checked when a page has no organic results: a normal results page also
# carries words like "challenge" in its scripts.
_CAPTCHA_MARKERS = ("b_captcha", "captcha", "solve the challenge", "unusual traffic", "verify you are a human")


def decode_bing_url(href: str) -> str | None:
    """Return the target URL of a Bing result link, or ``None`` for Bing-internal links."""
    try:
        parsed = urllib.parse.urlparse(href)
    except ValueError:
        return None
    if parsed.scheme not in ("http", "https") or not parsed.netloc:
        return None
    host = parsed.netloc.lower()
    if host != "bing.com" and not host.endswith(".bing.com"):
        return href
    if not parsed.path.startswith("/ck/a"):
        return None
    token = (urllib.parse.parse_qs(parsed.query).get("u") or [""])[0]
    if not token.startswith("a1"):
        return None
    encoded = token[2:]
    try:
        target = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)).decode("utf-8")
    except (binascii.Error, UnicodeDecodeError, ValueError):
        return None
    return target if target.startswith(("http://", "https://")) else None


@register_engine
class BingAdapter(ScrapeAdapter):
    name = "bing"
    display_name = "Bing"
    env_prefix = "ENGINE_BING"
    engine_type = "scrape"
    categories = ["general"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
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
        base_url = cfg.get("base_url", "https://www.bing.com/search")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 10)

        params_dict = {"q": query, "setlang": "en", "count": str(max_results)}
        proxy = self._get_proxy()
        client_kwargs: dict[str, Any] = {
            "timeout": timeout_ms / 1000.0,
            "follow_redirects": True,
        }
        if proxy:
            client_kwargs["proxies"] = proxy

        start_time = time.monotonic()
        try:
            async with self.http_client(**client_kwargs) as client:
                resp = await client.get(base_url, params=params_dict, headers=self.request_headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code in (403, 503):
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                resp.raise_for_status()

                results = self._parse_html(resp.text, max_results)
                if not results and self._is_captcha_page(resp.text):
                    self._report_proxy_failure(proxy)
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)

                self._report_proxy_success(proxy)
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

    @staticmethod
    def _is_captcha_page(raw_html: str) -> bool:
        lower = raw_html.lower()
        return any(marker in lower for marker in _CAPTCHA_MARKERS)

    def _parse_html(self, raw_html: str, max_results: int) -> list[SearchResult]:
        """Parse Bing's organic results (``li.b_algo``); ads and Bing-internal links are skipped."""
        if not raw_html.strip():
            return []
        doc = html.fromstring(raw_html)
        results: list[SearchResult] = []
        seen_urls: set[str] = set()
        for node in doc.cssselect("li.b_algo"):
            if len(results) >= max_results:
                break
            links = node.cssselect("h2 a")
            if not links:
                continue
            url = decode_bing_url(links[0].get("href", ""))
            if not url or url in seen_urls:
                continue
            seen_urls.add(url)
            title = links[0].text_content().strip()
            captions = node.cssselect(".b_caption p")
            snippet = captions[0].text_content().strip() if captions else ""
            results.append(
                SearchResult(
                    url=url,
                    title=title or url,
                    content=snippet,
                    engine=self.name,
                    position=len(results) + 1,
                ),
            )
        return results
