"""Tests for the OAuth 2.1 *client* flow used by the remote gateway.

Runs a real OAuth-mode SlopSearX MCP server over HTTP, then connects a
gateway through the full OAuth client flow (registration → authorize →
token exchange) using injected handlers that simulate the browser
completing the authorization. Also covers token persistence (reuse without
re-authorizing) and the loopback callback server.
"""

from __future__ import annotations

import asyncio
import socket
import time
from typing import Any
from urllib.parse import parse_qs, urlparse

try:
    import httpx2 as httpx
except ImportError:
    import httpx
import pytest
import uvicorn
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.gateway import create_gateway
from slopsearx.mcp.oauth import oauth_settings_from_policy
from slopsearx.mcp.oauth_client import (
    FileTokenStorage,
    LoopbackCallback,
    build_oauth_http_client,
)
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server
from tests.test_mcp_gateway import _gateway_client


def _free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


async def _wait_for_metadata(port: int, timeout: float = 20.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            async with httpx.AsyncClient(timeout=2.0) as client:
                response = await client.get(f"http://127.0.0.1:{port}/.well-known/oauth-authorization-server")
                if response.status_code == 200:
                    return True
        except Exception:
            pass
        await asyncio.sleep(0.2)
    return False


@pytest.fixture
async def oauth_remote() -> Any:
    """An OAuth-mode MCP server over streamable HTTP (the remote side)."""
    port = _free_port()
    policy = MCPPolicy(oauth_enabled=True, oauth_issuer_url=f"http://127.0.0.1:{port}")
    settings, provider = oauth_settings_from_policy(policy)
    server = create_server(host="127.0.0.1", port=port, oauth=settings, oauth_provider=provider)
    uvicorn_server = uvicorn.Server(
        uvicorn.Config(make_http_app(server, ""), host="127.0.0.1", port=port, log_level="warning")
    )
    task = asyncio.create_task(uvicorn_server.serve())
    assert await _wait_for_metadata(port), "OAuth remote did not start"
    yield port
    uvicorn_server.should_exit = True
    try:
        await asyncio.wait_for(task, timeout=10)
    except asyncio.TimeoutError:
        task.cancel()


class TestFileTokenStorage:
    async def test_round_trip(self, tmp_path) -> None:
        path = tmp_path / "tokens.json"
        storage = FileTokenStorage(path)

        from mcp.shared.auth import OAuthClientInformationFull, OAuthToken

        client_info = OAuthClientInformationFull(
            client_id="c1",
            redirect_uris=["http://127.0.0.1:8765/callback"],
            token_endpoint_auth_method="none",
            grant_types=["authorization_code", "refresh_token"],
            response_types=["code"],
        )
        tokens = OAuthToken(access_token="at", token_type="Bearer", expires_in=3600, scope=None, refresh_token="rt")

        await storage.set_client_info(client_info)
        await storage.set_tokens(tokens)

        reloaded = FileTokenStorage(path)
        assert await reloaded.get_client_info() is not None
        assert (await reloaded.get_client_info()).client_id == "c1"  # type: ignore[union-attr]
        assert (await reloaded.get_tokens()).access_token == "at"  # type: ignore[union-attr]

        # File permissions are restrictive
        assert (path.stat().st_mode & 0o077) == 0


class TestLoopbackCallback:
    async def test_receives_redirect(self) -> None:
        port = _free_port()
        callback = LoopbackCallback(port, timeout=10)
        await callback.start()

        async with httpx.AsyncClient(timeout=5) as client:
            response = await client.get(f"{callback.redirect_uri}?code=abc123&state=xyz")
            assert response.status_code == 200

        code, state = await callback.wait()
        assert code == "abc123"
        assert state == "xyz"
        await callback.stop()

    async def test_times_out(self) -> None:
        port = _free_port()
        callback = LoopbackCallback(port, timeout=0.5)
        await callback.start()
        with pytest.raises(Exception):
            await callback.wait()
        await callback.stop()


class TestGatewayOAuthFlow:
    async def test_full_flow_and_token_reuse(self, oauth_remote: Any, tmp_path) -> None:
        remote_url = f"http://127.0.0.1:{oauth_remote}/mcp"
        token_file = tmp_path / "gateway-oauth.json"

        # First run: complete the OAuth flow with simulated browser handlers.
        captured: dict[str, str] = {}
        redirect_calls: list[str] = []

        async def fake_redirect(authorization_url: str) -> None:
            redirect_calls.append(authorization_url)
            captured["url"] = authorization_url

        async def fake_callback() -> tuple[str, str | None]:
            async with httpx.AsyncClient(timeout=10) as client:
                response = await client.get(captured["url"], follow_redirects=False)
            query = parse_qs(urlparse(response.headers["location"]).query)
            return query["code"][0], query.get("state", [None])[0]

        gateway = create_gateway(
            remote_url,
            oauth=True,
            oauth_token_file=str(token_file),
            oauth_redirect_handler=fake_redirect,
            oauth_callback_handler=fake_callback,
        )
        async with _gateway_client(gateway) as client:
            tools = await client.list_tools()
            assert len(tools) == 35
            status = await client.call_tool_mcp("slopsearx_get_service_status", {})
            assert status.model_dump(by_alias=True)["isError"] is False

        assert len(redirect_calls) == 1, "exactly one authorization flow expected"
        assert token_file.exists()

        # Second run: stored tokens are reused — no re-authorization.
        async def should_not_redirect(authorization_url: str) -> None:
            raise AssertionError(f"re-authorized unexpectedly: {authorization_url}")

        # Verify a fresh OAuth client can reuse the persisted grant without
        # starting a second gateway ASGI lifespan in the same event loop.
        async with build_oauth_http_client(
            remote_url,
            token_file=str(token_file),
            redirect_handler=should_not_redirect,
            callback_handler=fake_callback,
        ) as authorized:
            async with streamable_http_client(remote_url, http_client=authorized) as streams:
                async with ClientSession(streams[0], streams[1]) as session:
                    await session.initialize()
                    tools = await session.list_tools()
                    assert len(tools.tools) == 35

    async def test_oauth_and_token_are_mutually_exclusive(self) -> None:
        with pytest.raises(ValueError):
            create_gateway("http://127.0.0.1:1/mcp", token="secret", oauth=True)

    async def test_build_oauth_http_client_wires_provider(self) -> None:
        client = build_oauth_http_client("http://127.0.0.1:9999/mcp", token_file="/tmp/nonexistent-oauth.json")
        assert isinstance(client, httpx.AsyncClient)
        await client.aclose()

    async def test_callback_adapter_preserves_legacy_tuple_for_current_sdk(self, tmp_path) -> None:
        """The provider callback remains compatible with the installed SDK generation."""
        client = build_oauth_http_client(
            "http://127.0.0.1:9999/mcp",
            token_file=tmp_path / "oauth.json",
            callback_handler=lambda: _callback_result(),
        )
        try:
            result = await client.auth.context.callback_handler()
            if hasattr(result, "code"):
                assert result.code == "code"
                assert result.state == "state"
            else:
                assert result == ("code", "state")
        finally:
            await client.aclose()


async def _callback_result() -> tuple[str, str]:
    return "code", "state"
