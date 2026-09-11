"""Immutable, tenant-scoped retrieval receipts; this module never performs I/O to supplied URLs."""

from __future__ import annotations

import asyncio
import hashlib
import json
import secrets
from typing import Any

from slopsearx.snapshot import KeyValueStore

RECEIPT_PREFIX = "mcp:retrieval-receipt:v1"
RECEIPT_CONTRACT = "slopsearx.retrieval_receipt"
RECEIPT_VERSION = 1
RECEIPT_RETENTION_SECONDS = 86_400
RECEIPT_STORE_MARGIN_SECONDS = 300
MAX_RECEIPTS_PER_RESULT = 20
MAX_RECEIPT_BYTES = 32 * 1024

_INSERT = """
local idem = redis.call('GET', KEYS[4])
if idem then
  local prior = cjson.decode(idem)
  if tonumber(prior.expires_at) > tonumber(ARGV[1]) then
    if prior.digest ~= ARGV[2] then return cjson.encode({status='conflict'}) end
    local existing = redis.call('GET', ARGV[3] .. prior.receipt_id)
    if existing then return cjson.encode({status='replayed', receipt=cjson.decode(existing)}) end
  end
  redis.call('DEL', KEYS[4])
end
local horizon = redis.call('GET', KEYS[1])
local expires_at = horizon and tonumber(cjson.decode(horizon).expires_at) or nil
local new_horizon = false
if not expires_at or expires_at <= tonumber(ARGV[1]) then
  expires_at = tonumber(ARGV[1]) + tonumber(ARGV[4])
  new_horizon = true
end
local ttl = math.max(1, math.floor(expires_at - tonumber(ARGV[1])) + tonumber(ARGV[5]))
if not new_horizon and redis.call('ZCARD', KEYS[2]) >= tonumber(ARGV[6]) then
  return cjson.encode({status='capacity'})
end
local receipt = cjson.decode(ARGV[7])
receipt.expires_at = tonumber(expires_at)
local encoded = cjson.encode(receipt)
if string.len(encoded) > tonumber(ARGV[10]) then return cjson.encode({status='size'}) end
if new_horizon then
  local old = redis.call('ZRANGE', KEYS[2], 0, -1)
  for _, receipt_id in ipairs(old) do redis.call('DEL', ARGV[3] .. receipt_id) end
  redis.call('DEL', KEYS[2])
  redis.call('SETEX', KEYS[1], tonumber(ARGV[4]) + tonumber(ARGV[5]), cjson.encode({expires_at=expires_at}))
end
redis.call('SETEX', KEYS[3], ttl, encoded)
redis.call('ZADD', KEYS[2], ARGV[8], ARGV[9])
redis.call('EXPIRE', KEYS[2], ttl)
local idempotency = {digest=ARGV[2], receipt_id=ARGV[9], expires_at=tonumber(expires_at)}
redis.call('SETEX', KEYS[4], ttl, cjson.encode(idempotency))
return cjson.encode({status='created', receipt=receipt})
"""


class ReceiptStore:
    """Valkey authority for immutable receipts and fixed result-bundle horizons."""

    def __init__(self, store: KeyValueStore | None, tenant: str = "default") -> None:
        self._store = store
        self._tenant = tenant
        self._lock = asyncio.Lock()

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    def for_tenant(self, tenant: str) -> "ReceiptStore":
        return self if tenant == self._tenant else ReceiptStore(self._store, tenant)

    def _token(self, value: str) -> str:
        return hashlib.sha256(value.encode()).hexdigest()

    def _horizon_key(self, result_id: str) -> str:
        return f"{RECEIPT_PREFIX}:horizon:{self._tenant}:{self._token(result_id)}"

    def _index_key(self, result_id: str) -> str:
        return f"{RECEIPT_PREFIX}:index:{self._tenant}:{self._token(result_id)}"

    def _receipt_prefix(self, result_id: str) -> str:
        return f"{RECEIPT_PREFIX}:receipt:{self._tenant}:{self._token(result_id)}:"

    def _receipt_key(self, result_id: str, receipt_id: str) -> str:
        return f"{self._receipt_prefix(result_id)}{receipt_id}"

    def _idem_key(self, result_id: str, key: str) -> str:
        return f"{RECEIPT_PREFIX}:idem:{self._tenant}:{self._token(result_id)}:{self._token(key)}"

    def _client(self) -> Any:
        return getattr(self._store, "_client", None)

    async def replay(
        self, result_id: str, idempotency_key: str, digest: str, *, now: float
    ) -> tuple[str, dict[str, Any] | None]:
        """Resolve retained replay before source-snapshot lookup."""
        if not self.available:
            return "unavailable", None
        prior = await self._store.get(self._idem_key(result_id, idempotency_key))  # type: ignore[union-attr]
        if not prior or float(prior.get("expires_at", 0)) <= now:
            return "missing", None
        if prior.get("digest") != digest:
            return "conflict", None
        receipt = await self._store.get(self._receipt_key(result_id, str(prior.get("receipt_id", ""))))  # type: ignore[union-attr]
        return ("replayed", receipt) if receipt else ("missing", None)

    async def submit(
        self,
        result_id: str,
        idempotency_key: str,
        digest: str,
        receipt: dict[str, Any],
        *,
        now: float,
    ) -> tuple[str, dict[str, Any] | None]:
        """Insert atomically, enforcing fixed horizon, replay and capacity."""
        if not self.available:
            return "unavailable", None
        receipt_id = f"receipt-{secrets.token_urlsafe(18)}"
        receipt = dict(receipt, receipt_id=receipt_id, submitted_at=now)
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            raw = await client.eval(
                _INSERT,
                4,
                self._horizon_key(result_id),
                self._index_key(result_id),
                self._receipt_key(result_id, receipt_id),
                self._idem_key(result_id, idempotency_key),
                str(now),
                digest,
                self._receipt_prefix(result_id),
                str(RECEIPT_RETENTION_SECONDS),
                str(RECEIPT_STORE_MARGIN_SECONDS),
                str(MAX_RECEIPTS_PER_RESULT),
                json.dumps(receipt, separators=(",", ":"), ensure_ascii=False),
                str(now),
                receipt_id,
                str(MAX_RECEIPT_BYTES),
            )
            value = json.loads(raw)
            return str(value["status"]), value.get("receipt")
        async with self._lock:
            replay, existing = await self.replay(result_id, idempotency_key, digest, now=now)
            if replay in {"replayed", "conflict"}:
                return replay, existing
            data = getattr(self._store, "_data", None)
            if data is None:
                return "unavailable", None
            horizon_key = self._horizon_key(result_id)
            horizon = data.get(horizon_key)
            expires_at = float(horizon.get("expires_at", 0)) if isinstance(horizon, dict) else 0
            prefix = self._receipt_prefix(result_id)
            new_horizon = expires_at <= now
            if new_horizon:
                expires_at = now + RECEIPT_RETENTION_SECONDS
            physical_ttl = max(1, int(expires_at - now) + RECEIPT_STORE_MARGIN_SECONDS)
            ids = [] if new_horizon else [key for key in data if key.startswith(prefix)]
            if len(ids) >= MAX_RECEIPTS_PER_RESULT:
                return "capacity", None
            receipt["expires_at"] = expires_at
            encoded = json.dumps(receipt, separators=(",", ":"), ensure_ascii=False).encode()
            if len(encoded) > MAX_RECEIPT_BYTES:
                return "size", None
            if new_horizon:
                for key in [key for key in data if key.startswith(prefix)]:
                    data.pop(key, None)
                data[horizon_key] = {"expires_at": expires_at}
            await self._store.set(self._receipt_key(result_id, receipt_id), receipt, physical_ttl)  # type: ignore[union-attr]
            await self._store.set(  # type: ignore[union-attr]
                self._idem_key(result_id, idempotency_key),
                {"digest": digest, "receipt_id": receipt_id, "expires_at": expires_at},
                physical_ttl,
            )
            return "created", receipt

    async def read(self, result_id: str, *, now: float, limit: int = 20) -> tuple[list[dict[str, Any]], int]:
        """Read newest-first retained receipts and the complete retained count."""
        if not self.available:
            return [], 0
        horizon = await self._store.get(self._horizon_key(result_id))  # type: ignore[union-attr]
        if not horizon or float(horizon.get("expires_at", 0)) <= now:
            return [], 0
        client = self._client()
        if client is not None and hasattr(client, "zrevrange"):
            total = int(await client.zcard(self._index_key(result_id)))
            raw = await client.zrevrange(self._index_key(result_id), 0, limit - 1)
            ids = [item.decode() if isinstance(item, bytes) else str(item) for item in raw]
        else:
            data = getattr(self._store, "_data", {})
            prefix = self._receipt_prefix(result_id)
            values = [value for key, value in data.items() if key.startswith(prefix)]
            values.sort(key=lambda item: float(item.get("submitted_at", 0)), reverse=True)
            return values[:limit], len(values)
        values = [await self._store.get(self._receipt_key(result_id, item)) for item in ids]  # type: ignore[union-attr]
        return [item for item in values if item], total
