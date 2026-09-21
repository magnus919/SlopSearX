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

from slopsearx.mcp.gateway import _make_proxy, _register_proxy, create_gateway
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
async def _gateway_client(gateway: FastMCP):
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

        async with _gateway_client(gateway) as client:
            tools = await client.list_tools()
            names = [tool.name for tool in tools]
            assert "slopsearx_search" in names
            assert "slopsearx_get_service_status" in names
            assert len(tools) == 35
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
            assert len(prompts) == 4
            prompt = await client.get_prompt("research_with_source_coverage", {"question": "test"})
            assert prompt.messages
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
