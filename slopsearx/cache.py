"""Response cache layer (stub — no-op in V1)."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class CacheEntry:
    """A cached search response."""
    query: str
    results: list[dict] = field(default_factory=list)
    engines_used: list[str] = field(default_factory=list)
    created_at: float = 0.0


class ResponseCache:
    """Valkey-backed response cache.

    V1 uses an in-memory dict. Valkey-backed distributed cache will follow.
    """

    def __init__(self, ttl_seconds: int = 300, max_entries: int = 10_000) -> None:
        self._ttl = ttl_seconds
        self._max = max_entries
        self._store: dict[str, CacheEntry] = {}

    async def get(self, key: str) -> CacheEntry | None:
        entry = self._store.get(key)
        if entry is None:
            return None
        import time

        if time.monotonic() - entry.created_at > self._ttl:
            del self._store[key]
            return None
        return entry

    async def set(self, key: str, entry: CacheEntry) -> None:
        import time

        entry.created_at = time.monotonic()
        if len(self._store) >= self._max:
            # Simple eviction: remove oldest
            oldest = min(self._store.keys(), key=lambda k: self._store[k].created_at)  # type: ignore[arg-type]
            del self._store[oldest]
        self._store[key] = entry

    async def clear(self) -> None:
        self._store.clear()
