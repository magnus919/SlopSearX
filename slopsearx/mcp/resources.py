"""MCP resources — stable, inspectable context (design §6).

Rendered as plain text so agents can read them without spending a tool
call. Registered on the FastMCP instance by ``slopsearx.mcp.server``.
"""

from __future__ import annotations

from slopsearx.mcp.state import get_state
from slopsearx.research import STRATEGIES


def render_capabilities() -> str:
    """Full engine catalog, redacted (no API keys ever)."""
    try:
        state = get_state()
    except RuntimeError:
        return "SlopSearX MCP server is not initialized."
    lines = ["# SlopSearX engine catalog", ""]
    for cap in state.catalog.all():
        marker = "" if cap.enabled else " (disabled)"
        lines.append(f"## {cap.name}{marker} — {cap.display_name}")
        lines.append(f"- type: {cap.engine_type}")
        lines.append(f"- categories: {', '.join(cap.categories)}")
        lines.append(f"- auth class: {cap.auth_class}" + (", credentials configured" if cap.auth_configured else ""))
        lines.append(f"- scope hints: {', '.join(cap.scope_hints)}")
        if cap.caveats:
            lines.append(f"- caveats: {'; '.join(cap.caveats)}")
        lines.append("")
    return "\n".join(lines)


def render_engine_capability(engine: str) -> str:
    """One engine's metadata, caveats, and auth class."""
    try:
        state = get_state()
    except RuntimeError:
        return "SlopSearX MCP server is not initialized."
    cap = state.catalog.get(engine)
    if cap is None:
        valid = ", ".join(sorted(state.catalog.known_names()))
        return f"Unknown engine '{engine}'. Valid engines: {valid}"
    marker = "" if cap.enabled else " (disabled)"
    lines = [
        f"# {cap.name}{marker} — {cap.display_name}",
        f"- type: {cap.engine_type}",
        f"- categories: {', '.join(cap.categories)}",
        f"- auth class: {cap.auth_class}" + (", credentials configured" if cap.auth_configured else ""),
        f"- scope hints: {', '.join(cap.scope_hints)}",
    ]
    if cap.caveats:
        lines.append(f"- caveats: {'; '.join(cap.caveats)}")
    return "\n".join(lines)


def render_routing_profiles() -> str:
    """Intent → category/engine profile definitions and provenance."""
    from slopsearx.capabilities import INTENT_PROFILES

    lines = [
        "# SlopSearX routing profiles",
        "",
        "Intent profiles map an agent's stated intent to engine scopes. "
        "Profiles are validated against the live registry at startup; "
        "sensitive intents (security) require an operator grant.",
        "",
    ]
    for intent, profile in INTENT_PROFILES.items():
        lines.append(f"## {intent}")
        lines.append(f"- {profile.description}")
        if profile.categories:
            lines.append(f"- categories: {', '.join(profile.categories)}")
        if profile.engines:
            lines.append(f"- engines: {', '.join(profile.engines)}")
        if profile.sensitive:
            lines.append("- sensitive: requires MCP_GRANT_SECURITY=1")
        lines.append("")
    lines.append("# Research strategies")
    for strategy in STRATEGIES:
        lines.append(f"- {strategy}")
    return "\n".join(lines)


def render_health_summary() -> str:
    """Current server health with the active-health limitation stated."""
    try:
        state = get_state()
    except RuntimeError:
        return "SlopSearX MCP server is not initialized."
    ctx = state.ctx
    window = ctx.client_rate_window
    valkey = "unknown"
    fail_closed = "unknown"
    from slopsearx.ratelimit import ValkeySlidingWindow

    if isinstance(window, ValkeySlidingWindow):
        valkey = str(bool(window._connected)).lower()
        fail_closed = str(bool(window._fail_closed)).lower()
    lines = [
        "# SlopSearX service status",
        "- status: ok (liveness)",
        f"- version: {state.version}",
        f"- valkey connected: {valkey}",
        f"- valkey fail-closed: {fail_closed}",
        f"- cache connected: {bool(ctx.cache is not None and ctx.cache.is_connected)}",
        f"- snapshots available: {state.snapshots.available}",
        f"- active engines: {len(ctx.active_engines)}",
        "- engine health: /health does not actively probe external APIs; use search outcomes for passive engine health",
    ]
    return "\n".join(lines)
