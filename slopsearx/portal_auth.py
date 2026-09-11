"""Browser identity, membership, and opaque session contracts for the portal.

This module deliberately contains no FastAPI or MCP code.  The browser
transport resolves a session into :class:`PortalAuthContext`; workflow
services receive that normalized context and never trust form-supplied tenant
or grant values.
"""

from __future__ import annotations

import asyncio
import hashlib
import hmac
import json
import secrets
import time
from dataclasses import asdict, dataclass
from typing import Any, Callable, Protocol

from slopsearx.snapshot import KeyValueStore

SESSION_PREFIX = "portal:session:v1"
SESSION_IDLE_SECONDS = 1_800
SESSION_ABSOLUTE_SECONDS = 28_800
_ROTATE_SCRIPT = """
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if tonumber(current.generation) ~= tonumber(ARGV[1]) or redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
redis.call('SETEX', KEYS[2], ARGV[2], ARGV[3])
redis.call('DEL', KEYS[1])
return 1
"""
_REFRESH_SCRIPT = """
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if tonumber(current.generation) ~= tonumber(ARGV[1]) then return 0 end
if tonumber(current.refresh_revision or 0) ~= tonumber(ARGV[2]) then return 0 end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
return 1
"""
_REPLACE_SCRIPT = """
if KEYS[1] == KEYS[2] or KEYS[1] == KEYS[3] then return 0 end
if redis.call('EXISTS', KEYS[1]) == 1 then return 0 end
redis.call('SETEX', KEYS[1], ARGV[1], ARGV[2])
redis.call('DEL', KEYS[2])
redis.call('DEL', KEYS[3])
return 1
"""


@dataclass(frozen=True)
class PortalMembership:
    tenant_id: str
    display_name: str
    grants: frozenset[str]
    revision: int = 1
    enabled: bool = True


@dataclass(frozen=True)
class PortalPrincipal:
    principal_id: str
    issuer: str
    subject: str
    memberships: tuple[PortalMembership, ...]
    revision: int = 1
    enabled: bool = True


@dataclass(frozen=True)
class PortalAuthContext:
    principal_id: str
    tenant_id: str
    tenant_label: str
    grants: frozenset[str]
    principal_revision: int
    membership_revision: int
    session_generation: int
    csrf_token: str

    def allows(self, action: str) -> bool:
        return action in self.grants


class MembershipResolver(Protocol):
    async def resolve_subject(self, issuer: str, subject: str) -> PortalPrincipal | None: ...

    async def resolve_principal(self, principal_id: str) -> PortalPrincipal | None: ...


@dataclass(frozen=True)
class LoginStart:
    location: str
    transaction_handle: str


class IdentityProvider(Protocol):
    async def begin(self, return_to: str) -> LoginStart: ...

    async def complete(self, request_url: str, transaction_handle: str | None) -> tuple[str, str, str]: ...


class StaticMembershipResolver:
    """Immutable operator-provided directory suitable for production wiring and tests."""

    def __init__(self, principals: tuple[PortalPrincipal, ...]) -> None:
        principal_ids = [item.principal_id for item in principals]
        subjects = [(item.issuer.rstrip("/"), item.subject) for item in principals]
        if len(set(principal_ids)) != len(principal_ids) or len(set(subjects)) != len(subjects):
            raise ValueError("portal principals must have unique IDs and issuer subjects")
        for item in principals:
            tenant_ids = [membership.tenant_id for membership in item.memberships]
            if len(set(tenant_ids)) != len(tenant_ids):
                raise ValueError("portal memberships must have unique tenant IDs per principal")
        self._by_id = {item.principal_id: item for item in principals}
        self._by_subject = {(item.issuer.rstrip("/"), item.subject): item for item in principals}

    async def resolve_subject(self, issuer: str, subject: str) -> PortalPrincipal | None:
        return self._by_subject.get((issuer.rstrip("/"), subject))

    async def resolve_principal(self, principal_id: str) -> PortalPrincipal | None:
        return self._by_id.get(principal_id)


@dataclass
class PortalSession:
    principal_id: str
    issuer: str
    active_tenant: str | None
    authenticated_at: float
    created_at: float
    last_seen_at: float
    idle_expires_at: float
    absolute_expires_at: float
    principal_revision: int
    membership_revision: int
    csrf_secret: str
    generation: int = 1
    refresh_revision: int = 0


@dataclass(frozen=True)
class SessionResolution:
    context: PortalAuthContext | None = None
    session: PortalSession | None = None
    expired: bool = False
    revoked: bool = False
    unavailable: bool = False


class PortalSessionStore:
    """Valkey-backed opaque browser sessions with revision revalidation."""

    _locks: dict[int, asyncio.Lock] = {}

    def __init__(
        self,
        store: KeyValueStore | None,
        resolver: MembershipResolver,
        *,
        hmac_key: bytes,
        clock: Callable[[], float] = time.time,
        token_source: Callable[[int], str] = secrets.token_urlsafe,
        idle_seconds: int = SESSION_IDLE_SECONDS,
        absolute_seconds: int = SESSION_ABSOLUTE_SECONDS,
        previous_hmac_key: bytes | None = None,
        previous_key_drain_seconds: int = 0,
    ) -> None:
        if len(hmac_key) < 32:
            raise ValueError("portal session HMAC key must contain at least 32 bytes")
        if not 60 <= idle_seconds <= SESSION_ABSOLUTE_SECONDS:
            raise ValueError("portal idle timeout must be between 60 seconds and 8 hours")
        if not idle_seconds <= absolute_seconds <= SESSION_ABSOLUTE_SECONDS:
            raise ValueError("portal absolute timeout must be between idle timeout and 8 hours")
        self._store = store
        self._resolver = resolver
        self._hmac_key = hmac_key
        self._clock = clock
        self._token_source = token_source
        self._idle = idle_seconds
        self._absolute = absolute_seconds
        if previous_hmac_key is not None and len(previous_hmac_key) < 32:
            raise ValueError("previous portal session HMAC key must contain at least 32 bytes")
        if previous_hmac_key is not None and not 60 <= previous_key_drain_seconds <= SESSION_ABSOLUTE_SECONDS:
            raise ValueError("previous portal key drain must be between 60 seconds and 8 hours")
        self._previous_hmac_key = previous_hmac_key
        self._previous_key_until = self._clock() + previous_key_drain_seconds if previous_hmac_key else 0
        self._lock = self._locks.setdefault(id(store), asyncio.Lock())

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    @staticmethod
    def _digest_with(key: bytes, handle: str) -> str:
        return hmac.new(key, handle.encode(), hashlib.sha256).hexdigest()

    def _digest(self, handle: str) -> str:
        return self._digest_with(self._hmac_key, handle)

    def _key(self, handle: str) -> str:
        return f"{SESSION_PREFIX}:{self._digest(handle)}"

    def _candidate_keys(self, handle: str) -> tuple[str, ...]:
        current = self._key(handle)
        if self._previous_hmac_key is None or self._clock() >= self._previous_key_until:
            return (current,)
        previous = f"{SESSION_PREFIX}:{self._digest_with(self._previous_hmac_key, handle)}"
        return (current, previous)

    async def _lookup(self, handle: str) -> tuple[str, dict[str, Any] | None]:
        for key in self._candidate_keys(handle):
            payload = await self._store.get(key)  # type: ignore[union-attr]
            if payload is not None:
                return key, payload
        return self._key(handle), None

    async def _delete(self, key: str) -> None:
        store = self._store
        if store is None:
            return
        delete = getattr(store, "delete", None)
        if delete is not None:
            await delete(key)
            return
        client = getattr(store, "_client", None)
        if client is not None:
            await client.delete(key)

    async def create(self, principal: PortalPrincipal, active_tenant: str | None) -> tuple[str, PortalSession]:
        if not self.available:
            raise RuntimeError("portal session store unavailable")
        now = self._clock()
        membership = next((item for item in principal.memberships if item.tenant_id == active_tenant), None)
        session = PortalSession(
            principal_id=principal.principal_id,
            issuer=principal.issuer,
            active_tenant=active_tenant,
            authenticated_at=now,
            created_at=now,
            last_seen_at=now,
            idle_expires_at=now + self._idle,
            absolute_expires_at=now + self._absolute,
            principal_revision=principal.revision,
            membership_revision=membership.revision if membership else 0,
            csrf_secret=self._token_source(32),
        )
        for _attempt in range(3):
            handle = self._token_source(32)
            key = self._key(handle)
            set_nx = getattr(self._store, "set_nx", None)
            if set_nx is not None:
                created = bool(await set_nx(key, asdict(session), self._absolute))
            else:
                client = getattr(self._store, "_client", None)
                created = bool(
                    client is not None
                    and await client.set(key, json.dumps(asdict(session)), ex=self._absolute, nx=True)
                )
            if created:
                return handle, session
        raise RuntimeError("portal session handle collision")

    async def replace(
        self, old_handle: str | None, principal: PortalPrincipal, active_tenant: str | None
    ) -> tuple[str, PortalSession]:
        """Atomically issue a fresh login session and invalidate a presented handle."""
        if not self.available:
            raise RuntimeError("portal session store unavailable")
        now = self._clock()
        membership = next((item for item in principal.memberships if item.tenant_id == active_tenant), None)
        session = PortalSession(
            principal_id=principal.principal_id,
            issuer=principal.issuer,
            active_tenant=active_tenant,
            authenticated_at=now,
            created_at=now,
            last_seen_at=now,
            idle_expires_at=now + self._idle,
            absolute_expires_at=now + self._absolute,
            principal_revision=principal.revision,
            membership_revision=membership.revision if membership else 0,
            csrf_secret=self._token_source(32),
        )
        old_keys = self._candidate_keys(old_handle or "")
        old_key_one = old_keys[0]
        old_key_two = old_keys[-1]
        for _attempt in range(3):
            handle = self._token_source(32)
            new_key = self._key(handle)
            client = getattr(self._store, "_client", None)
            if client is not None and hasattr(client, "eval"):
                created = bool(
                    await client.eval(
                        _REPLACE_SCRIPT,
                        3,
                        new_key,
                        old_key_one,
                        old_key_two,
                        str(self._absolute),
                        json.dumps(asdict(session), separators=(",", ":")),
                    )
                )
            else:
                async with self._lock:
                    if new_key in {old_key_one, old_key_two} or await self._store.get(new_key) is not None:  # type: ignore[union-attr]
                        created = False
                    else:
                        await self._store.set(new_key, asdict(session), self._absolute)  # type: ignore[union-attr]
                        await self._delete(old_key_one)
                        if old_key_two != old_key_one:
                            await self._delete(old_key_two)
                        created = True
            if created:
                return handle, session
        raise RuntimeError("portal session handle collision")

    async def revoke(self, handle: str) -> None:
        for key in self._candidate_keys(handle):
            await self._delete(key)

    async def rotate(self, handle: str, session: PortalSession, active_tenant: str) -> tuple[str, PortalSession]:
        principal = await self._resolver.resolve_principal(session.principal_id)
        if principal is None:
            raise PermissionError("principal unavailable")
        membership = next(
            (item for item in principal.memberships if item.tenant_id == active_tenant and item.enabled), None
        )
        if membership is None:
            raise PermissionError("membership unavailable")
        now = self._clock()
        if now >= session.absolute_expires_at:
            raise PermissionError("session expired")
        rotated_ttl = max(1, int(session.absolute_expires_at - now))
        rotated = PortalSession(
            principal_id=principal.principal_id,
            issuer=principal.issuer,
            active_tenant=active_tenant,
            authenticated_at=session.authenticated_at,
            created_at=session.created_at,
            last_seen_at=now,
            idle_expires_at=min(now + self._idle, session.absolute_expires_at),
            absolute_expires_at=session.absolute_expires_at,
            principal_revision=principal.revision,
            membership_revision=membership.revision,
            csrf_secret=self._token_source(32),
            generation=session.generation + 1,
        )
        new_handle = self._token_source(32)
        old_key, current_payload = await self._lookup(handle)
        if current_payload is None:
            raise PermissionError("session changed")
        new_key = self._key(new_handle)
        client = getattr(self._store, "_client", None)
        if client is not None and hasattr(client, "eval"):
            changed = bool(
                await client.eval(
                    _ROTATE_SCRIPT,
                    2,
                    old_key,
                    new_key,
                    str(session.generation),
                    str(rotated_ttl),
                    json.dumps(asdict(rotated), separators=(",", ":")),
                )
            )
        else:
            async with self._lock:
                current = await self._store.get(old_key)  # type: ignore[union-attr]
                if not current or int(current.get("generation", 0)) != session.generation:
                    changed = False
                elif await self._store.get(new_key) is not None:  # type: ignore[union-attr]
                    changed = False
                else:
                    await self._store.set(new_key, asdict(rotated), rotated_ttl)  # type: ignore[union-attr]
                    await self._delete(old_key)
                    changed = True
        if not changed:
            raise PermissionError("session changed")
        return new_handle, rotated

    async def _refresh(self, key: str, previous: PortalSession, updated: PortalSession, ttl: int) -> bool:
        client = getattr(self._store, "_client", None)
        if client is not None and hasattr(client, "eval"):
            return bool(
                await client.eval(
                    _REFRESH_SCRIPT,
                    1,
                    key,
                    str(previous.generation),
                    str(previous.refresh_revision),
                    str(ttl),
                    json.dumps(asdict(updated), separators=(",", ":")),
                )
            )
        async with self._lock:
            current = await self._store.get(key)  # type: ignore[union-attr]
            if (
                not current
                or int(current.get("generation", 0)) != previous.generation
                or int(current.get("refresh_revision", 0)) != previous.refresh_revision
            ):
                return False
            await self._store.set(key, asdict(updated), ttl)  # type: ignore[union-attr]
            return True

    async def resolve(self, handle: str | None, operator_actions: frozenset[str]) -> SessionResolution:
        if not handle:
            return SessionResolution()
        if not self.available:
            return SessionResolution(unavailable=True)
        session_key, payload = await self._lookup(handle)
        if payload is None:
            return SessionResolution()
        try:
            session = PortalSession(**payload)
        except (TypeError, ValueError):
            await self.revoke(handle)
            return SessionResolution(revoked=True)
        now = self._clock()
        if now >= min(session.idle_expires_at, session.absolute_expires_at):
            await self.revoke(handle)
            return SessionResolution(expired=True)
        principal = await self._resolver.resolve_principal(session.principal_id)
        if principal is None or not principal.enabled or principal.revision != session.principal_revision:
            await self.revoke(handle)
            return SessionResolution(revoked=True)
        if session.active_tenant is None:
            return SessionResolution(session=session)
        membership = next(
            (item for item in principal.memberships if item.tenant_id == session.active_tenant and item.enabled), None
        )
        if membership is None or membership.revision != session.membership_revision:
            await self.revoke(handle)
            return SessionResolution(revoked=True)
        # Sliding idle time never moves the absolute ceiling.
        previous = PortalSession(**asdict(session))
        session.last_seen_at = now
        session.idle_expires_at = min(now + self._idle, session.absolute_expires_at)
        session.refresh_revision += 1
        ttl = max(1, int(session.absolute_expires_at - now))
        if not await self._refresh(session_key, previous, session, ttl):
            latest_payload = await self._store.get(session_key)  # type: ignore[union-attr]
            if latest_payload is None:
                return SessionResolution(revoked=True)
            try:
                latest = PortalSession(**latest_payload)
            except (TypeError, ValueError):
                return SessionResolution(revoked=True)
            if (
                latest.generation != previous.generation
                or latest.principal_revision != principal.revision
                or latest.membership_revision != membership.revision
                or now >= min(latest.idle_expires_at, latest.absolute_expires_at)
            ):
                return SessionResolution(revoked=True)
            session = latest
        effective = membership.grants & operator_actions
        token = hmac.new(session.csrf_secret.encode(), b"portal-workflow-action-v1", hashlib.sha256).hexdigest()
        return SessionResolution(
            context=PortalAuthContext(
                principal_id=principal.principal_id,
                tenant_id=membership.tenant_id,
                tenant_label=membership.display_name,
                grants=frozenset(effective),
                principal_revision=principal.revision,
                membership_revision=membership.revision,
                session_generation=session.generation,
                csrf_token=token,
            ),
            session=session,
        )


def valid_csrf(context: PortalAuthContext, submitted: str | None) -> bool:
    return submitted is not None and bool(submitted) and hmac.compare_digest(context.csrf_token, submitted)


def pending_csrf(session: PortalSession) -> str:
    return hmac.new(session.csrf_secret.encode(), b"portal-tenant-choice-v1", hashlib.sha256).hexdigest()


def safe_return_to(value: str | None) -> str:
    """Return one canonical local workflow path or the workflow root."""
    if not value or not value.startswith("/workflows"):
        return "/workflows"
    if value.startswith("//") or "\\" in value or any(ord(char) < 32 for char in value):
        return "/workflows"
    return value if value == "/workflows" or value.startswith("/workflows/") else "/workflows"
