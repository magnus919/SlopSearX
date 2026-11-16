"""Exercise initialization deadlines in an isolated real HTTP process."""

import asyncio
from typing import Any

import pytest
import uvicorn
from fastmcp import FastMCP

from slopsearx.mcp import gateway
from slopsearx.mcp.harness import build_fixture_context
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server
from tests.test_mcp_gateway import TOKEN, _free_port, _wait_for_port


class DelayedInitialize:
    def __init__(self, app: Any) -> None:
        self.app = app
        self.delayed = False

    async def __call__(self, scope: Any, receive: Any, send: Any) -> None:
        # A new MCP session's first POST is initialize. Do not consume or
        # replace receive: streaming requests must retain their original body.
        if scope["type"] == "http" and scope["method"] == "POST" and not self.delayed:
            self.delayed = True
            await asyncio.sleep(1)
        await self.app(scope, receive, send)


def error_messages(exc: BaseException) -> list[str]:
    if isinstance(exc, BaseExceptionGroup):
        return [message for child in exc.exceptions for message in error_messages(child)]
    return [str(exc)]


async def test_initialization_deadline_fails_closed_and_cleans_owner() -> None:
    async def state():
        return build_fixture_context()

    port = _free_port()
    app = DelayedInitialize(make_http_app(create_server(state_factory=state), TOKEN))
    upstream = uvicorn.Server(
        uvicorn.Config(app, host="127.0.0.1", port=port, log_level="warning", timeout_graceful_shutdown=2)
    )
    task = asyncio.create_task(upstream.serve())
    try:
        assert await _wait_for_port(port)
        proxy = FastMCP("initialize-timeout")
        async with asyncio.timeout(2):
            with pytest.raises(Exception) as captured:
                async with gateway._gateway_lifespan(proxy, f"http://127.0.0.1:{port}/mcp", TOKEN, 0.05):
                    pytest.fail("a delayed initialize must not publish readiness")
        assert any(
            "cannot connect to remote SlopSearX MCP server" in message for message in error_messages(captured.value)
        )
        assert await proxy.list_tools() == []
        assert gateway._state is None
        assert not any(t.get_name() == "slopsearx-gateway-upstream" for t in asyncio.all_tasks())
        assert app.delayed
    finally:
        upstream.should_exit = True
        await asyncio.wait_for(task, 5)


async def test_oauth_initialization_retains_its_authorization_budget(monkeypatch: Any) -> None:
    from contextlib import asynccontextmanager
    from types import SimpleNamespace

    from slopsearx.mcp import oauth_client

    observed = {}

    class HttpClient:
        async def __aenter__(self):
            return self

        async def __aexit__(self, *args):
            pass

    def build_client(*args, **kwargs):
        observed.update(kwargs)
        return HttpClient()

    @asynccontextmanager
    async def streams(*args, **kwargs):
        yield (None, None)

    class Session:
        async def __aenter__(self):
            return self

        async def __aexit__(self, *args):
            pass

        def __init__(self, *args):
            pass

        async def initialize(self):
            # Browser authorization may exceed the noninteractive phase budget.
            await asyncio.sleep(0.04)

        async def list_tools(self):
            return SimpleNamespace(tools=[], nextCursor=None)

    monkeypatch.setattr(oauth_client, "build_oauth_http_client", build_client)
    monkeypatch.setattr(gateway, "streamable_http_client", streams)
    monkeypatch.setattr(gateway, "ClientSession", Session)
    async with asyncio.timeout(1):
        async with gateway._gateway_lifespan(
            FastMCP("oauth-budget"), "unused", None, 0.01, oauth=True, oauth_timeout=0.5
        ):
            assert gateway._state is not None
    assert observed["timeout"] == 0.5
    assert gateway._state is None
    assert not any(t.get_name() == "slopsearx-gateway-upstream" for t in asyncio.all_tasks())
