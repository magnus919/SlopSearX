"""Dedicated OIDC relying party for human portal authentication."""

from __future__ import annotations

import asyncio
import base64
import hashlib
import hmac
import json
import secrets
import time
from dataclasses import dataclass
from typing import Any, Callable
from urllib.parse import parse_qs, urlencode, urlsplit

import httpx

from slopsearx.portal_auth import LoginStart, safe_return_to
from slopsearx.snapshot import KeyValueStore

TRANSACTION_PREFIX = "portal:oidc:v1"
TRANSACTION_TTL_SECONDS = 600
_SHA256_DIGEST_INFO = bytes.fromhex("3031300d060960864801650304020105000420")
_CLAIM_TRANSACTION = """
local current = redis.call('GET', KEYS[1])
if current and current == ARGV[1] then
  redis.call('DEL', KEYS[1])
  return 1
end
return 0
"""


def _decode_segment(value: str) -> bytes:
    return base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))


def _verify_rs256(token: str, jwks: dict[str, Any]) -> dict[str, Any]:
    """Verify a compact RS256 JWT against one matching RSA JWK."""
    try:
        encoded_header, encoded_claims, encoded_signature = token.split(".")
        header = json.loads(_decode_segment(encoded_header))
        claims = json.loads(_decode_segment(encoded_claims))
        signature = _decode_segment(encoded_signature)
    except (ValueError, TypeError, json.JSONDecodeError) as exc:
        raise PermissionError("invalid ID token") from exc
    if header.get("alg") != "RS256" or not isinstance(header.get("kid"), str):
        raise PermissionError("unsupported ID token algorithm")
    key = next(
        (
            item
            for item in jwks.get("keys", [])
            if isinstance(item, dict)
            and item.get("kid") == header["kid"]
            and item.get("kty") == "RSA"
            and item.get("use", "sig") == "sig"
        ),
        None,
    )
    if key is None:
        raise PermissionError("ID token signing key unavailable")
    try:
        modulus = int.from_bytes(_decode_segment(str(key["n"])), "big")
        exponent = int.from_bytes(_decode_segment(str(key["e"])), "big")
        size = (modulus.bit_length() + 7) // 8
        recovered = pow(int.from_bytes(signature, "big"), exponent, modulus).to_bytes(size, "big")
    except (KeyError, ValueError, OverflowError) as exc:
        raise PermissionError("invalid ID token signing key") from exc
    digest = hashlib.sha256(f"{encoded_header}.{encoded_claims}".encode()).digest()
    padding_size = size - len(_SHA256_DIGEST_INFO) - len(digest) - 3
    expected = b"\x00\x01" + b"\xff" * padding_size + b"\x00" + _SHA256_DIGEST_INFO + digest
    if padding_size < 8 or not hmac.compare_digest(recovered, expected):
        raise PermissionError("ID token signature rejected")
    if not isinstance(claims, dict):
        raise PermissionError("invalid ID token claims")
    return claims


def _validate_claims(
    claims: dict[str, Any], settings: "OIDCSettings", transaction: dict[str, Any], now: float
) -> tuple[str, str]:
    try:
        expires_at = float(claims["exp"])
        issued_at = float(claims["iat"])
        not_before = float(claims.get("nbf", issued_at))
    except (KeyError, TypeError, ValueError) as exc:
        raise PermissionError("OIDC time claims rejected") from exc
    issuer = str(claims.get("iss", "")).rstrip("/")
    audiences = claims.get("aud", [])
    if isinstance(audiences, str):
        audiences = [audiences]
    subject = claims.get("sub")
    if (
        not isinstance(audiences, list)
        or issuer != settings.issuer.rstrip("/")
        or settings.client_id not in audiences
        or (len(audiences) > 1 and claims.get("azp") != settings.client_id)
        or not isinstance(subject, str)
        or not subject
        or now >= expires_at + 30
        or now + 30 < not_before
        or issued_at > now + 30
        or not hmac.compare_digest(str(claims.get("nonce", "")), str(transaction["nonce"]))
    ):
        raise PermissionError("OIDC claims rejected")
    return issuer, subject


@dataclass(frozen=True)
class OIDCSettings:
    issuer: str
    client_id: str
    client_secret: str
    callback_url: str

    def __post_init__(self) -> None:
        issuer = urlsplit(self.issuer)
        callback = urlsplit(self.callback_url)
        if issuer.scheme != "https" or not issuer.netloc or issuer.query or issuer.fragment:
            raise ValueError("OIDC issuer must be an HTTPS origin/path without query or fragment")
        if (
            callback.scheme != "https"
            or not callback.netloc
            or callback.path != "/auth/callback"
            or callback.query
            or callback.fragment
        ):
            raise ValueError("OIDC callback must be the exact HTTPS /auth/callback URL")
        if not self.client_id or not self.client_secret:
            raise ValueError("OIDC client credentials are required")


class OIDCIdentityProvider:
    """Authorization Code + PKCE relying party with one-time Valkey state."""

    _locks: dict[int, asyncio.Lock] = {}

    def __init__(
        self,
        settings: OIDCSettings,
        store: KeyValueStore,
        *,
        transaction_key: bytes,
        client: httpx.AsyncClient | None = None,
        clock: Callable[[], float] = time.time,
        token_source: Callable[[int], str] = secrets.token_urlsafe,
        previous_transaction_key: bytes | None = None,
        previous_key_drain_seconds: int = 0,
    ) -> None:
        if len(transaction_key) < 32:
            raise ValueError("OIDC transaction HMAC key must contain at least 32 bytes")
        self.settings = settings
        self.store = store
        self._key = transaction_key
        self._client = client or httpx.AsyncClient(timeout=10, follow_redirects=False)
        self._clock = clock
        self._token_source = token_source
        self._metadata: dict[str, Any] | None = None
        self._jwks: dict[str, Any] | None = None
        self._transaction_lock = self._locks.setdefault(id(store), asyncio.Lock())
        if previous_transaction_key is not None and len(previous_transaction_key) < 32:
            raise ValueError("previous OIDC transaction key must contain at least 32 bytes")
        if previous_transaction_key is not None and not 60 <= previous_key_drain_seconds <= TRANSACTION_TTL_SECONDS:
            raise ValueError("previous OIDC key drain must be between 60 and 600 seconds")
        self._previous_key = previous_transaction_key
        self._previous_key_until = self._clock() + previous_key_drain_seconds if previous_transaction_key else 0

    def _transaction_key(self, handle: str, key: bytes | None = None) -> str:
        digest = hmac.new(key or self._key, handle.encode(), hashlib.sha256).hexdigest()
        return f"{TRANSACTION_PREFIX}:{digest}"

    async def _delete(self, key: str) -> None:
        delete = getattr(self.store, "delete", None)
        if delete is not None:
            await delete(key)
            return
        client = getattr(self.store, "_client", None)
        if client is not None:
            await client.delete(key)

    async def _read_transaction(self, key: str) -> tuple[dict[str, Any] | None, str | None]:
        """Read a transaction and retain its exact Valkey representation for CAS."""
        client = getattr(self.store, "_client", None)
        if client is not None:
            raw = await client.get(key)
            if raw is None:
                return None, None
            if isinstance(raw, bytes):
                raw = raw.decode()
            value = json.loads(raw) if isinstance(raw, str) else raw
            return (value, raw) if isinstance(value, dict) and isinstance(raw, str) else (None, None)
        value = await self.store.get(key)
        return (value, None) if isinstance(value, dict) else (None, None)

    async def _consume_transaction(self, key: str, expected: dict[str, Any], raw: str | None = None) -> bool:
        """Atomically delete only the transaction that was already validated."""
        client = getattr(self.store, "_client", None)
        if client is not None:
            if raw is None:
                return False
            return bool(await client.eval(_CLAIM_TRANSACTION, 1, key, raw))
        async with self._transaction_lock:
            value = await self.store.get(key)
            if value != expected:
                return False
            await self._delete(key)
            return True

    async def _discover(self) -> dict[str, Any]:
        if self._metadata is not None:
            return self._metadata
        url = self.settings.issuer.rstrip("/") + "/.well-known/openid-configuration"
        response = await self._client.get(url)
        response.raise_for_status()
        raw_metadata = response.json()
        if not isinstance(raw_metadata, dict):
            raise PermissionError("OIDC metadata is invalid")
        metadata: dict[str, Any] = raw_metadata
        if metadata.get("issuer", "").rstrip("/") != self.settings.issuer.rstrip("/"):
            raise PermissionError("OIDC issuer mismatch")
        for field in ("authorization_endpoint", "token_endpoint", "jwks_uri"):
            parsed = urlsplit(str(metadata.get(field, "")))
            if parsed.scheme != "https" or not parsed.netloc:
                raise PermissionError("OIDC metadata endpoint is invalid")
        if "S256" not in metadata.get("code_challenge_methods_supported", ["S256"]):
            raise PermissionError("OIDC provider does not support PKCE S256")
        self._metadata = metadata
        return metadata

    async def begin(self, return_to: str) -> LoginStart:
        if not self.store.is_connected:
            raise RuntimeError("OIDC transaction store unavailable")
        metadata = await self._discover()
        state = self._token_source(32)
        nonce = self._token_source(32)
        verifier = self._token_source(48)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
        transaction = {
            "state": state,
            "nonce": nonce,
            "verifier": verifier,
            "return_to": safe_return_to(return_to),
            "created_at": self._clock(),
        }
        handle = ""
        for _attempt in range(3):
            candidate = self._token_source(32)
            key = self._transaction_key(candidate)
            set_nx = getattr(self.store, "set_nx", None)
            if set_nx is not None:
                created = bool(await set_nx(key, transaction, TRANSACTION_TTL_SECONDS))
            else:
                client = getattr(self.store, "_client", None)
                created = bool(
                    client is not None
                    and await client.set(
                        key,
                        json.dumps(transaction, separators=(",", ":")),
                        ex=TRANSACTION_TTL_SECONDS,
                        nx=True,
                    )
                )
            if created:
                handle = candidate
                break
        if not handle:
            raise RuntimeError("OIDC transaction handle collision")
        query = urlencode(
            {
                "response_type": "code",
                "client_id": self.settings.client_id,
                "redirect_uri": self.settings.callback_url,
                "scope": "openid",
                "state": state,
                "nonce": nonce,
                "code_challenge": challenge,
                "code_challenge_method": "S256",
            }
        )
        return LoginStart(f"{metadata['authorization_endpoint']}?{query}", handle)

    async def complete(self, request_url: str, transaction_handle: str | None) -> tuple[str, str, str]:
        if not transaction_handle or not self.store.is_connected:
            raise PermissionError("missing login transaction")
        values = parse_qs(urlsplit(request_url).query, keep_blank_values=True, max_num_fields=8)
        if len(values.get("code", [])) != 1 or len(values.get("state", [])) != 1:
            raise PermissionError("OIDC callback parameters rejected")
        code = values["code"][0]
        state = values["state"][0]
        if not code or not state:
            raise PermissionError("OIDC callback parameters rejected")
        keys = [self._transaction_key(transaction_handle)]
        if self._previous_key is not None and self._clock() < self._previous_key_until:
            keys.append(self._transaction_key(transaction_handle, self._previous_key))
        transaction = None
        for key in keys:
            candidate, raw = await self._read_transaction(key)
            if candidate is None:
                continue
            if self._clock() - float(candidate.get("created_at", 0)) > TRANSACTION_TTL_SECONDS:
                await self._consume_transaction(key, candidate, raw)
                raise PermissionError("expired login transaction")
            if not hmac.compare_digest(state, str(candidate.get("state", ""))):
                raise PermissionError("OIDC state mismatch")
            if not await self._consume_transaction(key, candidate, raw):
                raise PermissionError("expired login transaction")
            transaction = candidate
            break
        if transaction is None:
            raise PermissionError("expired login transaction")
        metadata = await self._discover()
        response = await self._client.post(
            str(metadata["token_endpoint"]),
            data={
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": self.settings.callback_url,
                "code_verifier": str(transaction["verifier"]),
            },
            auth=(self.settings.client_id, self.settings.client_secret),
            headers={"accept": "application/json"},
        )
        response.raise_for_status()
        id_token = response.json().get("id_token")
        if not isinstance(id_token, str) or not id_token:
            raise PermissionError("OIDC response omitted id_token")
        if self._jwks is None:
            jwks_response = await self._client.get(str(metadata["jwks_uri"]))
            jwks_response.raise_for_status()
            self._jwks = jwks_response.json()
        claims = _verify_rs256(id_token, self._jwks)
        issuer, subject = _validate_claims(claims, self.settings, transaction, self._clock())
        return issuer, subject, safe_return_to(str(transaction.get("return_to", "/workflows")))
