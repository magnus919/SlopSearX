"""T05 isolated source oracle; loopback synthetic FastMCP only. Retire under G08."""

import argparse
import asyncio
import base64
import hashlib
import importlib.metadata
import json
import os
import socket
import subprocess
import sys
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import httpx
import uvicorn
from fastmcp import FastMCP

from slopsearx.mcp.oauth import SlopSearxOAuthProvider
from slopsearx.mcp.security import make_http_app

BASE = "http://127.0.0.1"
BASELINE = "403dfd3720e969fa829f426136bf0f05b0360c56"


async def capture():
    provider = SlopSearxOAuthProvider(None, base_url=BASE, issuer_url=BASE)
    app = make_http_app(FastMCP("T05 offline reference", auth=provider), "")
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    sock.listen(128)
    sock.setblocking(False)
    server = uvicorn.Server(uvicorn.Config(app, log_level="error", lifespan="on"))
    task = asyncio.create_task(server.serve(sockets=[sock]))
    while not server.started:
        if task.done():
            await task
        await asyncio.sleep(0.001)
    aliases = {}
    rows = []
    counts = {}

    def alias(value, kind):
        if value not in aliases:
            counts[kind] = counts.get(kind, 0) + 1
            aliases[value] = "<" + kind + str(counts[kind]) + ">"

    def norm(v):
        if isinstance(v, dict):
            return {k: ("<epoch-seconds>" if k == "client_id_issued_at" else norm(x)) for k, x in v.items()}
        if isinstance(v, (list, tuple)):
            return [norm(x) for x in v]
        if isinstance(v, str):
            if v[:6].lower() == "basic ":
                try:
                    decoded = base64.b64decode(v[6:]).decode()
                    for old, new in aliases.items():
                        decoded = decoded.replace(old, new)
                    return v[:6] + base64.b64encode(decoded.encode()).decode()
                except (ValueError, UnicodeError):
                    pass
            for old, new in aliases.items():
                v = v.replace(old, new)
        return v

    async def local_only(request):
        if request.url.host != "127.0.0.1":
            raise RuntimeError("oracle forbids nonloopback request")

    try:
        async with httpx.AsyncClient(
            base_url=f"http://127.0.0.1:{sock.getsockname()[1]}",
            follow_redirects=False,
            event_hooks={"request": [local_only]},
        ) as c:

            async def req(name, method, path, **kw):
                response = await c.request(method, path, **kw)
                try:
                    body = response.json()
                except ValueError:
                    body = response.text
                if isinstance(body, dict):
                    for k, kind in [
                        ("client_id", "client"),
                        ("client_secret", "secret"),
                        ("access_token", "access"),
                        ("refresh_token", "refresh"),
                    ]:
                        if body.get(k):
                            alias(body[k], kind)
                if "location" in response.headers:
                    code = parse_qs(urlparse(response.headers["location"]).query).get("code", [])
                    if code:
                        alias(code[0], "code")
                keys = [
                    "content-type",
                    "cache-control",
                    "pragma",
                    "location",
                    "www-authenticate",
                    "allow",
                    "access-control-allow-origin",
                    "access-control-allow-methods",
                    "access-control-allow-headers",
                    "access-control-max-age",
                    "vary",
                ]
                rows.append(
                    {
                        "case": name,
                        "request": norm({"method": method, "path": path, **kw}),
                        "response": norm(
                            {
                                "status": response.status_code,
                                "headers": {k: response.headers[k] for k in keys if k in response.headers},
                                "body": body,
                            }
                        ),
                    }
                )
                return response

            for path in [
                "/.well-known/oauth-authorization-server",
                "/.well-known/oauth-protected-resource/mcp",
                "/.well-known/openid-configuration",
            ]:
                await req("metadata:" + path, "GET", path, headers={"Origin": "https://client.example"})
            for path in ["/register", "/token", "/revoke", "/.well-known/oauth-authorization-server"]:
                await req(
                    "cors:" + path,
                    "OPTIONS",
                    path,
                    headers={
                        "Origin": "https://client.example",
                        "Access-Control-Request-Method": "POST"
                        if path in ["/register", "/token", "/revoke"]
                        else "GET",
                        "Access-Control-Request-Headers": "authorization,content-type",
                    },
                )
            for method in ["GET", "HEAD", "POST", "DELETE", "PUT"]:
                await req(
                    "protected:missing:" + method,
                    method,
                    "/mcp",
                    headers={"Accept": "application/json, text/event-stream"},
                )
            registrations = [
                ("missing", {}),
                ("null", {"redirect_uris": None}),
                ("empty", {"redirect_uris": []}),
                ("bad-url", {"redirect_uris": ["not a URL"]}),
                ("bad-method", {"redirect_uris": ["http://localhost/cb"], "token_endpoint_auth_method": "bad"}),
                (
                    "private-key",
                    {"redirect_uris": ["http://localhost/cb"], "token_endpoint_auth_method": "private_key_jwt"},
                ),
                ("bad-grants", {"redirect_uris": ["http://localhost/cb"], "grant_types": ["refresh_token"]}),
                ("bad-response", {"redirect_uris": ["http://localhost/cb"], "response_types": []}),
                ("bad-app-type", {"redirect_uris": ["http://localhost/cb"], "application_type": "other"}),
                (
                    "public",
                    {
                        "redirect_uris": ["http://localhost/cb"],
                        "token_endpoint_auth_method": "none",
                        "scope": "read write",
                    },
                ),
                ("post", {"redirect_uris": ["http://localhost/cb"], "scope": "read write"}),
                (
                    "basic",
                    {
                        "redirect_uris": ["http://localhost/cb"],
                        "token_endpoint_auth_method": "client_secret_basic",
                        "scope": "read",
                    },
                ),
                (
                    "full",
                    {
                        "redirect_uris": ["custom:/callback", "http://localhost/cb"],
                        "token_endpoint_auth_method": "none",
                        "scope": "read write",
                        "client_name": "synthetic client",
                        "client_uri": "https://example.com",
                        "logo_uri": "",
                        "contacts": ["a@example.com"],
                        "tos_uri": "https://example.com/tos",
                        "policy_uri": "https://example.com/policy",
                        "jwks_uri": "https://example.com/keys",
                        "jwks": {"keys": []},
                        "software_id": "synthetic",
                        "software_version": "1",
                        "unknown_field": "ignored",
                    },
                ),
            ]
            clients = {}
            for name, body in registrations:
                response = await req("register:" + name, "POST", "/register", json=body)
                if response.status_code == 201:
                    clients[name] = response.json()
            client = clients["public"]
            cid = client["client_id"]
            verifier = "a" * 48
            challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
            params = {
                "client_id": cid,
                "response_type": "code",
                "code_challenge": challenge,
                "redirect_uri": "http://localhost/cb",
                "state": "source-state",
                "scope": "read",
                "resource": BASE + "/mcp",
            }
            changes = [
                ("unknown-client", {"client_id": "unknown-client"}),
                ("bad-redirect", {"redirect_uri": "http://other.invalid/cb"}),
                ("bad-scope", {"scope": "admin"}),
                ("bad-response", {"response_type": "token"}),
                ("plain", {"code_challenge_method": "plain"}),
                ("empty-challenge", {"code_challenge": ""}),
            ]
            for name, change in changes:
                await req("authorize:" + name, "GET", "/authorize", params={**params, **change})
            await req(
                "authorize:foreign-resource",
                "GET",
                "/authorize",
                params={**params, "resource": "https://other.example/mcp"},
            )
            await req("authorize:multipart", "POST", "/authorize", files={k: (None, v) for k, v in params.items()})
            await req(
                "token:multipart",
                "POST",
                "/token",
                files={
                    "client_id": (None, cid),
                    "grant_type": (None, "authorization_code"),
                    "code": (None, "unknown"),
                    "code_verifier": (None, "wrong"),
                },
            )
            await req(
                "revoke:multipart",
                "POST",
                "/revoke",
                files={"client_id": (None, cid), "client_secret": (None, ""), "token": (None, "unknown")},
            )
            for path in ["/register", "/token", "/revoke", "/.well-known/oauth-authorization-server"]:
                await req(
                    "cors-allowed:" + path,
                    "OPTIONS",
                    path,
                    headers={
                        "Origin": "https://client.example",
                        "Access-Control-Request-Method": "GET" if path.startswith("/.well-known/") else "POST",
                        "Access-Control-Request-Headers": "content-type",
                    },
                )

            for field in ["response_type", "code_challenge", "client_id"]:
                await req(
                    "authorize:missing:" + field,
                    "GET",
                    "/authorize",
                    params={k: v for k, v in params.items() if k != field},
                )
            auth = await req("authorize:good", "GET", "/authorize", params=params)
            code = parse_qs(urlparse(auth.headers["location"]).query)["code"][0]
            data = {
                "grant_type": "authorization_code",
                "client_id": cid,
                "code": code,
                "redirect_uri": "http://localhost/cb",
                "code_verifier": verifier,
            }
            for name, change in [
                ("bad-verifier", {"code_verifier": "wrong"}),
                ("bad-redirect", {"redirect_uri": "http://other.invalid/cb"}),
                ("unknown-code", {"code": "unknown"}),
            ]:
                await req("token:" + name, "POST", "/token", data={**data, **change})
            await req(
                "token:missing-redirect", "POST", "/token", data={k: v for k, v in data.items() if k != "redirect_uri"}
            )
            await req(
                "token:missing-client", "POST", "/token", data={k: v for k, v in data.items() if k != "client_id"}
            )
            pair = (await req("token:good", "POST", "/token", data=data)).json()
            await req("token:replay", "POST", "/token", data=data)
            refresh = {"grant_type": "refresh_token", "client_id": cid, "refresh_token": pair["refresh_token"]}
            await req("refresh:bad-scope", "POST", "/token", data={**refresh, "scope": "admin"})
            next_pair = (await req("refresh:good", "POST", "/token", data=refresh)).json()
            await req("refresh:replay", "POST", "/token", data=refresh)
            await req(
                "protected:valid-post",
                "POST",
                "/mcp",
                headers={
                    "Authorization": "Bearer " + next_pair["access_token"],
                    "Accept": "application/json, text/event-stream",
                },
                json={"jsonrpc": "2.0", "id": 1, "method": "ping"},
            )
            revoke = {"client_id": cid, "client_secret": "", "token": next_pair["access_token"]}
            await req(
                "revoke:missing-secret",
                "POST",
                "/revoke",
                data={k: v for k, v in revoke.items() if k != "client_secret"},
            )
            await req("revoke:bad-hint", "POST", "/revoke", data={**revoke, "token_type_hint": "wrong"})
            await req("revoke:good", "POST", "/revoke", data=revoke)
            await req("revoke:unknown", "POST", "/revoke", data={**revoke, "token": "unknown"})
            await req(
                "protected:revoked-post",
                "POST",
                "/mcp",
                headers={
                    "Authorization": "Bearer " + next_pair["access_token"],
                    "Accept": "application/json, text/event-stream",
                },
                json={"jsonrpc": "2.0", "id": 1, "method": "ping"},
            )
            implicit = {k: v for k, v in params.items() if k != "redirect_uri"}
            auth = await req("authorize:implicit-post", "POST", "/authorize", data=implicit)
            code = parse_qs(urlparse(auth.headers["location"]).query)["code"][0]
            await req("token:implicit-with-redirect", "POST", "/token", data={**data, "code": code})
            await req(
                "token:implicit-malformed-redirect",
                "POST",
                "/token",
                data={**data, "code": code, "redirect_uri": "not a url"},
            )
            await req(
                "token:implicit-good",
                "POST",
                "/token",
                data={k: v for k, v in {**data, "code": code}.items() if k != "redirect_uri"},
            )
            for kind in ["post", "basic"]:
                cl = clients[kind]
                form = {"grant_type": "refresh_token", "client_id": cl["client_id"], "refresh_token": "unknown"}
                if kind == "post":
                    await req("client-auth:post-missing", "POST", "/token", data=form)
                    await req("client-auth:post-wrong", "POST", "/token", data={**form, "client_secret": "wrong"})
                    await req(
                        "client-auth:post-good", "POST", "/token", data={**form, "client_secret": cl["client_secret"]}
                    )
                else:
                    b64 = base64.b64encode((cl["client_id"] + ":" + cl["client_secret"]).encode()).decode()
                    for label, header in [
                        ("missing", ""),
                        ("bad-case", "basic " + b64),
                        ("wrong", "Basic " + base64.b64encode((cl["client_id"] + ":wrong").encode()).decode()),
                        ("good", "Basic " + b64),
                    ]:
                        await req(
                            "client-auth:basic-" + label, "POST", "/token", data=form, headers={"Authorization": header}
                        )
            # Append review observations after the original corpus to retain all aliases.
            for field in ["scope", "client_name", "software_id", "software_version", "jwks"]:
                await req(
                    "register:empty-placeholder:" + field,
                    "POST",
                    "/register",
                    json={"redirect_uris": ["http://localhost/cb"], "token_endpoint_auth_method": "none", field: ""},
                )
            await req(
                "authorize:bad-response-missing-challenge",
                "GET",
                "/authorize",
                params={k: v for k, v in {**params, "response_type": "token"}.items() if k != "code_challenge"},
            )
            only_code = (
                await req(
                    "register:authorization-code-only",
                    "POST",
                    "/register",
                    json={
                        "redirect_uris": ["http://localhost/cb"],
                        "token_endpoint_auth_method": "none",
                        "grant_types": ["authorization_code"],
                    },
                )
            ).json()
            await req(
                "token:disallowed-refresh-missing-token",
                "POST",
                "/token",
                data={"client_id": only_code["client_id"], "grant_type": "refresh_token"},
            )
            await req(
                "token:disallowed-refresh-complete",
                "POST",
                "/token",
                data={"client_id": only_code["client_id"], "grant_type": "refresh_token", "refresh_token": "unknown"},
            )
            await req(
                "token:unknown-code-malformed-redirect",
                "POST",
                "/token",
                data={**data, "code": "unknown", "redirect_uri": "not a url"},
            )
    finally:
        server.should_exit = True
        await task
        sock.close()
    return rows


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    if os.environ.get("PYTHONHASHSEED") != "0":
        raise RuntimeError("PYTHONHASHSEED=0 required for source set-derived Allow header order")
    if (
        sys.version_info[:3] != (3, 14, 7)
        or importlib.metadata.version("mcp") != "2.3.0"
        or importlib.metadata.version("fastmcp") != "4.0.11"
    ):
        raise RuntimeError("reference runtime mismatch")
    paths = [
        "slopsearx/mcp/oauth.py",
        "slopsearx/mcp/security.py",
        "slopsearx/mcp/server.py",
        "tests/test_mcp_oauth.py",
        "tests/test_mcp_security.py",
        "uv.lock",
    ]
    sources = {p: subprocess.check_output(["git", "rev-parse", BASELINE + ":" + p]).decode().strip() for p in paths}
    if any(subprocess.check_output(["git", "hash-object", "--", p]).decode().strip() != v for p, v in sources.items()):
        raise RuntimeError("reference source drift")
    rows = asyncio.run(capture())
    out = {
        "schema_version": 1,
        "plan_id": "T05",
        "baseline": BASELINE,
        "python": "3.14.7",
        "python_hash_seed": "0",
        "mcp": "2.3.0",
        "fastmcp": "4.0.11",
        "sources": sources,
        "capture": "independent loopback HTTP client; synthetic provider; zero external/provider calls",
        "normalization": "opaque IDs/tokens/codes and issuance epoch only; no status/body/error/header normalization",
        "cases": rows,
    }
    Path(args.output).write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
