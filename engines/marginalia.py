"""Marginalia Search adapter — independent web and blog discovery.

This adapter targets api2.marginalia-search.com with the API-Key header.
Per the official API page (Updated 2025-12-08): the shared `public` key
is a development path only and often hits rate limits; production requires
a registered key (free non-commercial via email, paid non-commercial $10,
or commercial metered). Both non-commercial keys carry CC-BY-NC-SA 4.0;
the commercial key removes attribution and usage restrictions.

See docs/MARGINALIA.md for licensing and operator configuration guidance.
"""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult, register_engine


@register_engine
class MarginaliaAdapter(EngineAdapter):
    """Marginalia Search — independent web and blog discovery."""

    name = "marginalia"
    display_name = "Marginalia Search"
    env_prefix = "ENGINE_MARGINALIA"
    engine_type = "api"
    # General web category for inclusion in unscoped searches;
    # web:independent enables an explicit discovery scope.
    categories = ["general", "web:independent"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "blocked", "error", "timeout")
    cost_class = "freemium"  # requires API key; free non-commercial tier available

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        started = time.monotonic()
        cfg = self.config
        api_key = cfg.get("api_key", "")

        # Fallback: load API key from environment variable if not in config
        if not api_key and self.env_prefix:
            env_key = f"{self.env_prefix}_API_KEY"
            api_key = api_key or (os.environ.get(env_key, "") or "")

        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = min(100, max(1, int(cfg.get("max_results", 10))))

        if not api_key:
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="Marginalia API key not configured",
            )

        base_url = cfg.get("base_url", "https://api2.marginalia-search.com/search")

        headers = {
            "Accept": "application/json",
            "API-Key": api_key,
        }

        params_dict: dict[str, Any] = {
            "query": query,
            "count": max_results,
        }

        start_time = time.monotonic()
        try:
            async with self.http_client(timeout=timeout_ms / 1_000.0) as client:
                resp = await client.get(base_url, headers=headers, params=params_dict)
                latency = (time.monotonic() - start_time) * 1_000

                if resp.status_code == 429 or resp.status_code == 503:
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 401 or resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)

                resp.raise_for_status()
                data = resp.json()

                # Parse results robustly against documented schema:
                # {"query": "...", "license": "...", "results": [{"url":"...", "title":"...", "description":"..."}]}
                results = self._parse_results(data.get("results", []))

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1_000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)

        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1_000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )

    def _parse_results(self, raw: list[dict[str, Any]]) -> list[SearchResult]:
        """Parse Marginalia result items into SearchResult objects."""
        results: list[SearchResult] = []
        for i, item in enumerate(raw):
            if not isinstance(item, dict):
                continue
            url = item.get("url", "")
            title = item.get("title", "")
            description = item.get("description", "") or ""
            if not url or not title:
                continue
            content = description[:500] if isinstance(description, str) else ""
            score = float(item.get("quality") or item.get("ranking") or 0.0)
            published_date = None
            if "published" in item:
                pub = item["published"]
                if isinstance(pub, str) and len(pub) >= 10:
                    try:
                        published_date = pub[:10]
                    except Exception:  # noqa: S110
                        pass
                elif isinstance(pub, int):
                    try:
                        published_date = time.strftime("%Y-%m-%d", time.localtime(pub))
                    except Exception:  # noqa: S110
                        pass
            domain = item.get("domain")
            media = None
            # Optional thumbnail field from upstream results
            if item.get("thumbnail"):
                media = self._build_media("image", url=item.get("thumbnail"))
            results.append(
                SearchResult(
                    url=url,
                    title=title,
                    content=content,
                    engine=self.name,
                    position=i + 1,
                    score=score,
                    published_date=published_date,
                    # Preserve source/domain provenance for attribution
                    payload={
                        "domain": domain,
                        "marginalia_license": item.get("license"),
                        "keywords": item.get("keywords") if isinstance(item.get("keywords"), list) else [],
                    },
                    media=media,
                ),
            )
        return results

    def _build_media(self, media_type: str, *, url: str | None = None, thumbnail: str | None = None) -> SearchResult.media:
        from slopsearx.adapter import build_media, sanitize_url

        if not url:
            return None
        sanitized = sanitize_url(url) if isinstance(url, str) else None
        thumb = sanitize_url(thumbnail) if isinstance(thumbnail, str) else None
        return build_media(media_type, url=sanitized, thumbnail=thumb, source=None)
