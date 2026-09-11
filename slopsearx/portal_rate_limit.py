"""Fail-closed bounded rate limits for protected browser surfaces."""

from __future__ import annotations

import asyncio
import hashlib
import hmac
import time
from typing import Any, Callable

_INCREMENT = """
local count = redis.call('INCR', KEYS[1])
if count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
return count
"""


class PortalRateLimiter:
    _locks: dict[int, asyncio.Lock] = {}

    def __init__(self, store: Any, *, key: bytes, clock: Callable[[], float] = time.time) -> None:
        self.store = store
        self._key = key
        self._clock = clock
        self._lock = self._locks.setdefault(id(store), asyncio.Lock())

    def _storage_key(self, bucket: str, identity: str, window: int) -> str:
        slot = int(self._clock()) // window
        digest = hmac.new(self._key, f"{bucket}\0{identity}\0{slot}".encode(), hashlib.sha256).hexdigest()
        return f"portal:limit:v1:{bucket}:{digest}"

    async def allow(self, bucket: str, identity: str, *, limit: int, window: int) -> bool:
        if not self.store.is_connected:
            return False
        key = self._storage_key(bucket, identity, window)
        client = getattr(self.store, "_client", None)
        if client is not None and hasattr(client, "eval"):
            try:
                count = int(await client.eval(_INCREMENT, 1, key, str(window + 1)))
            except Exception:  # noqa: BLE001 - protected surfaces fail closed
                return False
            return count <= limit
        async with self._lock:
            current = await self.store.get(key) or {}
            count = int(current.get("count", 0)) + 1
            await self.store.set(key, {"count": count}, window + 1)
            return count <= limit
