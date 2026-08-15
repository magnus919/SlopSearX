"""Tests for the MCP HTTP transport auth boundary (design Phase 4)."""

from __future__ import annotations

from typing import Any

from slopsearx.mcp.security import bearer_auth_app, make_http_app


class _PassThroughApp:
    """Records that it was invoked and returns a fixed response."""

    def __init__(self) -> None:
        self.calls = 0
        self.scopes: list[dict[str, Any]] = []

    async def __call__(self, scope: dict[str, Any], receive: Any, send: Any) -> None:
        self.calls += 1
        self.scopes.append(scope)
        await send({"type": "http.response.start", "status": 200, "headers": []})
        await send({"type": "http.response.body", "body": b"ok"})


def _http_scope(headers: list[tuple[bytes, bytes]] | None = None) -> dict[str, Any]:
    return {"type": "http", "headers": headers or []}


async def _collect(app: Any, scope: dict[str, Any]) -> list[dict[str, Any]]:
    messages: list[dict[str, Any]] = []
    received: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        if not received:
            received.append({"type": "http.request", "body": b"", "more_body": False})
        return received[-1]

    async def send(message: dict[str, Any]) -> None:
        messages.append(message)

    await app(scope, receive, send)
    return messages


class TestBearerAuth:
    async def test_missing_token_returns_401(self) -> None:
        inner = _PassThroughApp()
        app = bearer_auth_app(inner, "sekret")
        messages = await _collect(app, _http_scope())

        assert messages[0]["status"] == 401
        assert inner.calls == 0

    async def test_wrong_token_returns_401(self) -> None:
        inner = _PassThroughApp()
        app = bearer_auth_app(inner, "sekret")
        messages = await _collect(app, _http_scope([(b"authorization", b"Bearer wrong")]))

        assert messages[0]["status"] == 401
        assert inner.calls == 0

    async def test_correct_token_passes_through(self) -> None:
        inner = _PassThroughApp()
        app = bearer_auth_app(inner, "sekret")
        messages = await _collect(app, _http_scope([(b"authorization", b"Bearer sekret")]))

        assert messages[0]["status"] == 200
        assert inner.calls == 1

    async def test_case_insensitive_scheme(self) -> None:
        inner = _PassThroughApp()
        app = bearer_auth_app(inner, "sekret")
        messages = await _collect(app, _http_scope([(b"authorization", b"bearer sekret")]))

        assert messages[0]["status"] == 200

    async def test_non_http_scope_passes_through(self) -> None:
        inner = _PassThroughApp()
        app = bearer_auth_app(inner, "sekret")
        await _collect(app, {"type": "lifespan"})

        assert inner.calls == 1

    async def test_make_http_app_without_token_unwrapped(self) -> None:
        class _Server:
            def __init__(self) -> None:
                self._inner = _PassThroughApp()

            def streamable_http_app(self) -> Any:
                return self._inner

        server = _Server()
        app = make_http_app(server, "")
        messages = await _collect(app, _http_scope())
        assert messages[0]["status"] == 200

    async def test_make_http_app_with_token_wrapped(self) -> None:
        class _Server:
            def __init__(self) -> None:
                self._inner = _PassThroughApp()

            def streamable_http_app(self) -> Any:
                return self._inner

        server = _Server()
        app = make_http_app(server, "sekret")
        denied = await _collect(app, _http_scope())
        assert denied[0]["status"] == 401
        allowed = await _collect(app, _http_scope([(b"authorization", b"Bearer sekret")]))
        assert allowed[0]["status"] == 200
