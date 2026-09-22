"""Bearer-token authentication for the MCP streamable-HTTP transport.

The stdio transport is trusted by its process-launch boundary. When the
server is exposed over HTTP, operators configure ``mcp.auth_token``
(or ``MCP_AUTH_TOKEN``); every request must then carry
``Authorization: Bearer <token>``. Requests without a valid token get
401. This is a plain ASGI wrapper around the FastMCP app — no secrets
are logged or echoed.
"""

from __future__ import annotations

import hmac
from typing import Any, Awaitable, Callable

ASGIApp = Callable[
    [dict[str, Any], Callable[..., Awaitable[dict[str, Any]]], Callable[..., Awaitable[None]]], Awaitable[None]
]

_UNAUTHORIZED_BODY = b"unauthorized"
_UNAUTHORIZED_HEADERS = [(b"content-type", b"text/plain; charset=utf-8")]


def bearer_auth_app(app: ASGIApp, token: str) -> ASGIApp:
    """Wrap an ASGI app so every HTTP request requires the bearer token."""
    expected = token.encode()

    async def wrapped(
        scope: dict[str, Any], receive: Callable[..., Awaitable[dict[str, Any]]], send: Callable[..., Awaitable[None]]
    ) -> None:
        if scope["type"] != "http":
            await app(scope, receive, send)
            return

        authorized = False
        for raw_name, raw_value in scope.get("headers", []):
            if raw_name.lower() == b"authorization":
                scheme, _, value = raw_value.partition(b" ")
                if scheme.lower() == b"bearer" and _safe_equal(value, expected):
                    authorized = True
                break

        if not authorized:
            await send(
                {
                    "type": "http.response.start",
                    "status": 401,
                    "headers": _UNAUTHORIZED_HEADERS,
                }
            )
            await send({"type": "http.response.body", "body": _UNAUTHORIZED_BODY})
            return

        await app(scope, receive, send)

    return wrapped


def _safe_equal(value: bytes, expected: bytes) -> bool:
    """Constant-time comparison for the bearer token."""
    return hmac.compare_digest(value, expected)


def make_http_app(server: Any, token: str) -> ASGIApp:
    """Build streamable HTTP, preserving static and OAuth auth boundaries.

    FastMCP 3/4 exposes ``http_app`` and owns OAuth middleware on that app.
    Its stateless JSON mode avoids keeping MCP sessions in individual replicas;
    the service's shared state lives in Valkey.
    The legacy MCP SDK exposes ``streamable_http_app`` instead. Static bearer
    authentication remains this outer wrapper in both generations.
    """
    if hasattr(server, "http_app"):
        app: ASGIApp = server.http_app(transport="streamable-http", stateless_http=True, json_response=True)
    else:  # pragma: no cover - exercised only with the legacy MCP SDK
        app = server.streamable_http_app()
    if token:
        return bearer_auth_app(app, token)
    return app
