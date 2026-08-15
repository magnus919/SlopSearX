"""Remote gateway mode — a stdio MCP server that proxies to a remote SlopSearX server.

``slopsearx-mcp --remote <url>`` runs on the *agent's* host as an ordinary
stdio MCP server and holds a streamable-HTTP MCP client connection to a
SlopSearX MCP server running elsewhere. The agent host needs only this
package — no Valkey, no engine API keys, no search wiring. Every tool,
resource, and prompt is re-exposed from the remote server's live
capability list, so the gateway stays faithful even if the remote gains
tools.

Authentication to the remote server is either a static bearer token
(``--token`` / ``MCP_REMOTE_TOKEN``) or the standard MCP OAuth 2.1 client
flow (``--oauth``, see slopsearx.mcp.oauth_client) for remotes running in
OAuth mode.
"""

from __future__ import annotations

import inspect
import json
import logging
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any, AsyncIterator, Awaitable, Callable, cast

import httpx
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client
from mcp.server.fastmcp import FastMCP
from mcp.types import CallToolResult

logger = logging.getLogger(__name__)

_GATEWAY_INSTRUCTIONS = """SlopSearX remote gateway.

This process is a proxy: it forwards every tool call over a streamable HTTP
connection to a remote SlopSearX server. Behavior, results, warnings, and
limitations are identical to connecting to that server directly — see the
remote server's instructions. All server-side configuration (Valkey, engine
keys, grants) lives on the remote host, not here.
"""


# ---------------------------------------------------------------------------
# Connection state
# ---------------------------------------------------------------------------


@dataclass
class _GatewayState:
    session: ClientSession
    remote_url: str


_state: _GatewayState | None = None


def _get_session() -> ClientSession:
    """Return the live remote session, or raise if not connected."""
    if _state is None:
        raise RuntimeError("remote gateway is not connected")
    return _state.session


# ---------------------------------------------------------------------------
# Result conversion
# ---------------------------------------------------------------------------


def _extract_tool_result(result: CallToolResult) -> Any:
    """Recover the remote tool's structured result from MCP content blocks.

    SlopSearX tools return JSON-serializable dicts, which FastMCP serializes
    to JSON text content; parse it back so the gateway's own envelope is
    identical to the remote's.
    """
    texts: list[str] = []
    for block in result.content:
        text = getattr(block, "text", None)
        if text is None and isinstance(block, dict):
            text = block.get("text")
        if text:
            texts.append(str(text))
    joined = "\n".join(texts).strip()

    if result.isError:
        return {"error": {"code": "remote_error", "message": joined or "remote tool failed"}}
    if not joined:
        return {}
    try:
        return json.loads(joined)
    except json.JSONDecodeError:
        return {"result": joined}


def _extract_resource_text(result: Any) -> str:
    """Join text contents from a ReadResourceResult."""
    parts: list[str] = []
    for content in result.contents:
        text = getattr(content, "text", None)
        if text is None and isinstance(content, dict):
            text = content.get("text")
        if text is not None:
            parts.append(str(text))
    return "\n".join(parts)


def _extract_prompt_text(result: Any) -> str:
    """Join prompt message texts from a GetPromptResult."""
    parts: list[str] = []
    for message in result.messages:
        content = getattr(message, "content", None)
        text = getattr(content, "text", None)
        if text:
            parts.append(str(text))
    return "\n".join(parts)


# ---------------------------------------------------------------------------
# Dynamic tool proxies
# ---------------------------------------------------------------------------


def _annotation_for(json_type: Any) -> Any:
    """Map a JSON Schema type to a Python annotation for tool signatures."""
    mapping: dict[str, Any] = {
        "string": str,
        "integer": int,
        "number": float,
        "boolean": bool,
        "array": list,
        "object": dict,
    }
    if isinstance(json_type, list):
        for item in json_type:
            if item in mapping:
                return mapping[item]
        return Any
    if not json_type or json_type == "null":
        return Any
    return mapping.get(json_type, Any)


def _make_proxy(tool_name: str, input_schema: dict[str, Any]) -> Callable[..., Awaitable[Any]]:
    """Build a callable that forwards one tool call to the remote server.

    The returned function carries a ``__signature__`` derived from the
    remote tool's input schema, so the gateway's advertised tool schema
    matches the remote's.
    """
    properties = input_schema.get("properties") or {}
    required = set(input_schema.get("required") or [])

    async def proxy(**kwargs: Any) -> Any:
        result = await _get_session().call_tool(tool_name, kwargs)
        return _extract_tool_result(result)

    params: list[inspect.Parameter] = []
    for name, schema in properties.items():
        default = inspect.Parameter.empty if name in required else schema.get("default", None)
        params.append(
            inspect.Parameter(
                name,
                kind=inspect.Parameter.KEYWORD_ONLY,
                default=default,
                annotation=_annotation_for(schema.get("type")),
            )
        )
    # mypy does not model dunder assignment on Callable; inspect honors
    # __signature__ at runtime to build the advertised tool schema.
    proxy.__signature__ = inspect.Signature(params)  # type: ignore[attr-defined]
    proxy.__name__ = tool_name
    proxy.__qualname__ = tool_name
    return proxy


# ---------------------------------------------------------------------------
# Resources and prompts (forwarded to the remote server)
# ---------------------------------------------------------------------------


async def _read_remote_resource(uri: str) -> str:
    # The MCP client accepts str URIs at runtime; its stubs declare AnyUrl.
    result = await _get_session().read_resource(cast(Any, uri))
    return _extract_resource_text(result)


async def _capabilities_resource() -> str:
    return await _read_remote_resource("slopsearx://capabilities")


async def _engine_capability_resource(engine: str) -> str:
    return await _read_remote_resource(f"slopsearx://capabilities/{engine}")


async def _routing_profiles_resource() -> str:
    return await _read_remote_resource("slopsearx://routing-profiles")


async def _health_summary_resource() -> str:
    return await _read_remote_resource("slopsearx://health/summary")


async def research_with_source_coverage(question: str) -> str:
    result = await _get_session().get_prompt("research_with_source_coverage", {"question": question})
    return _extract_prompt_text(result)


async def investigate_vulnerability(target: str) -> str:
    result = await _get_session().get_prompt("investigate_vulnerability", {"target": target})
    return _extract_prompt_text(result)


async def find_company_jobs(company: str) -> str:
    result = await _get_session().get_prompt("find_company_jobs", {"company": company})
    return _extract_prompt_text(result)


async def compare_package_or_project(name: str) -> str:
    result = await _get_session().get_prompt("compare_package_or_project", {"name": name})
    return _extract_prompt_text(result)


# ---------------------------------------------------------------------------
# Server assembly
# ---------------------------------------------------------------------------


def create_gateway(
    remote_url: str,
    token: str | None = None,
    connect_timeout: float = 60.0,
    *,
    oauth: bool = False,
    oauth_callback_port: int = 8765,
    oauth_timeout: float = 300.0,
    oauth_no_browser: bool = False,
    oauth_token_file: str | None = None,
    oauth_redirect_handler: Callable[[str], Awaitable[None]] | None = None,
    oauth_callback_handler: Callable[[], Awaitable[tuple[str, str | None]]] | None = None,
) -> FastMCP:
    """Build a stdio MCP server that proxies to a remote SlopSearX MCP server.

    Args:
        remote_url: Streamable-HTTP endpoint of the remote server
            (e.g. ``http://10.0.0.5:8000/mcp``).
        token: Static bearer token sent to the remote server (mutually
            exclusive with ``oauth``).
        connect_timeout: Initial connection timeout in seconds.
        oauth: Run the standard MCP OAuth 2.1 client flow against the
            remote server instead of sending a static token (for remotes
            in OAuth mode — see slopsearx.mcp.oauth_client).
        oauth_callback_port: Loopback port that receives the OAuth redirect.
        oauth_timeout: Seconds to wait for the authorization callback.
        oauth_no_browser: Print the authorize URL instead of opening a browser.
        oauth_token_file: Where to persist OAuth client/token state.
        oauth_redirect_handler / oauth_callback_handler: Test/advanced hooks
            that replace the default browser + loopback-callback flow.
    """
    if oauth and token:
        raise ValueError("oauth and token are mutually exclusive for the remote connection")

    mcp = FastMCP(
        "slopsearx-remote",
        instructions=_GATEWAY_INSTRUCTIONS,
        lifespan=lambda server: _gateway_lifespan(
            server,
            remote_url,
            token,
            connect_timeout,
            oauth=oauth,
            oauth_callback_port=oauth_callback_port,
            oauth_timeout=oauth_timeout,
            oauth_no_browser=oauth_no_browser,
            oauth_token_file=oauth_token_file,
            oauth_redirect_handler=oauth_redirect_handler,
            oauth_callback_handler=oauth_callback_handler,
        ),
    )

    # Resources — forwarded to the remote server
    mcp.resource(
        "slopsearx://capabilities",
        title="Engine catalog (remote)",
        description="Current generated engine and category catalog, redacted",
    )(_capabilities_resource)
    mcp.resource(
        "slopsearx://capabilities/{engine}",
        title="Engine capability (remote)",
        description="One engine's metadata, caveats, and auth class",
    )(_engine_capability_resource)
    mcp.resource(
        "slopsearx://routing-profiles",
        title="Routing profiles (remote)",
        description="Intent-to-category/profile definitions and their provenance",
    )(_routing_profiles_resource)
    mcp.resource(
        "slopsearx://health/summary",
        title="Health summary (remote)",
        description="Current server health with the active-health limitation stated",
    )(_health_summary_resource)

    # Prompts — forwarded to the remote server
    mcp.prompt()(research_with_source_coverage)
    mcp.prompt()(investigate_vulnerability)
    mcp.prompt()(find_company_jobs)
    mcp.prompt()(compare_package_or_project)

    return mcp


@asynccontextmanager
async def _gateway_lifespan(
    server: FastMCP,
    remote_url: str,
    token: str | None,
    connect_timeout: float,
    *,
    oauth: bool = False,
    oauth_callback_port: int = 8765,
    oauth_timeout: float = 300.0,
    oauth_no_browser: bool = False,
    oauth_token_file: str | None = None,
    oauth_redirect_handler: Callable[[str], Awaitable[None]] | None = None,
    oauth_callback_handler: Callable[[], Awaitable[tuple[str, str | None]]] | None = None,
) -> AsyncIterator[None]:
    """Connect to the remote server and register its tools as proxies."""
    global _state  # noqa: PLW0603

    if oauth:
        from slopsearx.mcp.oauth_client import build_oauth_http_client

        http_client = build_oauth_http_client(
            remote_url,
            token_file=oauth_token_file,
            callback_port=oauth_callback_port,
            timeout=oauth_timeout,
            no_browser=oauth_no_browser,
            redirect_handler=oauth_redirect_handler,
            callback_handler=oauth_callback_handler,
        )
    else:
        headers = {"Authorization": f"Bearer {token}"} if token else None
        http_client = httpx.AsyncClient(headers=headers, timeout=httpx.Timeout(connect_timeout, read=300))

    async with http_client:
        async with streamable_http_client(remote_url, http_client=http_client) as (read, write, _):
            async with ClientSession(read, write) as session:
                try:
                    await session.initialize()
                except Exception as exc:  # noqa: BLE001 — wrap into a clear startup error
                    raise RuntimeError(f"cannot connect to remote SlopSearX MCP server at {remote_url}: {exc}") from exc

                _state = _GatewayState(session=session, remote_url=remote_url)
                try:
                    tools_result = await session.list_tools()
                except Exception as exc:  # noqa: BLE001
                    _state = None
                    raise RuntimeError(f"cannot list tools from remote server at {remote_url}: {exc}") from exc

                for tool in tools_result.tools:
                    server.add_tool(
                        _make_proxy(tool.name, tool.inputSchema),
                        name=tool.name,
                        description=tool.description or "",
                    )
                logger.info(
                    "Remote gateway connected to %s (%d tools registered)",
                    remote_url,
                    len(tools_result.tools),
                )

                try:
                    yield
                finally:
                    _state = None
