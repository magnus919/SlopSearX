"""Offline migration-only T04 reference capture; retired under G08, never gate/runtime."""

import asyncio
import importlib.metadata
import json
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

from mcp.server.auth.provider import AuthorizationParams
from mcp.shared.auth import AnyUrl, OAuthClientInformationFull

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.oauth import SlopSearxOAuthProvider, oauth_settings_from_policy

BASELINE = "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5"
SOURCES = ["slopsearx/mcp/oauth.py", "slopsearx/mcp/security.py", "tests/test_mcp_oauth.py", "uv.lock"]


def blob(path):
    assert Path(path).read_bytes() == subprocess.check_output(["git", "show", f"{BASELINE}:{path}"])
    return subprocess.check_output(["git", "hash-object", path], text=True).strip()


async def main():
    assert sys.version_info[:3] == (3, 14, 7)
    clients = []
    inputs = [
        {"client_id": "minimal"},
        {"client_id": "null", "redirect_uris": None, "token_endpoint_auth_method": None},
        {"client_id": "empty", "redirect_uris": [], "token_endpoint_auth_method": ""},
        {
            "client_id": "complete",
            "client_secret": "synthetic-secret",
            "redirect_uris": ["http://localhost/cb"],
            "token_endpoint_auth_method": "client_secret_post",
            "scope": "read write",
            "client_name": "Synthetic client",
            "client_uri": "https://example.com/client",
            "logo_uri": "https://example.com/logo",
            "contacts": ["synthetic@example.com"],
            "tos_uri": "https://example.com/tos",
            "policy_uri": "https://example.com/policy",
            "jwks_uri": "https://example.com/jwks",
            "jwks": {"keys": [{"kty": "RSA", "kid": "synthetic"}]},
            "software_id": "software",
            "software_version": "1",
            "application_type": "web",
            "issuer": "https://example.com",
            "client_id_issued_at": 1,
            "client_secret_expires_at": 99,
        },
    ]
    provider = SlopSearxOAuthProvider(None)
    for raw in inputs:
        model = OAuthClientInformationFull.model_validate(raw)
        await provider.register_client(model)
        loaded = await provider.get_client(model.client_id)
        clients.append({"input": raw, "expected": loaded.model_dump(mode="json")})
    client = OAuthClientInformationFull(client_id="flow", redirect_uris=["http://localhost/cb"])
    await provider.register_client(client)
    params = AuthorizationParams(
        state="state",
        scopes=["read", "write"],
        code_challenge="pkce-challenge",
        redirect_uri=AnyUrl("http://localhost/cb"),
        redirect_uri_provided_explicitly=True,
        resource="https://example.com/mcp",
    )
    with (
        patch("slopsearx.mcp.oauth.time.time", return_value=1000),
        patch(
            "slopsearx.mcp.oauth.secrets.token_urlsafe",
            side_effect=["source-code", "source-access", "source-refresh", "rotated-access", "rotated-refresh"],
        ),
    ):
        redirect = await provider.authorize(client, params)
        code = await provider.load_authorization_code(client, parse_qs(urlparse(redirect).query)["code"][0])
        assert code is not None
        tokens = await provider.exchange_authorization_code(client, code)
        access = await provider.load_access_token(tokens.access_token)
        refresh = await provider.load_refresh_token(client, tokens.refresh_token)
        rotated = await provider.exchange_refresh_token(client, refresh, ["read"])
        rotation_old_access_missing = await provider.load_access_token(tokens.access_token) is None
        await provider.revoke_token(await provider.load_access_token(rotated.access_token))
        revoked_missing = await provider.load_refresh_token(client, rotated.refresh_token) is None
    settings_cases = []
    for enabled, issuer in [
        (False, ""),
        (True, ""),
        (True, "HTTPS://EXAMPLE.COM:443/"),
        (True, "https://éxample.com"),
        (True, "https://example.com/a/../b/"),
        (True, "http://localhost/"),
        (True, "https://example.com/a/.."),
        (True, "https://example.com/a/."),
        (True, "https://example.com/a/%2e%2e"),
    ]:
        try:
            settings, _ = oauth_settings_from_policy(MCPPolicy(oauth_enabled=enabled, oauth_issuer_url=issuer))
            expected = (
                None
                if settings is None
                else {
                    "issuer_url": str(settings.issuer_url),
                    "resource_server_url": str(settings.resource_server_url),
                }
            )
        except ValueError:
            expected = {"error": True}
        settings_cases.append({"enabled": enabled, "issuer": issuer, "expected": expected})
    return {
        "schema_version": 1,
        "baseline": BASELINE,
        "python": sys.version,
        "versions": {name: importlib.metadata.version(name) for name in ["mcp", "fastmcp"]},
        "sources": {path: blob(path) for path in SOURCES},
        "clients": clients,
        "settings": settings_cases,
        "flow": {
            "code": code.model_dump(mode="json"),
            "access": access.model_dump(mode="json"),
            "refresh": refresh.model_dump(mode="json"),
            "token_response": tokens.model_dump(mode="json"),
            "rotation_old_access_missing": rotation_old_access_missing,
            "revoked_refresh_missing": revoked_missing,
        },
    }


if __name__ == "__main__":
    print(json.dumps(asyncio.run(main()), ensure_ascii=False, indent=2) + "\n", end="")
