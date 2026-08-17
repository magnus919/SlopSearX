"""Opaque search snapshot store — stable pagination over captured evidence.

A snapshot captures the merged, ranked result set of one search so later
pages read from the *same* evidence instead of re-executing the query
against potentially changing engines. Cursors are opaque, tenant-bound
handles issued by the server; clients never supply arbitrary URLs or
queries to pagination tools.

Snapshots live in Valkey (via the shared :class:`~slopsearx.cache.SearchCache`)
with a bounded TTL and are immutable once written.
"""

from __future__ import annotations

import dataclasses
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Protocol

from slopsearx.adapter import SearchResult
from slopsearx.service import ScopeDecision, search_result_from_dict, search_result_to_dict

SNAPSHOT_KEY_PREFIX = "mcp:snapshot"

# How much longer the backing-store key lives than the snapshot's logical
# ``expires_at`` horizon. The store TTL must EXCEED the ``expires_at`` offset
# so the payload is still present (and classified ``expired``) on real Valkey
# after ``expires_at`` passes — otherwise the key is evicted at the exact
# moment it becomes "expired" and ``read()`` would surface ``unknown``
# instead of ``expired_handle`` (VAL-EXPAND-015).
SNAPSHOT_STORE_TTL_MARGIN_SECONDS = 300


class KeyValueStore(Protocol):
    """Minimal cache-like interface (SearchCache satisfies this)."""

    @property
    def is_connected(self) -> bool: ...

    async def get(self, key: str) -> dict[str, Any] | None: ...

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None: ...


@dataclass
class SearchSnapshot:
    """An immutable captured result set for one search."""

    snapshot_id: str
    query: str
    query_id: str
    results: list[SearchResult]
    scope: ScopeDecision
    total: int
    tenant: str
    created_at: float = field(default_factory=time.time)
    expires_at: float | None = None


@dataclass
class SnapshotRead:
    """Outcome of a snapshot read, distinguishing the lifecycle states.

    Exactly one terminal state is signaled:

    - ``unavailable`` — the backing store is unreachable (callers should
      surface ``store_unavailable``, not ``invalid_cursor``/``expired_handle``).
    - ``expired`` — the snapshot is present but past its ``expires_at``
      (callers should surface ``expired_handle`` with ``expires_at``).
    - ``snapshot is None`` with neither flag — the handle is unknown/missing
      (callers should surface ``invalid_cursor``/``invalid_result_id``).
    """

    snapshot: SearchSnapshot | None = None
    unavailable: bool = False
    expired: bool = False
    expires_at: float | None = None


class SnapshotStore:
    """Creates and reads search snapshots in the shared key-value store."""

    def __init__(
        self,
        store: KeyValueStore | None,
        tenant: str = "default",
        ttl_seconds: int = 3600,
    ) -> None:
        self._store = store
        self._tenant = tenant
        self._ttl = ttl_seconds

    @property
    def store_ttl_seconds(self) -> int:
        """Backing-store TTL, deliberately longer than the snapshot horizon.

        The snapshot is logically "expired" at ``expires_at`` (``_ttl`` after
        creation), but the store key must outlive that point so a real Valkey
        keeps the payload around long enough to report ``expired_handle`` with
        expiry metadata instead of an evicted/unknown handle.
        """
        return self._ttl + SNAPSHOT_STORE_TTL_MARGIN_SECONDS

    @property
    def available(self) -> bool:
        """Whether snapshots can be created (Valkey connected)."""
        return self._store is not None and self._store.is_connected

    def for_tenant(self, tenant: str) -> "SnapshotStore":
        """Return a tenant-scoped view sharing the same backing store."""
        if tenant == self._tenant:
            return self
        return SnapshotStore(self._store, tenant=tenant, ttl_seconds=self._ttl)

    def _key(self, snapshot_id: str) -> str:
        return f"{SNAPSHOT_KEY_PREFIX}:{self._tenant}:{snapshot_id}"

    async def create(
        self,
        query: str,
        query_id: str,
        results: list[SearchResult],
        scope: ScopeDecision,
    ) -> str | None:
        """Capture a result set and return its opaque snapshot ID.

        Returns ``None`` when the store is unavailable — callers should
        surface a warning rather than failing the search.
        """
        store = self._store
        if store is None or not store.is_connected:
            return None
        snapshot_id = f"snap-{uuid.uuid4().hex[:12]}"
        created_at = time.time()
        payload: dict[str, Any] = {
            "snapshot_id": snapshot_id,
            "query": query,
            "query_id": query_id,
            "results": [search_result_to_dict(result) for result in results],
            "scope": dataclasses.asdict(scope),
            "total": len(results),
            "tenant": self._tenant,
            "created_at": created_at,
            "expires_at": created_at + self._ttl,
        }
        # Store TTL exceeds the logical expires_at horizon (see
        # ``store_ttl_seconds``) so expired snapshots remain reachable on real
        # Valkey and surface ``expired_handle`` rather than ``invalid_cursor``.
        await store.set(self._key(snapshot_id), payload, self.store_ttl_seconds)
        return snapshot_id

    async def read(self, snapshot_id: str) -> SnapshotRead:
        """Read a snapshot, distinguishing expiry and store unavailability.

        Returns a :class:`SnapshotRead` describing the lifecycle state so
        callers can surface the correct structured error:
        ``store_unavailable`` (store unreachable), ``expired_handle``
        (present but past ``expires_at``), or ``invalid_cursor`` (unknown).
        """
        store = self._store
        if store is None or not store.is_connected:
            return SnapshotRead(unavailable=True)
        if not snapshot_id:
            return SnapshotRead()
        payload = await store.get(self._key(snapshot_id))
        if payload is None:
            return SnapshotRead()
        snapshot = _snapshot_from_payload(payload)
        if snapshot.tenant != self._tenant:
            # Tenant mismatch — treat as missing (defense in depth; the
            # key is already tenant-scoped).
            return SnapshotRead()
        if snapshot.expires_at is not None and time.time() > snapshot.expires_at:
            return SnapshotRead(expired=True, expires_at=snapshot.expires_at)
        return SnapshotRead(snapshot=snapshot)

    async def get(self, snapshot_id: str) -> SearchSnapshot | None:
        """Read a snapshot by opaque ID, or None if missing/unavailable/expired."""
        return (await self.read(snapshot_id)).snapshot

    def result_id(self, snapshot_id: str, index: int) -> str:
        """Build the stable server-issued ID for one snapshot result."""
        return f"{snapshot_id}:{index}"


def _snapshot_from_payload(payload: dict[str, Any]) -> SearchSnapshot:
    """Rehydrate a SearchSnapshot from a serialized payload."""
    scope = payload.get("scope") or {}
    created_at = float(payload.get("created_at", 0.0))
    expires_raw = payload.get("expires_at")
    expires_at = float(expires_raw) if expires_raw is not None else None
    return SearchSnapshot(
        snapshot_id=str(payload.get("snapshot_id", "")),
        query=str(payload.get("query", "")),
        query_id=str(payload.get("query_id", "")),
        results=[search_result_from_dict(item) for item in (payload.get("results") or [])],
        scope=ScopeDecision(
            selected_engines=list(scope.get("selected_engines") or []),
            resolved_categories=list(scope.get("resolved_categories") or []),
            routing_rule=str(scope.get("routing_rule") or ""),
            matched_topic=scope.get("matched_topic"),
            warnings=list(scope.get("warnings") or []),
        ),
        total=int(payload.get("total", 0)),
        tenant=str(payload.get("tenant", "")),
        created_at=created_at,
        expires_at=expires_at,
    )
