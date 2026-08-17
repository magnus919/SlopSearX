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
        lines.append(f"- sensitive: {cap.sensitive}")
        lines.append(f"- supported filters: {_render_supported_filters(cap.supported_filters)}")
        lines.append(f"- supported result types: {', '.join(cap.supported_result_types)}")
        lines.append(f"- failure classes: {', '.join(cap.failure_classes)}")
        lines.append(f"- cost class: {cap.cost_class or 'unknown'}")
        lines.append(f"- last known status: {cap.last_known_status}")
        lines.append(f"- last known status at: {cap.last_known_status_at or 'never observed'}")
        lines.append(f"- stale: {cap.last_known_status_stale}")
        lines.append(f"- circuit open: {cap.circuit_open}")
        lines.append(f"- consecutive errors: {cap.circuit_consecutive_errors}")
        lines.append(f"- scope hints: {', '.join(cap.scope_hints)}")
        if cap.caveats:
            lines.append(f"- caveats: {'; '.join(cap.caveats)}")
        lines.append("")
    return "\n".join(lines)


def _render_supported_filters(supported: dict[str, bool]) -> str:
    """Render supported_filters as 'language, time_range' (only the supported)."""
    supported_keys = [key for key, value in supported.items() if value]
    return ", ".join(supported_keys) if supported_keys else "none"


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
        f"- sensitive: {cap.sensitive}",
        f"- supported filters: {_render_supported_filters(cap.supported_filters)}",
        f"- supported result types: {', '.join(cap.supported_result_types)}",
        f"- failure classes: {', '.join(cap.failure_classes)}",
        f"- cost class: {cap.cost_class or 'unknown'}",
        f"- last known status: {cap.last_known_status}",
        f"- last known status at: {cap.last_known_status_at or 'never observed'}",
        f"- stale: {cap.last_known_status_stale}",
        f"- circuit open: {cap.circuit_open}",
        f"- consecutive errors: {cap.circuit_consecutive_errors}",
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
    """Current server health with the active-health limitation stated.

    Rendered from the same shared diagnostics schema as
    ``slopsearx_get_service_status`` so the resource and the tool report
    identical shared values (VAL-DIAG-010). No credentials, raw audit,
    environment, or unrestricted metrics are ever included.
    """
    try:
        state = get_state()
    except RuntimeError:
        return "SlopSearX MCP server is not initialized."
    from slopsearx.mcp.tools import service_diagnostics

    diag = service_diagnostics(state)
    health = diag["engine_health"]
    bounds = diag["policy_bounds"]
    degradation = diag["degradation"]
    grants = diag["grants"]["enabled"]
    causes = ", ".join(degradation["causes"]) if degradation["causes"] else "none"
    lines = [
        "# SlopSearX service status",
        "- status: ok (liveness)",
        f"- version: {diag['version']}",
        f"- contract version: {diag['contract_version']}",
        f"- valkey connected: {diag['valkey']['connected']}",
        f"- valkey fail-closed: {diag['valkey']['fail_closed']}",
        f"- cache connected: {diag['cache_connected']}",
        f"- snapshots available: {diag['snapshots_available']}",
        f"- job store available: {diag['job_store_available']}",
        f"- active engines: {diag['active_engines']}",
        f"- router enabled: {diag['router_enabled']}",
        f"- grants enabled: {', '.join(grants) if grants else 'none'}",
        "- policy bounds: "
        f"max_query_length={bounds['max_query_length']}, max_results={bounds['max_results']}, "
        f"snapshot_ttl_seconds={bounds['snapshot_ttl_seconds']}, "
        f"job_max_queries={bounds['job_max_queries']}, "
        f"job_max_engines_per_query={bounds['job_max_engines_per_query']}, "
        f"job_max_results={bounds['job_max_results']}, "
        f"job_default_deadline_seconds={bounds['job_default_deadline_seconds']}",
        f"- degradation: {degradation['summary']} (causes: {causes})",
        f"- freshness: {diag['freshness']}",
        "- engine health: "
        f"ok={health['ok']}, rate_limited={health['rate_limited']}, blocked={health['blocked']}, "
        f"error={health['error']}, timeout={health['timeout']}, unknown={health['unknown']}",
        f"- engine health note: {health['note']}",
    ]
    return "\n".join(lines)
