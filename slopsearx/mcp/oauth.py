"""MCP OAuth 2.1 authorization-server support.

SlopSearX has no user accounts, so authorization is granted automatically
to registered clients — the OAuth equivalent of possessing the server
token. The MCP SDK's auth handlers enforce the protocol checks (PKCE
verification, redirect_uri matching, client credentials, scope
validation); this module provides the storage and token-minting half:

- :class:`SlopSearxOAuthProvider` implements the
  ``OAuthAuthorizationServerProvider`` protocol for the FastMCP auth
  layer, backed by Valkey when available (multi-replica correctness) and
  an in-memory dict otherwise (single process).
- :func:`oauth_settings_from_policy` builds the ``AuthSettings`` from the
  operator policy so ``create_server`` can enable OAuth mode.

When OAuth is enabled, the server speaks standard MCP OAuth 2.1 with
dynamic client registration (RFC 7591), which OAuth-requiring clients
such as Claude Web connectors expect. Static bearer-token auth remains
available as the alternative mode.
"""

from __future__ import annotations

import json
import logging
import secrets
import time
from typing import Any, cast

from mcp.server.auth.provider import (
    AccessToken,
    AuthorizationCode,
    AuthorizationParams,
    RefreshToken,
    construct_redirect_uri,
)
from mcp.server.auth.settings import AuthSettings
from mcp.shared.auth import OAuthClientInformationFull, OAuthToken

from slopsearx.capabilities import MCPPolicy

logger = logging.getLogger(__name__)

_STATE_PREFIX = "mcp:oauth"
_CLIENT_TTL_SECONDS = 30 * 24 * 3600  # 30 days


class OAuthStateStore:
    """KV storage for OAuth state (clients, codes, tokens).

    Uses the shared Valkey client when one is available, so tokens and
    registrations survive across replicas; otherwise falls back to an
    in-memory dict (single process). Graceful degradation: Valkey
    failures fall back to memory.
    """

    def __init__(self, cache: Any = None) -> None:
        self._client: Any = None
        self._mem: dict[str, dict[str, Any]] = {}
        self.bind(cache)

    def bind(self, cache: Any) -> None:
        """Attach the shared cache (or None for memory-only)."""
        self._client = getattr(cache, "_client", None) if cache is not None else None

    def _key(self, kind: str, name: str) -> str:
        return f"{_STATE_PREFIX}:{kind}:{name}"

    async def get(self, kind: str, name: str) -> dict[str, Any] | None:
        key = self._key(kind, name)
        if self._client is not None:
            try:
                raw = await self._client.get(key)
                if raw is None:
                    return None
                if isinstance(raw, bytes):
                    raw = raw.decode()
                return cast("dict[str, Any] | None", json.loads(raw))
            except Exception as exc:  # noqa: BLE001 — graceful degradation
                logger.warning("OAuthStateStore: Valkey read failed, using memory: %s", exc)
        return self._mem.get(key)

    async def set(self, kind: str, name: str, value: dict[str, Any], ttl: int) -> None:
        key = self._key(kind, name)
        if self._client is not None:
            try:
                await self._client.setex(key, ttl, json.dumps(value))
                return
            except Exception as exc:  # noqa: BLE001
                logger.warning("OAuthStateStore: Valkey write failed, using memory: %s", exc)
        self._mem[key] = value

    async def delete(self, kind: str, name: str) -> None:
        key = self._key(kind, name)
        if self._client is not None:
            try:
                await self._client.delete(key)
                return
            except Exception as exc:  # noqa: BLE001
                logger.warning("OAuthStateStore: Valkey delete failed, using memory: %s", exc)
        self._mem.pop(key, None)


class SlopSearxOAuthProvider:
    """Auto-approving OAuth 2.1 authorization server for the MCP server.

    Implements the ``OAuthAuthorizationServerProvider`` protocol used by
    FastMCP's auth layer. The SDK handlers validate the protocol
    (PKCE, redirect_uri, client credentials, scopes); this provider only
    stores registered clients, authorization codes, and tokens, and mints
    opaque access/refresh tokens.
    """

    def __init__(
        self,
        cache: Any = None,
        *,
        access_token_ttl_seconds: int = 3600,
        refresh_token_ttl_seconds: int = 2_592_000,  # 30 days
        authorization_code_ttl_seconds: int = 600,
        subject: str = "operator",
    ) -> None:
        self._store = OAuthStateStore(cache)
        self._access_ttl = access_token_ttl_seconds
        self._refresh_ttl = refresh_token_ttl_seconds
        self._code_ttl = authorization_code_ttl_seconds
        self._subject = subject

    def bind(self, cache: Any) -> None:
        """Attach the shared cache once the runtime is built (lifespan)."""
        self._store.bind(cache)

    # -- clients ---------------------------------------------------------

    async def get_client(self, client_id: str) -> OAuthClientInformationFull | None:
        data = await self._store.get("client", client_id)
        return OAuthClientInformationFull.model_validate(data) if data else None

    async def register_client(self, client_info: OAuthClientInformationFull) -> None:
        await self._store.set(
            "client", client_info.client_id or "", client_info.model_dump(mode="json"), ttl=_CLIENT_TTL_SECONDS
        )

    # -- authorization ---------------------------------------------------

    async def authorize(self, client: OAuthClientInformationFull, params: AuthorizationParams) -> str:
        code = secrets.token_urlsafe(32)
        auth_code = AuthorizationCode(
            code=code,
            scopes=list(params.scopes or []),
            expires_at=time.time() + self._code_ttl,
            client_id=client.client_id or "",
            code_challenge=params.code_challenge,
            redirect_uri=params.redirect_uri,
            redirect_uri_provided_explicitly=params.redirect_uri_provided_explicitly,
            resource=params.resource,
            subject=self._subject,
        )
        await self._store.set("code", code, auth_code.model_dump(mode="json"), ttl=self._code_ttl)
        # The MCP SDK's helper is untyped (returns Any); the redirect URI is a string.
        return cast(str, construct_redirect_uri(str(params.redirect_uri), code=code, state=params.state))

    async def load_authorization_code(
        self, client: OAuthClientInformationFull, authorization_code: str
    ) -> AuthorizationCode | None:
        data = await self._store.get("code", authorization_code)
        if not data:
            return None
        code = AuthorizationCode.model_validate(data)
        if code.client_id != client.client_id or code.expires_at < time.time():
            return None
        return code

    async def exchange_authorization_code(
        self, client: OAuthClientInformationFull, authorization_code: AuthorizationCode
    ) -> OAuthToken:
        access, refresh = await self._issue_tokens(
            client.client_id or "", authorization_code.scopes, authorization_code.subject
        )
        await self._store.delete("code", authorization_code.code)
        return self._token_response(access, refresh)

    # -- refresh tokens ---------------------------------------------------

    async def load_refresh_token(self, client: OAuthClientInformationFull, refresh_token: str) -> RefreshToken | None:
        data = await self._store.get("refresh", refresh_token)
        if not data:
            return None
        token = RefreshToken.model_validate(data)
        if token.client_id != client.client_id:
            return None
        if token.expires_at is not None and token.expires_at < time.time():
            return None
        return token

    async def exchange_refresh_token(
        self,
        client: OAuthClientInformationFull,
        refresh_token: RefreshToken,
        scopes: list[str],
    ) -> OAuthToken:
        scopes = list(scopes) if scopes else refresh_token.scopes
        access, refresh = await self._issue_tokens(client.client_id or "", scopes, refresh_token.subject)
        await self.revoke_token(refresh_token)  # rotate
        return self._token_response(access, refresh)

    # -- access tokens -----------------------------------------------------

    async def load_access_token(self, token: str) -> AccessToken | None:
        data = await self._store.get("access", token)
        if not data:
            return None
        access = AccessToken.model_validate(data)
        if access.expires_at is not None and access.expires_at < time.time():
            return None
        return access

    async def revoke_token(self, token: AccessToken | RefreshToken) -> None:
        kind = "refresh" if isinstance(token, RefreshToken) else "access"
        data = await self._store.get(kind, token.token)
        if not data:
            return
        other = data.get("access_token" if kind == "refresh" else "refresh_token")
        await self._store.delete(kind, token.token)
        if other:
            await self._store.delete("refresh" if kind == "access" else "access", other)

    # -- helpers ------------------------------------------------------------

    async def _issue_tokens(
        self, client_id: str, scopes: list[str], subject: str | None
    ) -> tuple[AccessToken, RefreshToken]:
        access_str = secrets.token_urlsafe(32)
        refresh_str = secrets.token_urlsafe(40)
        now = int(time.time())
        access = AccessToken(
            token=access_str,
            client_id=client_id,
            scopes=scopes,
            expires_at=now + self._access_ttl,
            resource=None,
            subject=subject,
        )
        refresh = RefreshToken(
            token=refresh_str,
            client_id=client_id,
            scopes=scopes,
            expires_at=now + self._refresh_ttl,
            subject=subject,
        )
        access_payload = access.model_dump(mode="json")
        access_payload["refresh_token"] = refresh_str
        refresh_payload = refresh.model_dump(mode="json")
        refresh_payload["access_token"] = access_str
        await self._store.set("access", access_str, access_payload, ttl=self._access_ttl)
        await self._store.set("refresh", refresh_str, refresh_payload, ttl=self._refresh_ttl)
        return access, refresh

    def _token_response(self, access: AccessToken, refresh: RefreshToken) -> OAuthToken:
        return OAuthToken(
            access_token=access.token,
            token_type="Bearer",
            expires_in=self._access_ttl,
            scope=" ".join(access.scopes) or None,
            refresh_token=refresh.token,
        )


def oauth_settings_from_policy(
    policy: MCPPolicy,
) -> tuple[AuthSettings | None, SlopSearxOAuthProvider | None]:
    """Build FastMCP auth settings and a provider from the operator policy.

    Returns ``(None, None)`` when OAuth is disabled. Raises ``ValueError``
    when enabled without an issuer URL (it cannot be advertised).
    """
    if not policy.oauth_enabled:
        return None, None
    if not policy.oauth_issuer_url:
        raise ValueError("mcp.oauth.enabled requires mcp.oauth.issuer_url (or MCP_OAUTH_ISSUER_URL)")

    issuer = policy.oauth_issuer_url.rstrip("/")
    # model_validate coerces strings to AnyHttpUrl (pydantic) without needing
    # a direct AnyHttpUrl import here.
    settings = AuthSettings.model_validate(
        {
            "issuer_url": issuer,
            "resource_server_url": f"{issuer}/mcp",
            "service_documentation_url": policy.oauth_service_documentation_url or None,
            "client_registration_options": {"enabled": True},
            "revocation_options": {"enabled": True},
        }
    )
    provider = SlopSearxOAuthProvider(
        None,
        access_token_ttl_seconds=policy.oauth_access_token_ttl_seconds,
        refresh_token_ttl_seconds=policy.oauth_refresh_token_ttl_seconds,
    )
    return settings, provider
