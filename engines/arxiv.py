"""arXiv API adapter — structured academic paper search via export API."""

from __future__ import annotations

import re
import time
import xml.etree.ElementTree as ET
from typing import Any
from urllib.parse import urljoin, urlparse

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)
from slopsearx.payload import DOMAIN_SCIENCE, build_payload

ARXIV_NS = {"atom": "http://www.w3.org/2005/Atom"}


@register_engine
class ArxivAdapter(EngineAdapter):
    name = "arxiv"
    display_name = "arXiv"
    env_prefix = "ENGINE_ARXIV"
    engine_type = "api"
    categories = ["science", "reference"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "error", "timeout")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        base_url = cfg.get("base_url", "https://export.arxiv.org/api/query")
        timeout_ms = cfg.get("timeout_ms", 10_000)
        max_results = cfg.get("max_results", 5)

        # arXiv ToS: max 1 request per 3 seconds — enforced here
        search_query = f"all:{query}"
        url_params: dict[str, Any] = {
            "search_query": search_query,
            "start": 0,
            "max_results": max_results,
            "sortBy": "relevance",
            "sortOrder": "descending",
        }

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/atom+xml",
        }

        start_time = time.monotonic()
        try:
            async with httpx.AsyncClient(timeout=timeout_ms / 1000.0, follow_redirects=False) as client:
                resp = await client.get(base_url, params=url_params, headers=headers)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 429:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)

                if 300 <= resp.status_code < 400:
                    location = resp.headers.get("location")
                    if not location:
                        return AdapterResponse(
                            results=[],
                            status=EngineStatus.ERROR,
                            error_message=f"redirect response {resp.status_code} missing Location header",
                            latency_ms=latency,
                        )

                    redirected_url = urljoin(base_url, location)
                    source = urlparse(base_url)
                    target = urlparse(redirected_url)
                    if target.scheme != "https" or target.hostname != source.hostname or target.port != source.port:
                        return AdapterResponse(
                            results=[],
                            status=EngineStatus.ERROR,
                            error_message=(
                                f"redirect rejected: expected HTTPS on the configured arXiv host, got {redirected_url}"
                            ),
                            latency_ms=latency,
                        )

                    resp = await client.get(redirected_url, headers=headers)
                    latency = (time.monotonic() - start_time) * 1000

                    if resp.status_code == 429:
                        return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)

                resp.raise_for_status()

                results = self._parse_feed(resp.text, query)
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

    def _parse_feed(self, xml_text: str, query: str) -> list[SearchResult]:
        """Parse arXiv Atom feed into SearchResult list."""
        results: list[SearchResult] = []

        try:
            root = ET.fromstring(xml_text)
        except ET.ParseError:
            return results

        for idx, entry in enumerate(root.findall("atom:entry", ARXIV_NS)):
            title_el = entry.find("atom:title", ARXIV_NS)
            summary_el = entry.find("atom:summary", ARXIV_NS)
            id_el = entry.find("atom:id", ARXIV_NS)
            published_el = entry.find("atom:published", ARXIV_NS)

            title = title_el.text.strip() if title_el is not None and title_el.text else ""
            summary = summary_el.text.strip() if summary_el is not None and summary_el.text else ""
            paper_id = id_el.text.strip() if id_el is not None and id_el.text else ""

            # arXiv ID is the last segment of the <id> URL
            arxiv_id = paper_id.rsplit("/", 1)[-1] if "/" in paper_id else paper_id
            url = f"https://arxiv.org/abs/{arxiv_id}"

            # Clean summary: arXiv wraps newlines inside paragraphs
            abstract = re.sub(r"\s+", " ", summary).strip()
            clean = abstract
            if len(clean) > 300:
                clean = clean[:300] + "…"

            published_date = published_el.text.strip() if published_el is not None and published_el.text else None

            payload = build_payload(
                DOMAIN_SCIENCE,
                "publication",
                {
                    "publication_id": arxiv_id or None,
                    "abstract": abstract or None,
                },
                engine=self.name,
            )

            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=clean,
                    engine=self.name,
                    position=idx + 1,
                    published_date=published_date,
                    payload=payload,
                ),
            )

        return results
