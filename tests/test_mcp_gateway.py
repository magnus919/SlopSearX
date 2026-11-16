"""Tests for the remote gateway mode (stdio MCP server proxying to a remote SlopSearX server).

Spins up a real SlopSearX MCP server over streamable HTTP (with bearer-token
auth) in-process, then connects a gateway through the MCP session and
verifies tools, resources, and prompts are proxied faithfully.
"""

from __future__ import annotations

import asyncio
import socket
import time
from contextlib import asynccontextmanager, suppress
from typing import Any

try:
    import httpx2 as httpx
except ImportError:
    import httpx
import pytest
import uvicorn
from fastmcp import Client, FastMCP

from slopsearx.mcp.gateway import _field, _make_proxy, _register_proxy, create_gateway
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server

TOKEN = "sekret"


def _free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


async def _wait_for_port(port: int, timeout: float = 20.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            async with httpx.AsyncClient(timeout=2.0) as client:
                response = await client.get(f"http://127.0.0.1:{port}/mcp")
                if response.status_code == 401:
                    return True  # auth enforced → server is up
        except Exception:
            pass
        await asyncio.sleep(0.2)
    return False


@asynccontextmanager
async def _gateway_client(gateway: FastMCP, *, check_shutdown: bool = False):
    """Exercise the gateway across its HTTP boundary, with its real lifespan."""
    port = _free_port()
    server = uvicorn.Server(
        uvicorn.Config(make_http_app(gateway, ""), host="127.0.0.1", port=port, log_level="warning")
    )
    task = asyncio.create_task(server.serve())
    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if task.done():
                raise RuntimeError("gateway startup failed")
            try:
                async with httpx.AsyncClient(timeout=1) as probe:
                    await probe.get(f"http://127.0.0.1:{port}/mcp")
                break
            except httpx.ConnectError:
                await asyncio.sleep(0.1)
        else:
            raise RuntimeError("gateway did not start")
        async with Client(f"http://127.0.0.1:{port}/mcp") as client:
            yield client
    finally:
        server.should_exit = True
        # Uvicorn exits with SystemExit when the gateway's startup lifespan
        # rejects bad credentials; the test asserts that startup failure.
        with suppress(SystemExit):
            await asyncio.wait_for(task, timeout=10)
        if check_shutdown:
            assert not server.lifespan.shutdown_failed


@pytest.fixture
async def remote_server() -> Any:
    """A real SlopSearX MCP server over streamable HTTP with bearer auth."""
    port = _free_port()
    server = create_server(host="127.0.0.1", port=port)
    app = make_http_app(server, TOKEN)
    uvicorn_server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=port, log_level="warning"))
    task = asyncio.create_task(uvicorn_server.serve())
    assert await _wait_for_port(port), "remote server did not start"
    yield port
    uvicorn_server.should_exit = True
    try:
        await asyncio.wait_for(task, timeout=10)
    except asyncio.TimeoutError:
        task.cancel()


class TestGateway:
    async def test_dynamic_proxy_registration_preserves_remote_schema(self) -> None:
        server = FastMCP("gateway-test")
        proxy = _make_proxy(
            "remote_tool",
            {
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
            },
        )

        _register_proxy(server, proxy, "remote_tool", "Remote description")

        tool = await server.get_tool("remote_tool")
        assert tool.name == "remote_tool"
        assert tool.description == "Remote description"
        assert tool.parameters["properties"]["query"] == {"type": "string"}
        assert tool.parameters["required"] == ["query"]

    async def test_gateway_proxies_tools_and_resources(self, remote_server: Any) -> None:
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token=TOKEN)
        async with Client(url, auth=TOKEN) as direct:
            remote_tools = {
                tool.name: _field(tool, "input_schema", "inputSchema") for tool in await direct.list_tools()
            }

        async with _gateway_client(gateway) as client:
            tools = await client.list_tools()
            assert {tool.name: _field(tool, "input_schema", "inputSchema") for tool in tools} == remote_tools
            names = [tool.name for tool in tools]
            assert "slopsearx_search" in names
            assert "slopsearx_get_service_status" in names
            assert len(tools) == 38
            search = next(tool for tool in tools if tool.name == "slopsearx_search")
            schema = getattr(search, "input_schema", None) or getattr(search, "inputSchema")
            props = schema.get("properties", {})
            assert props["query"]["type"] == "string"
            assert "intent" in props
            assert "max_results" in props

            # Tool call is proxied to the remote server
            result = await client.call_tool_mcp("slopsearx_get_service_status", {})
            assert result.model_dump(by_alias=True)["isError"] is False
            import json

            payload = json.loads(result.content[0].text)
            assert payload["status"] == "ok"
            assert payload["active_engines"] > 0

            # Capability listing is proxied
            caps = await client.call_tool_mcp("slopsearx_list_capabilities", {"include_auth_requirements": False})
            caps_payload = json.loads(caps.content[0].text)
            assert caps_payload["count"] > 0
            assert "auth" not in caps_payload["engines"][0]

            # Resources are proxied
            resource = await client.read_resource("slopsearx://capabilities")
            assert "SlopSearX engine catalog" in resource[0].text

            engine_resource = await client.read_resource("slopsearx://capabilities/wikipedia")
            assert "wikipedia" in engine_resource[0].text

            # Prompts are proxied
            prompts = await client.list_prompts()
            assert len(prompts) == 5
            prompt = await client.get_prompt("research_with_source_coverage", {"question": "test"})
            assert prompt.messages
            planning_prompt = await client.get_prompt("plan_research_with_evidence", {"question": "test"})
            assert "slopsearx_plan_research_followup" in planning_prompt.messages[0].content.text
            # Unknown intent returns a structured error envelope unchanged.
            error_result = await client.call_tool_mcp("slopsearx_search", {"query": "x", "intent": "bogus"})
            error_payload = json.loads(error_result.content[0].text)
            assert error_payload["error"]["code"] == "invalid_input"
            assert "valid_alternatives" in error_payload["error"]

    async def test_gateway_rejects_wrong_token(self, remote_server: Any) -> None:
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token="wrong-token")

        async with httpx.AsyncClient() as probe:
            rejected = await probe.get(url, headers={"Authorization": "Bearer wrong-token"})
        assert rejected.status_code == 401
        # A wrong token must also fail gateway startup. Exercise the in-process
        # lifespan so expected startup rejection cannot kill a Uvicorn task.
        with pytest.raises(Exception):
            async with Client(gateway) as client:
                await client.list_tools()


@pytest.mark.parametrize(
    ("structured", "text", "error", "expected"),
    [
        ({"count": 1}, "One reading lead.", False, {"count": 1}),
        ({"count": 0}, '{"display": "summary"}', False, {"count": 0}),
        ({"count": 2}, None, False, {"count": 2}),
        ({}, "No reading leads.", False, {}),
        (None, '{"count": 1}', False, {"count": 1}),
        (None, "Unstructured note.", False, {"result": "Unstructured note."}),
        (None, None, False, {}),
        (
            {"ignored": "data"},
            "Remote rejected request.",
            True,
            {"error": {"code": "remote_error", "message": "Remote rejected request."}},
        ),
    ],
)
def test_gateway_structured_results_and_legacy_precedence(structured, text, error, expected):
    from mcp.types import CallToolResult, TextContent

    from slopsearx.mcp.gateway import _extract_tool_result

    result = CallToolResult.model_validate(
        {
            "content": [] if text is None else [TextContent(type="text", text=text)],
            "structuredContent": structured,
            "isError": error,
        }
    )
    assert _extract_tool_result(result) == expected


class TestCatalogPagination:
    @pytest.mark.parametrize("alias", ["nextCursor", "next_cursor"])
    async def test_follows_pages_including_empty_cursor(self, alias: str) -> None:
        from types import SimpleNamespace

        from slopsearx.mcp.gateway import _list_remote_tools

        class Session:
            calls: list[str | None] = []

            async def list_tools(self, *, params: Any = None) -> Any:
                cursor = None if params is None else params.cursor
                self.calls.append(cursor)
                pages = {
                    None: ([SimpleNamespace(name="first")], ""),
                    "": ([], "last"),
                    "last": ([SimpleNamespace(name="last")], None),
                }
                tools, following = pages[cursor]
                return SimpleNamespace(tools=tools, **{alias: following})

        session = Session()
        result = await _list_remote_tools(session)  # type: ignore[arg-type]
        assert [tool.name for tool in result] == ["first", "last"]
        assert session.calls == [None, "", "last"]

    @pytest.mark.parametrize("failure", ["cursor", "duplicate", "pages", "tools", "error"])
    async def test_broken_catalog_rejected(self, failure: str) -> None:
        from types import SimpleNamespace

        from slopsearx.mcp.gateway import _list_remote_tools

        class Session:
            count = 0

            async def list_tools(self, *, params: Any = None) -> Any:
                self.count += 1
                if failure == "error" and self.count == 2:
                    raise RuntimeError("upstream page failed")
                names = ["same"] if failure == "duplicate" else []
                if failure == "tools":
                    names = [f"tool_{i}" for i in range(4097)]
                cursor = "repeat" if failure in ("cursor", "duplicate") else str(self.count)
                return SimpleNamespace(tools=[SimpleNamespace(name=n) for n in names], nextCursor=cursor)

        session = Session()
        with pytest.raises(RuntimeError):
            await _list_remote_tools(session)  # type: ignore[arg-type]
        assert session.count <= 128

    async def test_empty_catalog(self) -> None:
        from types import SimpleNamespace

        from slopsearx.mcp.gateway import _list_remote_tools

        class Session:
            async def list_tools(self) -> Any:
                return SimpleNamespace(tools=[], nextCursor=None)

        assert await _list_remote_tools(Session()) == []  # type: ignore[arg-type]

    async def test_discovery_timeout_leaves_no_session_or_proxies(self, monkeypatch: Any) -> None:
        from slopsearx.mcp import gateway

        @asynccontextmanager
        async def streams(*args: Any, **kwargs: Any):
            yield (None, None)

        class Session:
            def __init__(self, *args: Any) -> None:
                pass

            async def __aenter__(self) -> Any:
                return self

            async def __aexit__(self, *args: Any) -> None:
                pass

            async def initialize(self) -> None:
                pass

            async def list_tools(self) -> Any:
                await asyncio.Event().wait()

        monkeypatch.setattr(gateway, "streamable_http_client", streams)
        monkeypatch.setattr(gateway, "ClientSession", Session)
        server = FastMCP("timeout")
        with pytest.raises(RuntimeError, match="cannot list tools"):
            async with gateway._gateway_lifespan(server, "https://example.invalid/mcp", None, 0.01):
                pytest.fail("incomplete discovery must not publish a ready gateway")
        assert gateway._state is None
        with pytest.raises(RuntimeError, match="not connected"):
            gateway._get_session()

    async def test_native_sdk_sends_followup_cursor(self, monkeypatch: Any) -> None:
        import anyio
        from mcp import ClientSession
        from mcp.types import ListToolsResult

        from slopsearx.mcp.gateway import _list_remote_tools

        write, read = anyio.create_memory_object_stream(1)
        session = ClientSession(read, write)
        requests: list[dict[str, Any]] = []

        async def send(request: Any, result_type: Any, **kwargs: Any) -> Any:
            payload = request.model_dump(mode="json", by_alias=True, exclude_none=True)
            requests.append(payload)
            return ListToolsResult.model_validate(
                {
                    "tools": [{"name": f"tool_{len(requests)}", "inputSchema": {"type": "object"}}],
                    "nextCursor": "opaque-next" if len(requests) == 1 else None,
                }
            )

        monkeypatch.setattr(session, "send_request", send)
        assert len(await _list_remote_tools(session)) == 2
        assert requests[1]["params"]["cursor"] == "opaque-next"
        await read.aclose()
        await write.aclose()


class TestInputSchemaFidelity:
    async def test_complete_schema_and_nested_copy(self) -> None:
        from copy import deepcopy

        schema = {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Exact query", "minLength": 1},
                "engines": {"type": "array", "items": {"type": "string", "enum": ["arxiv", "pubmed"]}},
                "filters": {
                    "anyOf": [
                        {
                            "type": "object",
                            "properties": {"limit": {"type": "integer", "minimum": 1}},
                            "additionalProperties": False,
                        },
                        {"type": "null"},
                    ],
                    "default": None,
                },
            },
            "required": ["query", "engines"],
            "additionalProperties": False,
        }
        expected = deepcopy(schema)
        server = FastMCP("schema-copy")
        _register_proxy(server, _make_proxy("remote", schema), "remote", "Description", schema)
        tool = await server.get_tool("remote")
        assert tool.parameters == expected
        assert schema == expected
        tool.parameters["properties"]["engines"]["items"]["enum"].append("changed")
        assert schema == expected

    async def test_optional_schema_keeps_direct_registration_compatible(self) -> None:
        server = FastMCP("old-helper-call")
        proxy = _make_proxy("simple", {"properties": {"query": {"type": "string"}}, "required": ["query"]})
        _register_proxy(server, proxy, "simple", "Description")
        tool = await server.get_tool("simple")
        assert tool.parameters["properties"]["query"]["type"] == "string"
        assert tool.parameters["required"] == ["query"]

    async def test_nested_values_forwarded_unchanged(self, monkeypatch: Any) -> None:
        import json

        from mcp.types import CallToolResult, TextContent

        from slopsearx.mcp import gateway

        schema = {
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "engines": {"type": "array", "items": {"type": "string"}},
                "filters": {"anyOf": [{"type": "object"}, {"type": "null"}], "default": None},
            },
            "required": ["query", "engines"],
        }
        calls: list[tuple[str, dict[str, Any]]] = []

        class Session:
            async def call_tool(self, name: str, arguments: dict[str, Any]) -> Any:
                calls.append((name, arguments))
                return CallToolResult(content=[TextContent(type="text", text=json.dumps(arguments))])

        monkeypatch.setattr(gateway, "_get_session", lambda: Session())
        server = FastMCP("schema-call")
        _register_proxy(server, _make_proxy("remote", schema), "remote", "Description", schema)
        arguments = {"query": "offline", "engines": ["arxiv"], "filters": {"limit": 2}}
        async with Client(server) as client:
            result = await client.call_tool_mcp("remote", arguments)
        assert calls == [("remote", arguments)]
        assert gateway._field(result, "structured_content", "structuredContent") == arguments


class TestGatewaySessionOwner:
    @pytest.mark.parametrize("exit_mode", ["normal", "body_error", "cancel"])
    async def test_connection_scopes_stay_in_owner_task(self, monkeypatch: Any, exit_mode: str) -> None:
        from slopsearx.mcp import gateway

        entered = asyncio.Event()
        exited = asyncio.Event()
        tasks = []

        @asynccontextmanager
        async def connection(*args: Any, **kwargs: Any):
            tasks.append(asyncio.current_task())
            try:
                yield
            finally:
                tasks.append(asyncio.current_task())
                exited.set()

        monkeypatch.setattr(gateway, "_gateway_connection", connection)

        async def hosting() -> None:
            async with gateway._gateway_lifespan(FastMCP("owner-test"), "unused", None, 1):
                entered.set()
                if exit_mode == "body_error":
                    raise ValueError("body failed")
                if exit_mode == "cancel":
                    await asyncio.Event().wait()

        host = asyncio.create_task(hosting())
        await asyncio.wait_for(entered.wait(), 1)
        if exit_mode == "cancel":
            host.cancel()
            with pytest.raises(asyncio.CancelledError):
                await host
        elif exit_mode == "body_error":
            with pytest.raises(ValueError, match="body failed"):
                await host
        else:
            await host
        assert exited.is_set()
        assert len(tasks) == 2
        assert tasks[0] is tasks[1] and tasks[0] is not host
        assert tasks[0].done()

    async def test_startup_failure_propagates_and_owner_finishes(self, monkeypatch: Any) -> None:
        from slopsearx.mcp import gateway

        owners = []

        @asynccontextmanager
        async def connection(*args: Any, **kwargs: Any):
            owners.append(asyncio.current_task())
            raise RuntimeError("startup failed")
            yield  # pragma: no cover

        monkeypatch.setattr(gateway, "_gateway_connection", connection)
        with pytest.raises(RuntimeError, match="startup failed"):
            async with gateway._gateway_lifespan(FastMCP("owner-test"), "unused", None, 1):
                pytest.fail("startup must fail closed")
        assert owners[0].done()

    async def test_cancelled_startup_closes_owner(self, monkeypatch: Any) -> None:
        from slopsearx.mcp import gateway

        entered = asyncio.Event()
        exited = asyncio.Event()
        owners = []

        @asynccontextmanager
        async def connection(*args: Any, **kwargs: Any):
            owners.append(asyncio.current_task())
            entered.set()
            try:
                await asyncio.Event().wait()
                yield
            finally:
                exited.set()

        monkeypatch.setattr(gateway, "_gateway_connection", connection)

        async def hosting() -> None:
            async with gateway._gateway_lifespan(FastMCP("owner-test"), "unused", None, 1):
                pytest.fail("cancelled startup cannot publish a session")

        host = asyncio.create_task(hosting())
        await asyncio.wait_for(entered.wait(), 1)
        host.cancel()
        with pytest.raises(asyncio.CancelledError):
            await host
        assert exited.is_set()
        assert owners[0].done()
