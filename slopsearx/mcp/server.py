"""FastMCP server assembly for SlopSearX.

Builds the :class:`~mcp.server.fastmcp.FastMCP` instance, wires the
shared runtime through the lifespan (same :mod:`slopsearx.service`
pipeline the HTTP server uses), and registers the tools, resources,
and prompts defined in this package.

Transport: stdio by default (``MCP_TRANSPORT`` unset). Set
``MCP_TRANSPORT=http`` (plus ``MCP_HOST``/``MCP_PORT``) for the
streamable-HTTP transport; if ``mcp.auth_token``/``MCP_AUTH_TOKEN`` is
set, the HTTP transport requires ``Authorization: Bearer <token>``.
"""

from __future__ import annotations

import argparse
import asyncio
import functools
import logging
import os
import time
from contextlib import asynccontextmanager
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as _pkg_version
from typing import Any, AsyncIterator, Awaitable, Callable

import uvicorn
from mcp.server.fastmcp import FastMCP

import engines  # noqa: F401 — triggers @register_engine to populate the registry
from slopsearx import metrics as m
from slopsearx.capabilities import (
    CapabilityCatalog,
    load_mcp_policy,
    validate_intent_profiles,
)
from slopsearx.config import load_config
from slopsearx.mcp import prompts as _prompts
from slopsearx.mcp import resources as _resources
from slopsearx.mcp import tools as _tools
from slopsearx.mcp.gateway import create_gateway
from slopsearx.mcp.oauth import oauth_settings_from_policy
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService, build_context, destroy_context
from slopsearx.snapshot import SnapshotStore

logger = logging.getLogger(__name__)

_INSTRUCTIONS = """SlopSearX is a meta-search engine: it fans a query out to
many specialized engines and merges the results.

How to search correctly:
- Prefer intent-based search (slopsearx_search) over explicit engines. Use
  slopsearx_explain_search_scope first to preview routing without spending
  rate limits.
- Use slopsearx_search_targeted only when you need a specific evidence
  boundary; unknown engines are rejected with valid alternatives.
- Results are search findings, not verified facts. Slopsearx never fetches
  or verifies full page bodies — treat snippets as leads and cite with the
  citation.url. The score is a cross-engine presence signal
  (tier_then_cross_engine_presence), not relevance confidence.
- Partial results are normal: check engine_outcomes for which sources
  answered and which failed. Absence from a source does not prove absence
  of the thing you searched for.
- Pagination: use the cursor from a search with slopsearx_read_results;
  pages come from a captured snapshot and never re-run the query.
- Specialist tools (jobs, security, science, research) are disabled until
  the operator grants them (MCP_GRANT_JOBS / MCP_GRANT_SECURITY /
  MCP_GRANT_SCIENCE / MCP_GRANT_RESEARCH).
- Capabilities, routing profiles, and health are available as resources
  (slopsearx://capabilities, slopsearx://routing-profiles,
  slopsearx://health/summary) — read them instead of guessing engine names.
"""


def _package_version() -> str:
    try:
        return _pkg_version("slopsearx")
    except PackageNotFoundError:
        return "0.0.0"


@asynccontextmanager
async def _lifespan(
    server: FastMCP,
    oauth_provider: Any = None,
    state_factory: Callable[[], Awaitable[AppContext]] | None = None,
) -> AsyncIterator[McpState]:
    """Build and tear down the shared runtime for one server session.

    ``state_factory`` is an injectable override for the runtime wiring. It
    returns an :class:`AppContext`; when omitted, ``build_context()`` wires
    the live engines and Valkey-backed cache. The fixture harness
    (``slopsearx.mcp.harness``) uses it to inject deterministic fake engines
    and an in-memory cache/snapshot/job store so the real MCP server can be
    driven over streamable HTTP without a live network or Valkey.
    """
    del server
    ctx = await (state_factory() if state_factory is not None else build_context())
    if oauth_provider is not None:
        # Bind the OAuth token store to the shared Valkey cache so tokens
        # and registrations survive across replicas.
        oauth_provider.bind(ctx.cache)
    policy = load_mcp_policy()
    cfg = load_config()
    catalog = CapabilityCatalog(
        config=cfg,
        adapters=ctx.active_engines,
        required_key_engines=policy.required_key_engines,
    )
    for problem in validate_intent_profiles(catalog) + policy.validate(catalog):
        logger.warning("MCP startup problem: %s", problem)

    service = SearchService(ctx)
    snapshots = SnapshotStore(ctx.cache, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(ctx.cache)
    expired = await job_store.expire_stale_running()
    if expired:
        logger.warning("MCP startup: expired %d stale running research job(s)", expired)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    state = McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version=_package_version(),
    )
    set_state(state)
    runner_task = asyncio.create_task(runner.run_forever())
    try:
        yield state
    finally:
        runner_task.cancel()
        try:
            await runner_task
        except asyncio.CancelledError:
            pass
        set_state(None)
        await destroy_context(ctx)


def _instrumented(fn: Any) -> Any:
    """Wrap a tool with per-tool metrics (calls, latency, structured errors)."""

    @functools.wraps(fn)
    async def wrapper(*args: Any, **kwargs: Any) -> Any:
        tool = fn.__name__
        t0 = time.monotonic()
        m.mcp_tool_calls.inc({"tool": tool})
        try:
            result = await fn(*args, **kwargs)
        except Exception:
            m.mcp_tool_errors.inc({"tool": tool, "code": "exception"})
            raise
        finally:
            m.mcp_tool_latency.observe({"tool": tool}, time.monotonic() - t0)
        if isinstance(result, dict) and "error" in result:
            code = str(result["error"].get("code", "unknown"))
            m.mcp_tool_errors.inc({"tool": tool, "code": code})
        return result

    return wrapper


def create_server(
    host: str = "127.0.0.1",
    port: int = 8000,
    *,
    oauth: Any = None,
    oauth_provider: Any = None,
    state_factory: Callable[[], Awaitable[AppContext]] | None = None,
) -> FastMCP:
    """Build the configured FastMCP server with all tools/resources/prompts.

    When ``oauth`` (AuthSettings) is provided, the server runs in OAuth
    2.1 mode with dynamic client registration instead of static bearer
    tokens; ``oauth_provider`` must accompany it. See slopsearx.mcp.oauth.

    ``state_factory`` overrides the runtime wiring with a caller-supplied
    :class:`AppContext` factory (used by the fixture harness); when omitted
    the server wires the live engines and Valkey-backed cache itself.
    """
    kwargs: dict[str, Any] = {}
    if oauth is not None:
        if oauth_provider is None:
            raise ValueError("oauth_provider is required when oauth settings are provided")
        kwargs["auth"] = oauth
        kwargs["auth_server_provider"] = oauth_provider

    mcp = FastMCP(
        "slopsearx",
        instructions=_INSTRUCTIONS,
        lifespan=lambda server: _lifespan(server, oauth_provider=oauth_provider, state_factory=state_factory),
        host=host,
        port=port,
        **kwargs,
    )

    # --- tools ---------------------------------------------------------
    mcp.tool()(_instrumented(_tools.slopsearx_search))
    mcp.tool()(_instrumented(_tools.slopsearx_search_targeted))
    mcp.tool()(_instrumented(_tools.slopsearx_search_jobs))
    mcp.tool()(_instrumented(_tools.slopsearx_search_security))
    mcp.tool()(_instrumented(_tools.slopsearx_search_science))
    mcp.tool()(_instrumented(_tools.slopsearx_list_capabilities))
    mcp.tool()(_instrumented(_tools.slopsearx_explain_search_scope))
    mcp.tool()(_instrumented(_tools.slopsearx_get_service_status))
    mcp.tool()(_instrumented(_tools.slopsearx_read_results))
    mcp.tool()(_instrumented(_tools.slopsearx_read_result))
    mcp.tool()(_instrumented(_tools.slopsearx_start_research))
    mcp.tool()(_instrumented(_tools.slopsearx_get_job))
    mcp.tool()(_instrumented(_tools.slopsearx_cancel_job))
    mcp.tool()(_instrumented(_tools.slopsearx_retry_research))
    mcp.tool()(_instrumented(_tools.slopsearx_extend_research))

    # --- resources ------------------------------------------------------
    mcp.resource(
        "slopsearx://capabilities",
        title="Engine catalog",
        description="Current generated engine and category catalog, redacted",
    )(_resources.render_capabilities)
    mcp.resource(
        "slopsearx://capabilities/{engine}",
        title="Engine capability",
        description="One engine's metadata, caveats, and auth class",
    )(_resources.render_engine_capability)
    mcp.resource(
        "slopsearx://routing-profiles",
        title="Routing profiles",
        description="Intent-to-category/profile definitions and their provenance",
    )(_resources.render_routing_profiles)
    mcp.resource(
        "slopsearx://health/summary",
        title="Health summary",
        description="Current server health with the active-health limitation stated",
    )(_resources.render_health_summary)

    # --- prompts ---------------------------------------------------------
    mcp.prompt()(_prompts.research_with_source_coverage)
    mcp.prompt()(_prompts.investigate_vulnerability)
    mcp.prompt()(_prompts.find_company_jobs)
    mcp.prompt()(_prompts.compare_package_or_project)

    return mcp


def _env_flag(name: str) -> bool:
    """Parse an on/off environment variable (true/1/yes)."""
    return os.environ.get(name, "").strip().lower() in ("true", "1", "yes")


def main(argv: list[str] | None = None) -> None:
    """CLI entry point.

    Modes:
    - ``--remote <url>`` — stdio gateway that proxies to a remote SlopSearX
      MCP server over streamable HTTP (use on the agent's host).
    - ``MCP_TRANSPORT=http`` — serve streamable HTTP (use on the SlopSearX
      host so remote clients/gateways can connect).
    - default — local stdio server with the embedded pipeline.
    """
    parser = argparse.ArgumentParser(
        prog="slopsearx-mcp",
        description="SlopSearX MCP server: local stdio server, HTTP serve mode, "
        "or a stdio gateway to a remote SlopSearX server.",
    )
    parser.add_argument(
        "--remote",
        default=os.environ.get("MCP_REMOTE_URL", ""),
        help="Remote SlopSearX MCP server URL (streamable HTTP); runs as a stdio gateway to it",
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("MCP_REMOTE_TOKEN", ""),
        help="Bearer token sent to the remote server (client credential; prefer MCP_REMOTE_TOKEN env)",
    )
    parser.add_argument(
        "--oauth",
        action="store_true",
        default=_env_flag("MCP_REMOTE_OAUTH"),
        help="Connect to the remote server via the MCP OAuth 2.1 client flow "
        "(for remotes in OAuth mode; mutually exclusive with --token)",
    )
    parser.add_argument(
        "--oauth-callback-port",
        type=int,
        default=int(os.environ.get("MCP_REMOTE_OAUTH_CALLBACK_PORT", "8765")),
        help="Loopback port that receives the OAuth redirect (default 8765)",
    )
    parser.add_argument(
        "--oauth-no-browser",
        action="store_true",
        default=_env_flag("MCP_REMOTE_OAUTH_NO_BROWSER"),
        help="Print the authorize URL instead of opening a browser",
    )
    parser.add_argument(
        "--oauth-token-file",
        default=os.environ.get("MCP_REMOTE_TOKEN_FILE", ""),
        help="Persist OAuth client/token state to this file instead of the default config path",
    )
    parser.add_argument(
        "--transport",
        default=os.environ.get("MCP_TRANSPORT", "stdio").strip().lower(),
        help="stdio or http (serve mode)",
    )
    args = parser.parse_args(argv)

    if args.remote:
        remote_url = args.remote.strip()
        token = args.token.strip() or None
        if args.oauth and token:
            parser.error("--oauth and --token are mutually exclusive")
        logger.info(
            "Remote gateway mode: proxying to %s over stdio (%s)",
            remote_url,
            "OAuth 2.1" if args.oauth else "static bearer token",
        )
        gateway = create_gateway(
            remote_url,
            token=token,
            oauth=args.oauth,
            oauth_callback_port=args.oauth_callback_port,
            oauth_no_browser=args.oauth_no_browser,
            oauth_token_file=args.oauth_token_file or None,
        )
        gateway.run(transport="stdio")
        return

    host = os.environ.get("MCP_HOST", "127.0.0.1")
    try:
        port = int(os.environ.get("MCP_PORT", "8000"))
    except ValueError:
        port = 8000

    if args.transport == "http":
        policy = load_mcp_policy()
        oauth_settings, oauth_provider = oauth_settings_from_policy(policy)
        if oauth_settings is not None:
            logger.info("MCP HTTP transport enabled with OAuth 2.1 authorization")
            server = create_server(
                host=host,
                port=port,
                oauth=oauth_settings,
                oauth_provider=oauth_provider,
            )
            # OAuth mode replaces static-token mode; FastMCP's auth layer
            # protects /mcp (via the provider's token verifier).
            app = make_http_app(server, "")
        else:
            server = create_server(host=host, port=port)
            app = make_http_app(server, policy.auth_token)
            if policy.auth_token:
                logger.info("MCP HTTP transport enabled with bearer-token authentication")
            elif not policy.oauth_enabled:
                logger.warning(
                    "MCP HTTP transport has no authentication configured — set MCP_AUTH_TOKEN or enable mcp.oauth"
                )
        uvicorn.run(app, host=host, port=port, log_level=os.environ.get("MCP_LOG_LEVEL", "info"))
        return

    if args.transport not in ("stdio", "sse"):
        logger.warning("Unknown MCP_TRANSPORT %r; falling back to stdio", args.transport)
        args.transport = "stdio"
    server = create_server()
    server.run(transport=args.transport)


if __name__ == "__main__":
    main()
