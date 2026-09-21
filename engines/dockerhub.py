"""Docker Hub adapter — container image registry search.

Free, public JSON API. No auth required for anonymous search.
Docs: https://docs.docker.com/docker-hub/api/latest/
"""

from __future__ import annotations

import time
from typing import Any
from urllib.parse import quote

import httpx

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    register_engine,
)

_DOCKER_HUB_REGISTRIES = {"docker.io", "index.docker.io", "registry-1.docker.io"}
_DOCKER_HUB_MAX_PAGE_SIZE = 100
_MAX_CANDIDATE_OVERFETCH = 40


def _candidate_size(max_results: int) -> int:
    """Return a bounded Docker Hub window before presentation truncation."""
    return min(max_results + min(max_results * 2, _MAX_CANDIDATE_OVERFETCH), _DOCKER_HUB_MAX_PAGE_SIZE)


def _repository_scope(query: str) -> tuple[str, str]:
    """Return a safe Docker Hub namespace and repository-name filter."""
    value = query.strip()
    normalized = value.casefold()
    parts = [part for part in normalized.split("/") if part]
    if parts and parts[0] in _DOCKER_HUB_REGISTRIES:
        parts = parts[1:]

    if len(parts) == 1:
        namespace, repository = "library", parts[0]
    elif len(parts) == 2:
        namespace, repository = parts
    else:
        # Keep malformed or non-Docker-Hub references on the default scope;
        # the adapter remains non-raising and does not silently change their namespace.
        return "library", value

    if "@" in repository:
        repository = repository.split("@", 1)[0]
    repository = repository.split(":", 1)[0]
    if namespace == "library":
        return namespace, repository
    return namespace, repository


@register_engine
class DockerHubAdapter(EngineAdapter):
    """Docker Hub image search."""

    name = "dockerhub"
    display_name = "Docker Hub"
    env_prefix = "ENGINE_DOCKERHUB"
    engine_type = "api"
    categories = ["it", "reference", "packages"]

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
        namespace, repository_name = _repository_scope(query)
        base_url = cfg.get("base_url") or (
            f"https://hub.docker.com/v2/namespaces/{quote(namespace, safe='')}/repositories"
        )
        timeout_ms = cfg.get("timeout_ms", 5_000)
        max_results = cfg.get("max_results", 10)
        candidate_size = _candidate_size(max_results)

        headers = {
            "User-Agent": "SlopSearX/0.1.0 (meta search engine; agent-native)",
            "Accept": "application/json",
        }
        start_time = time.monotonic()

        try:
            async with self.http_client(timeout=timeout_ms / 1000.0) as client:
                resp = await client.get(
                    f"{base_url}",
                    params={"name": repository_name, "page_size": candidate_size},
                    headers=headers,
                )
                latency = (time.monotonic() - start_time) * 1000
                resp.raise_for_status()
                data = resp.json()

                repos = data.get("results", [])
                ordered_repos = sorted(
                    enumerate(repos),
                    key=lambda item: (
                        str(item[1].get("name", "")).strip().casefold() != repository_name,
                        str(item[1].get("namespace", namespace)).strip().casefold() != namespace,
                        item[0],
                    ),
                )

                results = []
                for idx, (_, repo) in enumerate(ordered_repos[:max_results]):
                    name = repo.get("name", "")
                    result_namespace = str(repo.get("namespace", namespace)).strip() or namespace
                    description = repo.get("description", "") or ""
                    pull_count = repo.get("pull_count", 0)
                    star_count = repo.get("star_count", 0)

                    content = description
                    if pull_count:
                        content += f" — Pulls: {pull_count:,}"

                    if result_namespace.casefold() == "library":
                        url = f"https://hub.docker.com/_/{name}"
                        title = f"{name}"
                    else:
                        url = f"https://hub.docker.com/r/{result_namespace}/{name}"
                        title = f"{result_namespace}/{name}"

                    results.append(
                        SearchResult(
                            url=url,
                            title=title,
                            content=content[:500],
                            engine=self.name,
                            position=idx + 1,
                            score=float(star_count) if star_count else float(pull_count) / 100000.0,
                        ),
                    )

                return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

        except httpx.TimeoutException:
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
        except httpx.HTTPStatusError as exc:
            latency = (time.monotonic() - start_time) * 1000
            if exc.response.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
        except Exception as exc:  # noqa: BLE001
            latency = (time.monotonic() - start_time) * 1000
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(exc),
                latency_ms=latency,
            )
