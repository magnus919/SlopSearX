"""MCP server state — shared wiring for tool/resource/prompt handlers.

FastMCP registers handlers as module-level callables; the lifespan
builds the runtime once and hands it to handlers through this holder.
A stdio MCP server serves a single session, so process-wide state is
correct for that transport. For HTTP (especially behind a load balancer)
the tenant identity is derived per-request from the authenticated client
context rather than this process-wide holder — see :func:`current_tenant`.
"""

from __future__ import annotations

from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Any, Iterator

from slopsearx.capabilities import CapabilityCatalog, MCPPolicy
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore

# Request-scoped tenant override. Used by tests and by transports that set
# an explicit tenant per request; production HTTP/OAuth derives the tenant
# from the authenticated access token instead (see current_tenant()).
_tenant_override: ContextVar[str | None] = ContextVar("slopsearx_tenant_override", default=None)


@dataclass
class McpState:
    """Everything an MCP handler needs at call time."""

    ctx: AppContext
    policy: MCPPolicy
    catalog: CapabilityCatalog
    service: SearchService
    snapshots: SnapshotStore
    job_store: ResearchJobStore
    runner: ResearchJobRunner
    version: str


_state: McpState | None = None


def get_state() -> McpState:
    """Return the live MCP state, or raise if not initialized."""
    if _state is None:
        raise RuntimeError("MCP server state is not initialized")
    return _state


def set_state(state: McpState | None) -> None:
    """Install or clear the MCP state (called by the lifespan)."""
    global _state  # noqa: PLW0603
    _state = state


@contextmanager
def tenant_scope(tenant: str) -> Iterator[None]:
    """Temporarily pin the request-scoped tenant (test/deterministic seam)."""
    token = _tenant_override.set(tenant)
    try:
        yield
    finally:
        _tenant_override.reset(token)


def _access_token() -> Any:
    """Return the authenticated OAuth access token for the current request."""
    try:
        from mcp.server.auth.middleware.auth_context import get_access_token
    except Exception:  # noqa: BLE001 — stdio/static-token transports lack it
        return None
    return get_access_token()


def current_tenant() -> str:
    """Return the request-scoped tenant identity.

    Precedence: an explicit per-request override (tests/transports), then
    the authenticated OAuth client id (load-balanced HTTP deployments),
    then the single-tenant default. This never reads process-global mutable
    state, so concurrent requests with different authenticated clients do
    not bleed tenant identity across each other.
    """
    override = _tenant_override.get()
    if override is not None:
        return override
    token = _access_token()
    if token is not None:
        client_id = getattr(token, "client_id", None)
        if client_id:
            return str(client_id)
    return "default"
