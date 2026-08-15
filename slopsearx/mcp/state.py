"""MCP server state — shared wiring for tool/resource/prompt handlers.

FastMCP registers handlers as module-level callables; the lifespan
builds the runtime once and hands it to handlers through this holder.
A stdio MCP server serves a single session, so process-wide state is
correct. (HTTP transport behind a load balancer would need per-request
tenant state — see docs/MCP_SERVER_DESIGN.md Phase 4.)
"""

from __future__ import annotations

from dataclasses import dataclass

from slopsearx.capabilities import CapabilityCatalog, MCPPolicy
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore


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
    tenant: str = "default"


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
