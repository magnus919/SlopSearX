"""OAuth 2.1 *client* flow for the remote gateway mode.

Lets ``slopsearx-mcp --remote <url> --oauth`` connect to a SlopSearX MCP
server that runs in OAuth mode (§4.5 of docs/MCP_SERVER.md), completing the
standard MCP OAuth flow on the agent's host:

1. discovery of the authorization-server metadata;
2. dynamic client registration (RFC 7591, public client + PKCE);
3. authorization via a local loopback callback (browser opened/printed,
   redirect received on ``http://127.0.0.1:<port>/callback``);
4. token exchange, refresh, and persistence in a 0600 JSON file so
   subsequent runs skip re-authorization.

The heavy lifting (registration, PKCE, refresh, 401 retry) is the MCP
SDK's :class:`~mcp.client.auth.oauth2.OAuthClientProvider`, an
``httpx.Auth`` that this module wires with storage and the interactive
handlers.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Any, Awaitable, Callable, cast

import httpx
import uvicorn
from mcp.client.auth.exceptions import OAuthFlowError
from mcp.client.auth.oauth2 import OAuthClientProvider
from mcp.shared.auth import OAuthClientInformationFull, OAuthClientMetadata, OAuthToken

logger = logging.getLogger(__name__)

DEFAULT_CALLBACK_PORT = 8765


# ---------------------------------------------------------------------------
# Token storage
# ---------------------------------------------------------------------------


class FileTokenStorage:
    """Persists OAuth client info and tokens to a JSON file (0600 perms)."""

    def __init__(self, path: str | Path) -> None:
        self._path = Path(path)
        self._data: dict[str, Any] = {}
        self._load()

    def _load(self) -> None:
        if self._path.exists():
            try:
                self._data = json.loads(self._path.read_text())
            except (OSError, json.JSONDecodeError):
                self._data = {}

    def _save(self) -> None:
        self._path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self._path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self._data, indent=2))
        os.chmod(tmp, 0o600)
        tmp.replace(self._path)

    async def get_client_info(self) -> OAuthClientInformationFull | None:
        raw = self._data.get("client_info")
        return OAuthClientInformationFull.model_validate(raw) if raw else None

    async def set_client_info(self, client_info: OAuthClientInformationFull) -> None:
        self._data["client_info"] = client_info.model_dump(mode="json")
        self._save()

    async def get_tokens(self) -> OAuthToken | None:
        raw = self._data.get("tokens")
        return OAuthToken.model_validate(raw) if raw else None

    async def set_tokens(self, tokens: OAuthToken) -> None:
        self._data["tokens"] = tokens.model_dump(mode="json")
        self._save()


def default_token_path(server_url: str) -> Path:
    """Per-server token file under the user config dir."""
    digest = hashlib.sha256(server_url.encode()).hexdigest()[:16]
    base = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
    return base / "slopsearx" / "oauth" / f"{digest}.json"


# ---------------------------------------------------------------------------
# Interactive handlers
# ---------------------------------------------------------------------------


class LoopbackCallback:
    """Local callback server that receives the OAuth redirect on loopback.

    Started *before* the browser is opened so the redirect cannot race the
    listener; ``wait()`` blocks until the code/state arrive or timeout.
    """

    def __init__(self, port: int, timeout: float) -> None:
        self._port = port
        self._timeout = timeout
        self._result: dict[str, str | None] = {}
        self._server: uvicorn.Server | None = None
        self._task: asyncio.Task[None] | None = None

    @property
    def redirect_uri(self) -> str:
        return f"http://127.0.0.1:{self._port}/callback"

    async def start(self) -> None:
        """Bind and listen on the loopback callback port."""
        from starlette.applications import Starlette
        from starlette.requests import Request
        from starlette.responses import PlainTextResponse
        from starlette.routing import Route

        async def callback(request: Request) -> PlainTextResponse:
            self._result["code"] = request.query_params.get("code")
            self._result["state"] = request.query_params.get("state")
            return PlainTextResponse("Authorization complete — you can close this tab and return to the gateway.")

        app = Starlette(routes=[Route("/callback", callback)])
        self._server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=self._port, log_level="warning"))
        self._task = asyncio.create_task(self._server.serve())
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self._server.started:
                return
            await asyncio.sleep(0.05)
        raise OAuthFlowError(f"could not start callback server on http://127.0.0.1:{self._port}/callback")

    async def wait(self) -> tuple[str, str | None]:
        """Block until the browser redirect arrives, then return (code, state)."""
        assert self._server is not None
        deadline = time.monotonic() + self._timeout
        while time.monotonic() < deadline:
            if "code" in self._result:
                code = self._result["code"]
                state = self._result["state"]
                return code or "", state
            await asyncio.sleep(0.1)
        raise OAuthFlowError(
            f"Authorization timed out after {self._timeout:.0f}s — no callback received on {self.redirect_uri}"
        )

    async def stop(self) -> None:
        """Shut the callback server down."""
        if self._server is not None:
            self._server.should_exit = True
        if self._task is not None:
            try:
                await asyncio.wait_for(self._task, timeout=5)
            except asyncio.TimeoutError:
                self._task.cancel()


async def _open_authorization_url(authorization_url: str, no_browser: bool, callback: LoopbackCallback) -> None:
    """Ensure the callback is listening, then surface the authorize URL.

    Printed to *stderr*: in gateway mode stdout carries the MCP protocol,
    so the human-facing prompt must not pollute the transport.
    """
    await callback.start()
    print(
        "\nAuthorize the SlopSearX gateway by visiting:\n"
        f"  {authorization_url}\n"
        f"(waiting for the redirect to {callback.redirect_uri} …)\n",
        file=sys.stderr,
        flush=True,
    )
    if not no_browser:
        try:
            import webbrowser

            webbrowser.open(authorization_url)
        except Exception:  # noqa: BLE001 — headless hosts just print the URL
            pass


# ---------------------------------------------------------------------------
# Client construction
# ---------------------------------------------------------------------------


def build_oauth_http_client(
    server_url: str,
    *,
    token_file: str | Path | None = None,
    callback_port: int = DEFAULT_CALLBACK_PORT,
    timeout: float = 300.0,
    no_browser: bool = False,
    redirect_handler: Callable[[str], Awaitable[None]] | None = None,
    callback_handler: Callable[[], Awaitable[tuple[str, str | None]]] | None = None,
) -> httpx.AsyncClient:
    """Build an httpx client that authenticates to the remote via OAuth.

    ``redirect_handler``/``callback_handler`` default to the loopback flow
    (browser + local callback server); tests and advanced setups may inject
    their own.
    """
    storage = FileTokenStorage(token_file or default_token_path(server_url))
    callback = LoopbackCallback(callback_port, timeout)
    metadata = OAuthClientMetadata(
        redirect_uris=[cast(Any, callback.redirect_uri)],
        token_endpoint_auth_method="none",
        grant_types=["authorization_code", "refresh_token"],
        response_types=["code"],
        client_name="slopsearx-mcp gateway",
    )
    redirect = redirect_handler or (lambda url: _open_authorization_url(url, no_browser, callback))
    callback_fn = callback_handler or callback.wait
    provider = OAuthClientProvider(
        server_url,
        metadata,
        storage,
        redirect_handler=redirect,
        callback_handler=callback_fn,
        timeout=timeout,
    )
    return httpx.AsyncClient(auth=provider, timeout=httpx.Timeout(30, read=300))
