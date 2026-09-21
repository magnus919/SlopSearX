"""GitHub API adapter — public code, repository, and issue/PR search."""

from __future__ import annotations

import time
from typing import Any

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)
from slopsearx.payload import build_payload


@register_engine
class GitHubAdapter(EngineAdapter):
    name = "github"
    display_name = "GitHub"
    env_prefix = "ENGINE_GITHUB"
    engine_type = "api"
    categories = ["reference", "github:code", "github:issues", "github:prs"]

    # -- Declared capability metadata (audited, issue 185) --
    supported_result_types = ("text",)
    failure_classes = ("rate_limited", "blocked", "error", "timeout", "unavailable")
    cost_class = "free"

    async def search(
        self,
        query: str,
        params: dict[str, Any] | None = None,
    ) -> AdapterResponse:
        if early := await self._check_rate_limit():
            return early

        cfg = self.config
        token = str(cfg.get("api_key") or "").strip()
        base_url = cfg.get("base_url", "https://api.github.com")
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 5)
        categories = (params or {}).get("categories", []) or ["general"]

        # Determine sub-mode from categories
        if "github:code" in categories:
            endpoint = f"{base_url}/search/code"
        elif "github:issues" in categories or "github:prs" in categories:
            endpoint = f"{base_url}/search/issues"
        else:
            endpoint = f"{base_url}/search/repositories"

        # GitHub's current search documentation requires authentication for
        # code search, while repository and issue/PR search support public
        # unauthenticated resources. Keep the stricter code-search boundary
        # until the provider contract is unambiguous.
        if "/search/code" in endpoint and not token:
            return AdapterResponse(
                results=[],
                status=EngineStatus.UNAVAILABLE,
                error_message="GitHub token required for code search (set ENGINE_GITHUB_API_KEY)",
            )

        headers = {
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "SlopSearX/0.1.0",
        }
        # A token is optional for public repository and issue/PR search. It
        # raises the rate limit and may add access to private resources.
        if token:
            headers["Authorization"] = f"Bearer {token}"

        params_dict: dict[str, Any] = {
            "q": query,
            "per_page": max_results,
            "page": 1,
        }

        start_time = time.monotonic()
        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(endpoint, headers=headers, params=params_dict)
                latency = (time.monotonic() - start_time) * 1000

                if resp.status_code == 403 and "rate limit" in (resp.text or "").lower():
                    return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
                if resp.status_code == 403:
                    return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
                if resp.status_code == 401:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.UNAVAILABLE,
                        error_message="GitHub authentication required for this request",
                        latency_ms=latency,
                    )
                if resp.status_code == 422:
                    return AdapterResponse(
                        results=[],
                        status=EngineStatus.ERROR,
                        error_message="GitHub rejected the search query (validation failed)",
                        latency_ms=latency,
                    )
                resp.raise_for_status()

                data = resp.json()
                items = data.get("items", [])
                results = self._parse_items(items, query, endpoint)
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

    def _parse_items(self, items: list[dict[str, Any]], query: str, endpoint: str) -> list[SearchResult]:
        """Parse GitHub API search results into SearchResult list."""
        results: list[SearchResult] = []

        is_code = "/search/code" in endpoint
        is_issues = "/search/issues" in endpoint

        for idx, item in enumerate(items):
            if is_code:
                # Code search results
                repo_name = (item.get("repository") or {}).get("full_name", "")
                path = item.get("path", "")
                url = f"https://github.com/{repo_name}/blob/main/{path}" if repo_name else item.get("html_url", "")
                title = f"{repo_name}: {path}" if repo_name else path
                content = item.get("text_matches", [{}])[0].get("fragment", "") if item.get("text_matches") else ""
                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=content[:300] if content else f"Code file in {repo_name}",
                        engine=self.name,
                        position=idx + 1,
                        category="code",
                        payload=build_payload(
                            "code",
                            "repository",
                            {"repository": repo_name or None, "path": path or None},
                            engine=self.name,
                        ),
                    ),
                )
            elif is_issues:
                # Issue/PR search results
                url = item.get("html_url", "")
                title = item.get("title", "")
                state = item.get("state", "")
                raw_labels = item.get("labels") or []
                labels = ", ".join(lbl["name"] for lbl in raw_labels if isinstance(lbl, dict))
                content = item.get("body", "") or ""
                clean = content.strip()[:300] if content else ""
                if labels:
                    clean = f"[{state}] [{labels}] {clean}" if clean else f"[{state}] [{labels}]"
                else:
                    clean = f"[{state}] {clean}" if clean else f"[{state}]"
                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=clean.strip(),
                        engine=self.name,
                        position=idx + 1,
                        category="issues",
                        published_date=item.get("created_at", ""),
                    ),
                )
            else:
                # Repository search results
                url = item.get("html_url", "")
                title = item.get("full_name", item.get("name", ""))
                desc = item.get("description") or ""
                lang = item.get("language") or ""
                stars = item.get("stargazers_count", 0)
                topics = ", ".join(item.get("topics", []) or [])
                detail_parts = [f"★ {stars}"]
                if lang:
                    detail_parts.append(lang)
                if topics:
                    detail_parts.append(topics)
                content = f"{desc} — {' | '.join(detail_parts)}" if desc else " — ".join(detail_parts)
                results.append(
                    SearchResult(
                        url=url,
                        title=title,
                        content=content,
                        engine=self.name,
                        position=idx + 1,
                        score=float(stars),
                        category="repositories",
                        payload=build_payload(
                            "code",
                            "repository",
                            {"repository": item.get("full_name") or None},
                            engine=self.name,
                        ),
                    ),
                )

        return results
