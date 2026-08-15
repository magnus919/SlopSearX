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
    def available(self) -> bool:
        """Whether snapshots can be created (Valkey connected)."""
        return self._store is not None and self._store.is_connected

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
        payload: dict[str, Any] = {
            "snapshot_id": snapshot_id,
            "query": query,
            "query_id": query_id,
            "results": [search_result_to_dict(result) for result in results],
            "scope": dataclasses.asdict(scope),
            "total": len(results),
            "tenant": self._tenant,
            "created_at": time.time(),
        }
        await store.set(self._key(snapshot_id), payload, self._ttl)
        return snapshot_id

    async def get(self, snapshot_id: str) -> SearchSnapshot | None:
        """Read a snapshot by opaque ID, or None if missing/unavailable."""
        store = self._store
        if store is None or not store.is_connected or not snapshot_id:
            return None
        payload = await store.get(self._key(snapshot_id))
        if payload is None:
            return None
        snapshot = _snapshot_from_payload(payload)
        if snapshot.tenant != self._tenant:
            # Tenant mismatch — treat as missing (defense in depth; the
            # key is already tenant-scoped).
            return None
        return snapshot

    def result_id(self, snapshot_id: str, index: int) -> str:
        """Build the stable server-issued ID for one snapshot result."""
        return f"{snapshot_id}:{index}"


def _snapshot_from_payload(payload: dict[str, Any]) -> SearchSnapshot:
    """Rehydrate a SearchSnapshot from a serialized payload."""
    scope = payload.get("scope") or {}
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
        created_at=float(payload.get("created_at", 0.0)),
    )
