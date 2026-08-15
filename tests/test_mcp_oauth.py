"""Tests for MCP OAuth 2.1 authorization-server support.

Unit tests exercise the provider directly; integration tests run a real
OAuth-enabled server over HTTP and drive the full client flow
(discovery → dynamic registration → authorization → token → MCP call →
revocation) the way Claude Web connectors would.
"""

from __future__ import annotations

import asyncio
import base64
import hashlib
import secrets
import socket
import time
from typing import Any
from urllib.parse import parse_qs, urlparse

import httpx
import pytest
import uvicorn
from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client
from mcp.server.auth.provider import AuthorizationParams
from mcp.shared.memory import create_connected_server_and_client_session
from mcp.types import AnyUrl

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.oauth import (
    SlopSearxOAuthProvider,
    oauth_settings_from_policy,
)
from slopsearx.mcp.security import make_http_app
from slopsearx.mcp.server import create_server


def _free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


async def _wait_for_port(port: int, timeout: float = 20.0) -> bool:
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


def _pkce_pair() -> tuple[str, str]:
    verifier = secrets.token_urlsafe(48)
    digest = hashlib.sha256(verifier.encode()).digest()
    challenge = base64.urlsafe_b64encode(digest).decode().rstrip("=")
    return verifier, challenge


# ---------------------------------------------------------------------------
# Provider unit tests
# ---------------------------------------------------------------------------


class TestProvider:
    async def test_register_and_get_client(self) -> None:
        provider = SlopSearxOAuthProvider(None)
        from mcp.shared.auth import OAuthClientInformationFull

        info = OAuthClientInformationFull(
            client_id="test-client",
            redirect_uris=["http://localhost/cb"],
            token_endpoint_auth_method="none",
            grant_types=["authorization_code"],
            response_types=["code"],
        )
        await provider.register_client(info)
        loaded = await provider.get_client("test-client")
        assert loaded is not None
        assert loaded.client_id == "test-client"
        assert await provider.get_client("missing") is None

    async def test_authorize_exchange_and_refresh(self) -> None:
        provider = SlopSearxOAuthProvider(None)
        from mcp.shared.auth import OAuthClientInformationFull

        client = OAuthClientInformationFull(
            client_id="c1",
            redirect_uris=["http://localhost/cb"],
            token_endpoint_auth_method="none",
            grant_types=["authorization_code"],
            response_types=["code"],
        )
        await provider.register_client(client)
        params = AuthorizationParams(
            state="st-123",
            scopes=[],
            code_challenge="challenge-value",
            redirect_uri=AnyUrl("http://localhost/cb"),
            redirect_uri_provided_explicitly=True,
        )
        url = await provider.authorize(client, params)
        assert "code=" in url
        assert "state=st-123" in url
        code_str = parse_qs(urlparse(url).query)["code"][0]

        code = await provider.load_authorization_code(client, code_str)
        assert code is not None
        assert code.client_id == "c1"

        token = await provider.exchange_authorization_code(client, code)
        assert token.access_token
        assert token.refresh_token
        assert token.token_type == "Bearer"

        access = await provider.load_access_token(token.access_token)
        assert access is not None
        assert access.client_id == "c1"

        # Refresh rotation: new tokens, old refresh token invalid
        refresh = await provider.load_refresh_token(client, token.refresh_token)
        assert refresh is not None
        rotated = await provider.exchange_refresh_token(client, refresh, [])
        assert rotated.access_token != token.access_token
        assert rotated.refresh_token != token.refresh_token
        assert await provider.load_refresh_token(client, token.refresh_token) is None

        # Revocation removes the whole family
        new_access = await provider.load_access_token(rotated.access_token)
        assert new_access is not None
        await provider.revoke_token(new_access)
        assert await provider.load_access_token(rotated.access_token) is None
        assert await provider.load_refresh_token(client, rotated.refresh_token) is None

    async def test_code_bound_to_client(self) -> None:
        provider = SlopSearxOAuthProvider(None)
        from mcp.shared.auth import OAuthClientInformationFull

        def make_client(cid: str) -> OAuthClientInformationFull:
            return OAuthClientInformationFull(
                client_id=cid,
                redirect_uris=["http://localhost/cb"],
                token_endpoint_auth_method="none",
                grant_types=["authorization_code"],
                response_types=["code"],
            )

        await provider.register_client(make_client("a"))
        await provider.register_client(make_client("b"))
        params = AuthorizationParams(
            state=None,
            scopes=[],
            code_challenge="ch",
            redirect_uri=AnyUrl("http://localhost/cb"),
            redirect_uri_provided_explicitly=True,
        )
        url = await provider.authorize(await provider.get_client("a"), params)  # type: ignore[arg-type]
        code_str = parse_qs(urlparse(url).query)["code"][0]

        # Client B must not be able to load or exchange A's code
        assert await provider.load_authorization_code(await provider.get_client("b"), code_str) is None  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Full OAuth flow over HTTP
# ---------------------------------------------------------------------------


@pytest.fixture
async def oauth_server() -> Any:
    """An OAuth-enabled MCP server over streamable HTTP."""
    port = _free_port()
    issuer = f"http://127.0.0.1:{port}"
    policy = MCPPolicy(oauth_enabled=True, oauth_issuer_url=issuer)
    settings, provider = oauth_settings_from_policy(policy)
    server = create_server(host="127.0.0.1", port=port, oauth=settings, oauth_provider=provider)
    app = make_http_app(server, "")
    uvicorn_server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=port, log_level="warning"))
    task = asyncio.create_task(uvicorn_server.serve())
    assert await _wait_for_port(port), "OAuth server did not start"
    yield port
    uvicorn_server.should_exit = True
    try:
        await asyncio.wait_for(task, timeout=10)
    except asyncio.TimeoutError:
        task.cancel()


async def _oauth_client_credentials(base: str) -> tuple[str, str, str, str]:
    """Register a public client and complete the authorize step.

    Returns (client_id, verifier, code, redirect_uri).
    """
    redirect_uri = f"{base}/callback"
    async with httpx.AsyncClient(timeout=10) as client:
        registration = await client.post(
            f"{base}/register",
            json={
                "redirect_uris": [redirect_uri],
                "token_endpoint_auth_method": "none",
                "grant_types": ["authorization_code", "refresh_token"],
                "response_types": ["code"],
            },
        )
        assert registration.status_code in (200, 201), registration.text
        client_id = registration.json()["client_id"]

        verifier, challenge = _pkce_pair()
        authorize = await client.get(
            f"{base}/authorize",
            params={
                "response_type": "code",
                "client_id": client_id,
                "redirect_uri": redirect_uri,
                "code_challenge": challenge,
                "code_challenge_method": "S256",
                "state": "xyz",
            },
            follow_redirects=False,
        )
        assert authorize.status_code in (302, 307), authorize.text
        location = authorize.headers["location"]
        query = parse_qs(urlparse(location).query)
        assert query.get("state") == ["xyz"]
        code = query["code"][0]
    return client_id, verifier, code, redirect_uri


class TestOAuthOverHTTP:
    async def test_metadata_discovery(self, oauth_server: Any) -> None:
        base = f"http://127.0.0.1:{oauth_server}"
        async with httpx.AsyncClient(timeout=10) as client:
            metadata = (await client.get(f"{base}/.well-known/oauth-authorization-server")).json()
        assert metadata["issuer"].rstrip("/") == base
        assert metadata["authorization_endpoint"] == f"{base}/authorize"
        assert metadata["token_endpoint"] == f"{base}/token"
        assert metadata["registration_endpoint"] == f"{base}/register"
        assert metadata["revocation_endpoint"] == f"{base}/revoke"

    async def test_full_flow_and_pkce(self, oauth_server: Any) -> None:
        base = f"http://127.0.0.1:{oauth_server}"
        client_id, verifier, code, redirect_uri = await _oauth_client_credentials(base)

        async with httpx.AsyncClient(timeout=10) as client:
            # Wrong PKCE verifier is rejected
            bad = await client.post(
                f"{base}/token",
                data={
                    "grant_type": "authorization_code",
                    "code": code,
                    "redirect_uri": redirect_uri,
                    "code_verifier": "wrong-verifier",
                    "client_id": client_id,
                },
            )
            assert bad.status_code == 400, bad.text

            # Correct verifier succeeds
            token = await client.post(
                f"{base}/token",
                data={
                    "grant_type": "authorization_code",
                    "code": code,
                    "redirect_uri": redirect_uri,
                    "code_verifier": verifier,
                    "client_id": client_id,
                },
            )
            assert token.status_code == 200, token.text
            tokens = token.json()
            assert "access_token" in tokens
            assert "refresh_token" in tokens

            # Access token works against /mcp
            async with httpx.AsyncClient(
                headers={"Authorization": f"Bearer {tokens['access_token']}"}, timeout=15
            ) as authed:
                async with streamable_http_client(f"{base}/mcp", http_client=authed) as (read, write, _):
                    async with ClientSession(read, write) as session:
                        await session.initialize()
                        tools = await session.list_tools()
                        assert len(tools.tools) == 15

            # Revocation invalidates the token (the SDK's revocation request
            # model requires the client_secret field; public clients send "")
            revoked = await client.post(
                f"{base}/revoke",
                data={"token": tokens["access_token"], "client_id": client_id, "client_secret": ""},
            )
            assert revoked.status_code == 200, revoked.text

            # The revoked token is rejected by /mcp
            async with httpx.AsyncClient(
                headers={"Authorization": f"Bearer {tokens['access_token']}"}, timeout=15
            ) as authed:
                probe = await authed.get(f"{base}/mcp")
                assert probe.status_code == 401

    async def test_oauth_settings_from_policy(self) -> None:
        policy = MCPPolicy(oauth_enabled=True, oauth_issuer_url="https://mcp.example.com/")
        settings, provider = oauth_settings_from_policy(policy)
        assert settings is not None
        assert str(settings.issuer_url) == "https://mcp.example.com/"
        assert str(settings.resource_server_url) == "https://mcp.example.com/mcp"
        assert settings.client_registration_options is not None
        assert settings.client_registration_options.enabled is True
        assert provider is not None

    async def test_oauth_disabled_returns_none(self) -> None:
        settings, provider = oauth_settings_from_policy(MCPPolicy())
        assert settings is None
        assert provider is None

    async def test_oauth_requires_issuer(self) -> None:
        with pytest.raises(ValueError):
            oauth_settings_from_policy(MCPPolicy(oauth_enabled=True))

    async def test_gateway_style_session_still_works_without_auth(self, oauth_server: Any) -> None:
        """OAuth mode does not break the in-process server surface."""
        port = oauth_server
        policy = MCPPolicy(oauth_enabled=True, oauth_issuer_url=f"http://127.0.0.1:{port}")
        settings, provider = oauth_settings_from_policy(policy)
        server = create_server(host="127.0.0.1", port=port, oauth=settings, oauth_provider=provider)
        async with create_connected_server_and_client_session(server) as client:
            tools = await client.list_tools()
            assert len(tools.tools) == 15
