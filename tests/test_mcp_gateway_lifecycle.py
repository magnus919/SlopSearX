"""Run real gateway teardown in a fresh process, as for other transport probes."""

import asyncio
from typing import Any

import pytest
import uvicorn
from fastmcp import Client

from slopsearx.mcp.gateway import _field, create_gateway
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server
from tests.test_mcp_gateway import TOKEN, _free_port, _gateway_client, _wait_for_port


@pytest.fixture
async def offline_remote_server() -> Any:
    from slopsearx.mcp.harness import build_fixture_context

    async def state():
        return build_fixture_context()

    port = _free_port()
    server = create_server(state_factory=state)
    host = uvicorn.Server(
        uvicorn.Config(make_http_app(server, TOKEN), host="127.0.0.1", port=port, log_level="warning")
    )
    task = asyncio.create_task(host.serve())
    try:
        assert await _wait_for_port(port)
        yield port
    finally:
        host.should_exit = True
        await asyncio.wait_for(task, 10)


class TestGatewaySessionOwner:
    async def test_http_shutdown_is_clean(self, offline_remote_server: Any, caplog: Any) -> None:
        from slopsearx.mcp import gateway

        async with asyncio.timeout(15):
            url = f"http://127.0.0.1:{offline_remote_server}/mcp"
            controls = [
                ("slopsearx_list_capabilities", {"include_auth_requirements": False}),
                ("slopsearx_search", {"query": "offline", "intent": "not-real"}),
            ]
            async with Client(url, auth=TOKEN) as direct:
                assert len(await direct.list_tools()) == 38
                expected = [await direct.call_tool_mcp(name, args) for name, args in controls]
            async with _gateway_client(create_gateway(url, token=TOKEN), check_shutdown=True) as client:
                assert len(await client.list_tools()) == 38
                actual = [await client.call_tool_mcp(name, args) for name, args in controls]
                assert [_field(r, "structured_content", "structuredContent") for r in actual] == [
                    _field(r, "structured_content", "structuredContent") for r in expected
                ]
            assert gateway._state is None
            assert not any(task.get_name() == "slopsearx-gateway-upstream" for task in asyncio.all_tasks())
            assert "Application shutdown failed" not in caplog.text
            assert "Attempted to exit a cancel scope" not in caplog.text
