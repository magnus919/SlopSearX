"""Tests for the remote gateway mode (stdio MCP server proxying to a remote SlopSearX server).

Spins up a real SlopSearX MCP server over streamable HTTP (with bearer-token
auth) in-process, then connects a gateway through the MCP session and
verifies tools, resources, and prompts are proxied faithfully.
"""

from __future__ import annotations

import asyncio
import socket
import time
from typing import Any

import httpx
import pytest
import uvicorn
from mcp.shared.memory import create_connected_server_and_client_session

from slopsearx.mcp.gateway import create_gateway
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
    async def test_gateway_proxies_tools_and_resources(self, remote_server: Any) -> None:
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token=TOKEN)

        async with create_connected_server_and_client_session(gateway) as client:
            tools = await client.list_tools()
            names = [tool.name for tool in tools.tools]
            assert "slopsearx_search" in names
            assert "slopsearx_get_service_status" in names
            assert len(tools.tools) == 15

            # Tool call is proxied to the remote server
            result = await client.call_tool("slopsearx_get_service_status", {})
            assert result.isError is False
            import json

            payload = json.loads(result.content[0].text)
            assert payload["status"] == "ok"
            assert payload["active_engines"] > 0

            # Capability listing is proxied
            caps = await client.call_tool("slopsearx_list_capabilities", {"include_auth_requirements": False})
            caps_payload = json.loads(caps.content[0].text)
            assert caps_payload["count"] > 0
            assert "auth" not in caps_payload["engines"][0]

            # Resources are proxied
            resource = await client.read_resource("slopsearx://capabilities")
            assert "SlopSearX engine catalog" in resource.contents[0].text

            engine_resource = await client.read_resource("slopsearx://capabilities/wikipedia")
            assert "wikipedia" in engine_resource.contents[0].text

            # Prompts are proxied
            prompts = await client.list_prompts()
            assert len(prompts.prompts) == 4
            prompt = await client.get_prompt("research_with_source_coverage", {"question": "test"})
            assert prompt.messages

    async def test_gateway_tool_schema_matches_remote(self, remote_server: Any) -> None:
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token=TOKEN)

        async with create_connected_server_and_client_session(gateway) as client:
            tools = await client.list_tools()
            search = next(tool for tool in tools.tools if tool.name == "slopsearx_search")
            props = search.inputSchema.get("properties", {})
            assert "query" in props
            assert props["query"]["type"] == "string"
            assert "intent" in props
            assert "max_results" in props

    async def test_gateway_rejects_wrong_token(self, remote_server: Any) -> None:
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token="wrong-token")

        # A wrong token must fail the connection (the remote returns 401)
        with pytest.raises(Exception):
            async with create_connected_server_and_client_session(gateway) as client:
                await client.list_tools()

    async def test_gateway_error_envelope_proxied(self, remote_server: Any) -> None:
        """Remote tool error envelopes arrive intact through the gateway."""
        url = f"http://127.0.0.1:{remote_server}/mcp"
        gateway = create_gateway(url, token=TOKEN)

        async with create_connected_server_and_client_session(gateway) as client:
            # Unknown intent returns a structured error envelope from the remote
            result = await client.call_tool("slopsearx_search", {"query": "x", "intent": "bogus"})
            import json

            payload = json.loads(result.content[0].text)
            assert payload["error"]["code"] == "invalid_input"
            assert "valid_alternatives" in payload["error"]
