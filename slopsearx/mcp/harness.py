"""Deterministic transport-level fixture harness for the MCP server.

This module lets flow validators (and tests) drive the **real** MCP server
over streamable HTTP with deterministic inputs and **no live network / no
Valkey**:

- fake engines (injected ``EngineAdapter`` instances) return scripted,
  deterministic results instead of calling upstream APIs;
- an :class:`InMemoryStore` stands in for Valkey so cache, snapshots, and
  research jobs work locally (cursors/``result_id``/cache state resolve);

Both are injected through :func:`slopsearx.mcp.server.create_server`'s
``state_factory`` hook, so the transport, tools, resources, prompts, and
auth layer are the same production code the ordinary server uses. Only the
engine fan-out and shared store are swapped.

Usage (serve mode):
    MCP_HARNESS_PORT=8105 .venv/bin/python -m slopsearx.mcp.harness

Optional env knobs:
    MCP_HARNESS_HOST / MCP_HARNESS_PORT      where to bind (default 127.0.0.1:8105)
    MCP_HARNESS_TOKEN                        bearer token (sets MCP_AUTH_TOKEN semantics)
    MCP_HARNESS_ENGINES                      comma-separated fake engine names
    MCP_HARNESS_RESULTS                      deterministic result count per engine
    MCP_GRANT_* / MCP_TARGETED_SENSITIVE_ALLOWED   grant policy (as usual)
"""

from __future__ import annotations

import argparse
import asyncio
import os
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable, cast

import uvicorn

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import load_mcp_policy
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server
from slopsearx.router import QueryRouter
from slopsearx.service import AppContext

__all__ = [
    "DEFAULT_FAKE_ENGINES",
    "FakeEngine",
    "FakeEngineSpec",
    "InMemoryStore",
    "build_fake_engines",
    "build_fixture_context",
    "create_fixture_server",
    "make_fixture_http_app",
    "main",
]

DEFAULT_FAKE_ENGINES = ("wikipedia", "brave", "duckduckgo")


class InMemoryStore:
    """Minimal in-memory key/value store that stands in for Valkey.

    Implements the async ``get``/``set`` surface (plus ``is_connected`` and
    ``close``) that :class:`~slopsearx.service.SearchService`,
    :class:`~slopsearx.snapshot.SnapshotStore`, and
    :class:`~slopsearx.research.ResearchJobStore` depend on, so cache,
    snapshot, and research state work deterministically without a server.
    State is per-process and resets on restart (honest, non-durable).
    """

    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, Any] = {}
        self.set_ttls: list[int] = []

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        self._data[key] = value
        self.set_ttls.append(ttl)

    async def keys(self, pattern: str) -> list[str]:
        prefix = pattern.rstrip("*")
        return [key for key in self._data if key.startswith(prefix)]

    async def delete(self, key: str) -> None:
        self._data.pop(key, None)

    async def set_nx(self, key: str, value: dict[str, Any], ttl: int = 300) -> bool:
        if key in self._data:
            return False
        self._data[key] = value
        self.set_ttls.append(ttl)
        return True

    async def acquire_lease(self, key: str, token: str, ttl: int) -> bool:
        return await self.set_nx(key, {"token": token}, ttl)

    async def renew_lease(self, key: str, token: str, ttl: int) -> bool:
        current = self._data.get(key)
        if current is None or not isinstance(current, dict) or current.get("token") != token:
            return False
        self._data[key] = {"token": token}
        self.set_ttls.append(ttl)
        return True

    async def release_lease(self, key: str, token: str) -> bool:
        current = self._data.get(key)
        if current is None or not isinstance(current, dict) or current.get("token") != token:
            return False
        self._data.pop(key, None)
        return True

    async def close(self) -> None:
        # In-memory: nothing to release; kept so destroy_context works.
        return None


@dataclass
class FakeEngineSpec:
    """Scripted inputs for one fake engine."""

    name: str
    count: int = 3
    status: EngineStatus = EngineStatus.OK
    categories: list[str] = field(default_factory=lambda: ["general"])
    content: str | None = None
    engines: set[str] | None = None
    engine_type: str = "api"


class FakeEngine(EngineAdapter):
    """Deterministic engine adapter that never touches the network."""

    def __init__(self, spec: FakeEngineSpec) -> None:
        super().__init__()
        self.name = spec.name
        self.display_name = spec.name.title()
        self.env_prefix = f"ENGINE_{spec.name.upper()}"
        self.engine_type = spec.engine_type
        self.categories = list(spec.categories)
        self._count = spec.count
        self._status = spec.status
        self._content = spec.content
        self._engines = spec.engines
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(
                results=[],
                status=self._status,
                error_message="simulated engine failure",
                latency_ms=1.0,
            )
        engines = self._engines or {self.name}
        body = self._content or f"Content for {self.name} result "
        results = [
            SearchResult(
                url=f"https://{self.name}{i}.example",
                title=f"{self.name} result {i}",
                content=f"{body}{i}.",
                engine=self.name,
                engines=set(engines),
                score=float(self._count - i),
                position=i + 1,
                category=self.categories[0] if self.categories else "general",
                tier=1,
            )
            for i in range(self._count)
        ]
        return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=2.0)


def build_fake_engines(specs: list[FakeEngineSpec]) -> dict[str, EngineAdapter]:
    """Instantiate fake engines keyed by name."""
    return {spec.name: FakeEngine(spec) for spec in specs}


def build_fixture_context(
    specs: list[FakeEngineSpec] | None = None,
    *,
    store: InMemoryStore | None = None,
    router: QueryRouter | None = None,
    tier1_engines: set[str] | None = None,
    sensitive_engines: set[str] | None = None,
) -> AppContext:
    """Build an :class:`AppContext` wired to fake engines + an in-memory store.

    This is the ``state_factory`` payload for the fixture harness. It is a
    pure in-process construction: no live engines, no Valkey, no network.
    Sensitive engines default to the effective policy's sensitive set so the
    shared policy gate behaves identically to the production server.
    """
    specs = specs or [FakeEngineSpec(name=name) for name in DEFAULT_FAKE_ENGINES]
    store = store or InMemoryStore()
    active = build_fake_engines(specs)
    policy = load_mcp_policy()
    if sensitive_engines is None:
        sensitive_engines = set(policy.sensitive_engines)
    if tier1_engines is None:
        tier1_engines = set(active)
    return AppContext(
        active_engines=active,
        cache=cast(Any, store),
        rate_limiter=None,
        router=router,
        suggestion_service=None,
        stats_tracker=None,
        audit_logger=None,
        engine_semaphore=asyncio.Semaphore(10),
        client_rate_window=None,
        tier1_engines=tier1_engines,
        sensitive_engines=sensitive_engines,
    )


def create_fixture_server(
    specs: list[FakeEngineSpec] | None = None,
    *,
    store: InMemoryStore | None = None,
    router: QueryRouter | None = None,
    state_factory: Callable[[], Awaitable[AppContext]] | None = None,
    host: str = "127.0.0.1",
    port: int = 8105,
    oauth: Any = None,
    oauth_provider: Any = None,
) -> Any:
    """Build the real MCP server with the fixture context injected.

    Returns the :class:`FastMCP` instance (same production ``create_server``
    wiring) whose lifespan uses the fixture context instead of live engines
    and Valkey. The optional ``store`` is captured by the factory so it is
    shared across sessions (cross-session cursor stability).
    """

    def factory() -> Awaitable[AppContext]:
        return _build_ctx(specs, store=store, router=router, state_factory=state_factory)

    return create_server(
        host=host,
        port=port,
        oauth=oauth,
        oauth_provider=oauth_provider,
        state_factory=factory,
    )


async def _build_ctx(
    specs: list[FakeEngineSpec] | None,
    *,
    store: InMemoryStore | None,
    router: QueryRouter | None,
    state_factory: Callable[[], Awaitable[AppContext]] | None,
) -> AppContext:
    if state_factory is not None:
        return await state_factory()
    return build_fixture_context(specs, store=store, router=router)


def make_fixture_http_app(
    specs: list[FakeEngineSpec] | None = None,
    *,
    store: InMemoryStore | None = None,
    router: QueryRouter | None = None,
    token: str = "",
    host: str = "127.0.0.1",
    port: int = 8105,
) -> Any:
    """Build the streamable-HTTP ASGI app over the fixture-injected server.

    When ``token`` is non-empty, every HTTP request must carry
    ``Authorization: Bearer <token>`` (401 otherwise), matching production
    auth behavior.
    """
    server = create_fixture_server(specs, store=store, router=router, host=host, port=port)
    return make_http_app(server, token)


def main(argv: list[str] | None = None) -> None:
    """CLI entrypoint: serve the fixture harness over streamable HTTP.

    ``.venv/bin/python -m slopsearx.mcp.harness``
    """
    parser = argparse.ArgumentParser(
        prog="slopsearx-mcp-fixture",
        description="Deterministic MCP fixture harness: real streamable-HTTP "
        "server with fake engines and an in-memory cache/snapshot/job store.",
    )
    parser.add_argument("--host", default=os.environ.get("MCP_HARNESS_HOST", "127.0.0.1"))
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.environ.get("MCP_HARNESS_PORT", "8105")),
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("MCP_HARNESS_TOKEN", ""),
        help="Require Authorization: Bearer <token> on every HTTP request",
    )
    parser.add_argument(
        "--engines",
        default=os.environ.get("MCP_HARNESS_ENGINES", ",".join(DEFAULT_FAKE_ENGINES)),
        help="Comma-separated fake engine names to inject",
    )
    parser.add_argument(
        "--results",
        type=int,
        default=int(os.environ.get("MCP_HARNESS_RESULTS", "3")),
        help="Deterministic result count per fake engine",
    )
    args = parser.parse_args(argv)

    specs = [FakeEngineSpec(name=name.strip(), count=args.results) for name in args.engines.split(",") if name.strip()]
    app = make_fixture_http_app(specs, token=args.token, host=args.host, port=args.port)
    uvicorn.run(app, host=args.host, port=args.port, log_level=os.environ.get("MCP_LOG_LEVEL", "info"))


if __name__ == "__main__":
    main()
