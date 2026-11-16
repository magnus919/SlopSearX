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

import asyncio
import inspect
import json
import logging
from contextlib import asynccontextmanager
from copy import deepcopy
from dataclasses import dataclass
from importlib import import_module
from typing import Any, AsyncIterator, Awaitable, Callable, cast

from fastmcp import FastMCP
from fastmcp.tools import FunctionTool
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client
from mcp.types import CallToolResult, PaginatedRequestParams

try:
    httpx: Any = import_module("httpx2")
except ModuleNotFoundError:  # MCP SDK v1 uses the standard httpx package.
    httpx = import_module("httpx")

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


def _field(value: Any, snake_name: str, legacy_name: str) -> Any:
    """Read an MCP model field across SDK v1 and SDK v2 attribute names."""
    result = getattr(value, snake_name, None)
    return result if result is not None else getattr(value, legacy_name, None)


async def _list_remote_tools(session: ClientSession) -> list[Any]:
    """Collect a complete, bounded catalog before publishing any proxies."""
    tools: list[Any] = []
    names: set[str] = set()
    cursors: set[str] = set()
    cursor: str | None = None
    for _ in range(128):
        page = (
            await session.list_tools()
            if cursor is None
            else await session.list_tools(params=PaginatedRequestParams(cursor=cursor))
        )
        for tool in page.tools:
            if tool.name in names:
                raise RuntimeError("remote tool catalog contains duplicate names")
            names.add(tool.name)
            tools.append(tool)
            if len(tools) > 4096:
                raise RuntimeError("remote tool catalog exceeds 4096 tools")
        cursor = _field(page, "next_cursor", "nextCursor")
        if cursor is None:
            return tools
        if cursor in cursors:
            raise RuntimeError("remote tool catalog repeats a pagination cursor")
        cursors.add(cursor)
    raise RuntimeError("remote tool catalog exceeds 128 pages")


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

    Prefer the remote structured object when supplied, including an empty
    object. Legacy text-only results retain their JSON/plain-text fallbacks;
    remote errors keep their existing precedence and normalized envelope.
    """
    texts: list[str] = []
    for block in result.content:
        text = getattr(block, "text", None)
        if text is None and isinstance(block, dict):
            text = block.get("text")
        if text:
            texts.append(str(text))
    joined = "\n".join(texts).strip()

    if _field(result, "is_error", "isError"):
        return {"error": {"code": "remote_error", "message": joined or "remote tool failed"}}
    structured = _field(result, "structured_content", "structuredContent")
    if structured is not None:
        return structured
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
    remote tool's input schema for invocation. Registration separately
    preserves the complete remote discovery schema.
    """
    properties = input_schema.get("properties") or {}
    required = set(input_schema.get("required") or [])

    async def proxy(**kwargs: Any) -> Any:
        result = await _get_session().call_tool(tool_name, kwargs)
        return _extract_tool_result(result)

    params: list[inspect.Parameter] = []
    annotations: dict[str, Any] = {}
    for name, schema in properties.items():
        default = inspect.Parameter.empty if name in required else schema.get("default", None)
        annotation = _annotation_for(schema.get("type"))
        params.append(
            inspect.Parameter(
                name,
                kind=inspect.Parameter.KEYWORD_ONLY,
                default=default,
                annotation=annotation,
            )
        )
        annotations[name] = annotation
    # mypy does not model dunder assignment on Callable; inspect honors
    # __signature__ at runtime to build the advertised tool schema.
    proxy.__signature__ = inspect.Signature(params)  # type: ignore[attr-defined]
    # FastMCP 3/4 uses Pydantic's TypeAdapter, which resolves annotations
    # independently of ``__signature__``.  Mirror the generated parameters
    # there so dynamic proxies validate and advertise the remote schema.
    annotations["return"] = Any
    proxy.__annotations__ = annotations
    proxy.__name__ = tool_name
    proxy.__qualname__ = tool_name
    return proxy


def _register_proxy(
    server: FastMCP,
    proxy: Callable[..., Awaitable[Any]],
    tool_name: str,
    description: str,
    input_schema: dict[str, Any] | None = None,
) -> None:
    """Register a proxy with both the bundled and standalone FastMCP APIs.

    FastMCP 3/4 derive the tool name and description from the callable and
    accept only that callable.  The older SDK-bundled FastMCP accepted those
    values as ``add_tool`` keyword arguments.  Keep the callable metadata as
    the source of truth so the advertised schema and wire name stay stable.
    """
    proxy.__name__ = tool_name
    proxy.__qualname__ = tool_name
    proxy.__doc__ = description
    add_tool_parameters = inspect.signature(server.add_tool).parameters
    if "name" in add_tool_parameters:
        server.add_tool(proxy, name=tool_name, description=description)  # type: ignore[call-arg]
    else:
        tool = FunctionTool.from_function(proxy)
        if input_schema is not None:
            # Python signatures cannot express all remote JSON Schema details.
            # Preserve discovery independently of the existing callable's
            # argument handling; upstream validation remains authoritative.
            tool.parameters = deepcopy(input_schema)
        server.add_tool(tool)


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


async def plan_research_with_evidence(question: str) -> str:
    result = await _get_session().get_prompt("plan_research_with_evidence", {"question": question})
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
    mcp.prompt()(plan_research_with_evidence)

    return mcp


@asynccontextmanager
async def _gateway_lifespan(
    server: FastMCP,
    remote_url: str,
    token: str | None,
    connect_timeout: float,
    **options: Any,
) -> AsyncIterator[None]:
    """Keep the upstream client's cancel scopes in one dedicated task.

    FastMCP enters other task groups after this lifespan yields. Its later
    cleanup therefore cannot safely unwind the MCP client's AnyIO scopes in
    the hosting task. The owner enters and exits all upstream scopes itself.
    """
    ready: asyncio.Future[None] = asyncio.get_running_loop().create_future()
    stop = asyncio.Event()

    async def own_connection() -> None:
        try:
            async with _gateway_connection(server, remote_url, token, connect_timeout, **options):
                ready.set_result(None)
                await stop.wait()
        except BaseException as exc:
            if not ready.done():
                ready.set_exception(exc)
            raise

    owner = asyncio.create_task(own_connection(), name="slopsearx-gateway-upstream")
    try:
        await asyncio.shield(ready)
        yield
    finally:
        stop.set()
        if not ready.done():
            ready.cancel()
            owner.cancel()
        await owner


@asynccontextmanager
async def _gateway_connection(
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
        async with streamable_http_client(remote_url, http_client=http_client) as streams:
            # MCP SDK v1 yields (read, write, session_id); v2 yields only
            # (read, write). The gateway does not use the session id.
            read, write = streams[0], streams[1]
            async with ClientSession(read, write) as session:
                try:
                    # Interactive OAuth runs inside initialize and has its own
                    # authorization budget; do not truncate the browser flow.
                    async with asyncio.timeout(None if oauth else connect_timeout):
                        await session.initialize()
                except Exception as exc:  # noqa: BLE001 — wrap into a clear startup error
                    raise RuntimeError(f"cannot connect to remote SlopSearX MCP server at {remote_url}: {exc}") from exc

                try:
                    async with asyncio.timeout(connect_timeout):
                        tools = await _list_remote_tools(session)
                except Exception as exc:  # noqa: BLE001
                    _state = None
                    raise RuntimeError(f"cannot list tools from remote server at {remote_url}: {exc}") from exc

                for tool in tools:
                    input_schema = _field(tool, "input_schema", "inputSchema") or {}
                    _register_proxy(
                        server, _make_proxy(tool.name, input_schema), tool.name, tool.description or "", input_schema
                    )
                logger.info(
                    "Remote gateway connected to %s (%d tools registered)",
                    remote_url,
                    len(tools),
                )

                _state = _GatewayState(session=session, remote_url=remote_url)
                try:
                    yield
                finally:
                    _state = None
