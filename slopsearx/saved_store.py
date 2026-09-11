"""Tenant-scoped Valkey persistence for scheduled saved searches."""

from __future__ import annotations

import asyncio
import json
import secrets
import time
from typing import Any

from slopsearx.saved_events import (
    cursor_tuple,
    definition_event,
    validate_consumer_id,
    validate_cursor,
)
from slopsearx.saved_models import SavedDefinition, definition_from_payload, definition_to_payload
from slopsearx.snapshot import KeyValueStore

PREFIX = "mcp:saved:v1"
STORE_TTL_MARGIN = 300
LEASE_SECONDS = 60
DEFAULT_EVENT_CAPACITY = 1000
DEFAULT_EVENT_RETENTION_SECONDS = 604_800
DEFAULT_EVENT_CONSUMERS = 100

_CREATE = """
redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', ARGV[1])
if redis.call('EXISTS', KEYS[1]) == 1 then return 0 end
local authoritative = tonumber(redis.call('GET', KEYS[5]))
local indexed = redis.call('ZCARD', KEYS[2])
if not authoritative or authoritative ~= indexed then
  local known_tenant = redis.call('ZSCORE', KEYS[4], ARGV[10])
  if indexed == 0 and not authoritative and not known_tenant and redis.call('EXISTS', KEYS[4]) == 1 then
    authoritative = 0
  else
    local page = redis.call('SCAN', '0', 'MATCH', ARGV[9], 'COUNT', 256)
    local found = {}
    authoritative = 0
    for _, key in ipairs(page[2]) do
      local raw = redis.call('GET', key)
      if raw then
        local decoded, definition = pcall(cjson.decode, raw)
        if not decoded then return -2 end
        if definition.tenant == ARGV[10] and not definition.deleted and
           tonumber(definition.expires_at) > tonumber(ARGV[1]) then
          authoritative = authoritative + 1
          found[definition.search_id] = tonumber(definition.expires_at)
        end
      end
    end
    if authoritative >= tonumber(ARGV[2]) then return -1 end
    if page[1] ~= '0' then return -2 end
    redis.call('DEL', KEYS[2])
    for search_id, expires_at in pairs(found) do redis.call('ZADD', KEYS[2], expires_at, search_id) end
  end
end
if authoritative >= tonumber(ARGV[2]) then return -1 end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
redis.call('ZADD', KEYS[2], ARGV[5], ARGV[6])
redis.call('ZADD', KEYS[3], ARGV[7], ARGV[6])
redis.call('ZADD', KEYS[4], 'GT', ARGV[5], ARGV[8])
if redis.call('TTL', KEYS[2]) < tonumber(ARGV[3]) then redis.call('EXPIRE', KEYS[2], ARGV[3]) end
if redis.call('TTL', KEYS[3]) < tonumber(ARGV[3]) then redis.call('EXPIRE', KEYS[3], ARGV[3]) end
local count_ttl = redis.call('TTL', KEYS[5])
redis.call('SET', KEYS[5], authoritative + 1)
redis.call('EXPIRE', KEYS[5], math.max(count_ttl, tonumber(ARGV[3])))
return 1
"""

_CAS = """
local raw = redis.call('GET', KEYS[1])
if not raw then return -1 end
local current = cjson.decode(raw)
if current.deleted or tonumber(current.expires_at) <= tonumber(ARGV[1]) then return -1 end
if tonumber(current.revision) ~= tonumber(ARGV[2]) then return tonumber(current.revision) end
if ARGV[8] ~= '' then
  redis.call('XTRIM', KEYS[5], 'MINID', ARGV[9])
  if redis.call('XLEN', KEYS[5]) >= tonumber(ARGV[10]) then return -3 end
  redis.call('XADD', KEYS[5], '*', 'payload', ARGV[8])
  redis.call('EXPIRE', KEYS[5], ARGV[11])
end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[4])
if ARGV[5] == 'paused' then redis.call('ZREM', KEYS[2], ARGV[6])
else redis.call('ZADD', KEYS[2], ARGV[7], ARGV[6]) end
redis.call('DEL', KEYS[3])
redis.call('DEL', KEYS[4])
return 0
"""

_DELETE = """
local raw = redis.call('GET', KEYS[1])
if not raw then return -1 end
local current = cjson.decode(raw)
if tonumber(current.revision) ~= tonumber(ARGV[1]) then return tonumber(current.revision) end
redis.call('DEL', KEYS[1])
redis.call('ZREM', KEYS[2], ARGV[2])
redis.call('ZREM', KEYS[3], ARGV[2])
redis.call('SETEX', KEYS[4], ARGV[3], ARGV[1])
redis.call('DEL', KEYS[5])
local reports = redis.call('ZRANGE', KEYS[6], 0, -1)
for _, run_id in ipairs(reports) do redis.call('DEL', ARGV[4] .. run_id) end
redis.call('DEL', KEYS[6])
redis.call('DEL', KEYS[7])
redis.call('DEL', KEYS[8])
return 0
"""

_CLAIM = """
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if current.deleted or current.paused or tonumber(current.expires_at) <= tonumber(ARGV[1]) or
   tonumber(current.next_due) > tonumber(ARGV[1]) then return 0 end
if redis.call('SET', KEYS[2], ARGV[2], 'NX', 'EX', ARGV[3]) == false then return 0 end
current.lease_token = ARGV[2]
current.lease_expires_at = tonumber(ARGV[1]) + tonumber(ARGV[3])
redis.call('SETEX', KEYS[1], ARGV[4], cjson.encode(current))
return cjson.encode(current)
"""

_COMMIT = """
if redis.call('GET', KEYS[2]) ~= ARGV[1] then return {-2, ''} end
local raw = redis.call('GET', KEYS[1])
if not raw then return {-1, ''} end
local current = cjson.decode(raw)
if tonumber(current.revision) ~= tonumber(ARGV[2]) or current.paused or current.deleted or
   tonumber(current.expires_at) <= tonumber(ARGV[3]) or current.policy_fingerprint ~= ARGV[4] then return {-3, ''} end
local event_cursor = ''
if ARGV[17] ~= '' then
  redis.call('XTRIM', KEYS[7], 'MINID', ARGV[18])
  if redis.call('XLEN', KEYS[7]) >= tonumber(ARGV[19]) then return {-4, ''} end
  event_cursor = redis.call('XADD', KEYS[7], '*', 'payload', ARGV[17])
  redis.call('EXPIRE', KEYS[7], ARGV[20])
end
redis.call('SETEX', KEYS[3], ARGV[5], ARGV[6])
redis.call('ZADD', KEYS[4], ARGV[7], ARGV[8])
redis.call('EXPIRE', KEYS[4], ARGV[5])
while redis.call('ZCARD', KEYS[4]) > tonumber(ARGV[9]) do
  local oldest = redis.call('ZRANGE', KEYS[4], 0, 0)
  if #oldest == 0 then break end
  redis.call('ZREM', KEYS[4], oldest[1])
  redis.call('DEL', ARGV[10] .. oldest[1])
end
redis.call('SETEX', KEYS[1], ARGV[12], ARGV[11])
redis.call('ZADD', KEYS[5], ARGV[13], ARGV[14])
redis.call('EXPIRE', KEYS[5], ARGV[12])
if ARGV[15] ~= '' then redis.call('SETEX', KEYS[6], ARGV[16], ARGV[15]) end
redis.call('DEL', KEYS[2])
return {1, event_cursor}
"""

_READ_EVENTS = """
redis.call('XTRIM', KEYS[1], 'MINID', ARGV[1])
local first = redis.call('XRANGE', KEYS[1], '-', '+', 'COUNT', 1)
local first_cursor = ''
if #first > 0 then first_cursor = first[1][1] end
local entries = redis.call('XRANGE', KEYS[1], '(' .. ARGV[2], '+', 'COUNT', tonumber(ARGV[3]))
return {first_cursor, entries}
"""

_ACK_EVENT = """
redis.call('ZREMRANGEBYSCORE', KEYS[2], '-inf', ARGV[1])
redis.call('XTRIM', KEYS[3], 'MINID', ARGV[7])
local tail = redis.call('XREVRANGE', KEYS[3], '+', '-', 'COUNT', 1)
if #tail == 0 then return -3 end
local function parts(value)
  local dash = string.find(value, '-')
  return tonumber(string.sub(value, 1, dash - 1)), tonumber(string.sub(value, dash + 1))
end
local requested_ms, requested_seq = parts(ARGV[2])
local tail_ms, tail_seq = parts(tail[1][1])
if requested_ms > tail_ms or (requested_ms == tail_ms and requested_seq > tail_seq) then return -2 end
local head = redis.call('XRANGE', KEYS[3], '-', '+', 'COUNT', 1)
local head_ms, head_seq = parts(head[1][1])
if requested_ms < head_ms or (requested_ms == head_ms and requested_seq < head_seq) then return -4 end
local current = redis.call('GET', KEYS[1])
if current then
  local current_ms, current_seq = parts(current)
  if requested_ms < current_ms or (requested_ms == current_ms and requested_seq <= current_seq) then
    redis.call('EXPIRE', KEYS[1], ARGV[3])
    redis.call('ZADD', KEYS[2], ARGV[4], ARGV[5])
    redis.call('EXPIRE', KEYS[2], ARGV[3])
    return 0
  end
elseif redis.call('ZCARD', KEYS[2]) >= tonumber(ARGV[6]) then
  return -1
end
redis.call('SETEX', KEYS[1], ARGV[3], ARGV[2])
redis.call('ZADD', KEYS[2], ARGV[4], ARGV[5])
redis.call('EXPIRE', KEYS[2], ARGV[3])
return 1
"""

_PUBLISH_EXPIRY = """
if redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
local raw = redis.call('GET', KEYS[1])
if not raw then return 0 end
local current = cjson.decode(raw)
if tonumber(current.expires_at) > tonumber(ARGV[1]) then return 0 end
redis.call('XTRIM', KEYS[3], 'MINID', ARGV[2])
if redis.call('XLEN', KEYS[3]) >= tonumber(ARGV[3]) then return -1 end
redis.call('XADD', KEYS[3], '*', 'payload', ARGV[4])
redis.call('EXPIRE', KEYS[3], ARGV[5])
redis.call('SETEX', KEYS[2], ARGV[5], '1')
return 1
"""

_RENEW = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
redis.call('EXPIRE', KEYS[1], ARGV[2])
return 1
"""

_RELEASE = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
redis.call('DEL', KEYS[1])
return 1
"""


class RevisionConflictError(ValueError):
    def __init__(self, current_revision: int) -> None:
        super().__init__("saved search revision changed")
        self.current_revision = current_revision


class OutboxCapacityError(RuntimeError):
    """The bounded tenant stream cannot accept another atomic publication."""


class SavedSearchStore:
    """Durable authority; indexes are bounded, repairable scheduling hints."""

    def __init__(
        self,
        store: KeyValueStore | None,
        tenant: str = "default",
        *,
        event_capacity: int = DEFAULT_EVENT_CAPACITY,
        event_retention_seconds: int = DEFAULT_EVENT_RETENTION_SECONDS,
        event_consumers: int = DEFAULT_EVENT_CONSUMERS,
    ) -> None:
        self._store = store
        self._tenant = tenant
        self._event_capacity = event_capacity
        self._event_retention_seconds = event_retention_seconds
        self._event_consumers = event_consumers
        self._lock = asyncio.Lock()
        self._children: dict[str, SavedSearchStore] = {}
        self._definition_scan_cursor = 0
        self._tenant_scan_cursor = 0
        self._expiry_scan_cursor = 0

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    def for_tenant(self, tenant: str) -> "SavedSearchStore":
        if tenant == self._tenant:
            return self
        if tenant not in self._children:
            self._children[tenant] = SavedSearchStore(
                self._store,
                tenant,
                event_capacity=self._event_capacity,
                event_retention_seconds=self._event_retention_seconds,
                event_consumers=self._event_consumers,
            )
        return self._children[tenant]

    def _definition_key(self, search_id: str) -> str:
        return f"{PREFIX}:definition:{self._tenant}:{search_id}"

    def _definition_scan_pattern(self) -> str:
        prefix = f"{PREFIX}:definition:"
        for token in ("\\", "*", "?", "[", "]"):
            prefix = prefix.replace(token, f"\\{token}")
        return f"{prefix}*"

    def _definition_count(self) -> str:
        return f"{PREFIX}:definition-count:{self._tenant}"

    def _definition_index(self) -> str:
        return f"{PREFIX}:definitions:{self._tenant}"

    def _due_index(self) -> str:
        return f"{PREFIX}:due:{self._tenant}"

    def _tenant_index(self) -> str:
        return f"{PREFIX}:tenants"

    def _lease_key(self, search_id: str) -> str:
        return f"{PREFIX}:lease:{self._tenant}:{search_id}"

    def _tombstone_key(self, search_id: str) -> str:
        return f"{PREFIX}:deleted:{self._tenant}:{search_id}"

    def _report_key(self, search_id: str, run_id: str) -> str:
        return f"{PREFIX}:report:{self._tenant}:{search_id}:{run_id}"

    def _report_prefix(self, search_id: str) -> str:
        return f"{PREFIX}:report:{self._tenant}:{search_id}:"

    def _report_index(self, search_id: str) -> str:
        return f"{PREFIX}:reports:{self._tenant}:{search_id}"

    def _baseline_key(self, search_id: str) -> str:
        return f"{PREFIX}:baseline:{self._tenant}:{search_id}"

    def _event_stream(self) -> str:
        return f"{PREFIX}:events:{self._tenant}"

    def _event_ack_key(self, consumer_id: str) -> str:
        return f"{PREFIX}:event-ack:{self._tenant}:{consumer_id}"

    def _event_consumers_key(self) -> str:
        return f"{PREFIX}:event-consumers:{self._tenant}"

    def _expiry_event_marker(self, search_id: str) -> str:
        return f"{PREFIX}:event-expired:{self._tenant}:{search_id}"

    def _client(self) -> Any:
        return getattr(self._store, "_client", None)

    def _ttl(self, definition: SavedDefinition, now: float) -> int:
        return max(1, int(definition.expires_at - now) + STORE_TTL_MARGIN)

    @staticmethod
    def _stored_payload(definition: SavedDefinition) -> dict[str, Any]:
        payload = definition_to_payload(definition)
        payload["baseline"] = None
        return payload

    async def load(self, search_id: str, *, now: float | None = None) -> SavedDefinition | None:
        if not self.available:
            return None
        payload = await self._store.get(self._definition_key(search_id))  # type: ignore[union-attr]
        if payload is None:
            return None
        value = definition_from_payload(payload)
        instant = time.time() if now is None else now
        if value.tenant != self._tenant or value.deleted or value.expires_at <= instant:
            return None
        baseline = await self._store.get(self._baseline_key(search_id))  # type: ignore[union-attr]
        if baseline is not None and float(baseline.get("expires_at", 0)) > instant:
            value.baseline = baseline
        return value

    async def create(self, definition: SavedDefinition, quota: int, *, now: float) -> str:
        """Create atomically; returns created, quota_exceeded, or duplicate."""
        if not self.available:
            return "unavailable"
        payload = json.dumps(self._stored_payload(definition), separators=(",", ":"))
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            result = await client.eval(
                _CREATE,
                5,
                self._definition_key(definition.search_id),
                self._definition_index(),
                self._due_index(),
                self._tenant_index(),
                self._definition_count(),
                str(now),
                str(quota),
                str(self._ttl(definition, now)),
                payload,
                str(definition.expires_at),
                definition.search_id,
                str(definition.next_due),
                self._tenant,
                self._definition_scan_pattern(),
                self._tenant,
            )
            return {1: "created", 0: "duplicate", -1: "quota_exceeded", -2: "unavailable"}.get(
                int(result), "unavailable"
            )
        async with self._lock:
            current = await self.list_definitions(now=now)
            if len(current) >= quota:
                return "quota_exceeded"
            if await self.load(definition.search_id, now=now):
                return "duplicate"
            await self._store.set(  # type: ignore[union-attr]
                self._definition_key(definition.search_id),
                self._stored_payload(definition),
                self._ttl(definition, now),
            )
            return "created"

    async def list_definitions(self, *, now: float | None = None, limit: int = 128) -> list[SavedDefinition]:
        if not self.available:
            return []
        instant = time.time() if now is None else now
        client = self._client()
        ids: list[str] = []
        if client is not None and hasattr(client, "zrangebyscore"):
            await client.zremrangebyscore(self._definition_index(), "-inf", instant)
            raw = await client.zrangebyscore(self._definition_index(), instant, "+inf", start=0, num=limit)
            ids = [item.decode() if isinstance(item, bytes) else str(item) for item in raw]
        else:
            data = getattr(self._store, "_data", {})
            prefix = f"{PREFIX}:definition:{self._tenant}:"
            ids = [key[len(prefix) :] for key in data if key.startswith(prefix)][:limit]
        values = [await self.load(item, now=instant) for item in ids]
        return [item for item in values if item is not None]

    async def scan_definitions(self, *, now: float, limit: int = 128) -> list[SavedDefinition]:
        """Scan authoritative records, independent of the derived index."""
        if not self.available:
            return []
        prefix = f"{PREFIX}:definition:{self._tenant}:"
        client = self._client()
        keys: list[str] = []
        if client is not None and hasattr(client, "scan_iter"):
            cursor, raw_keys = await client.scan(self._definition_scan_cursor, match=f"{prefix}*", count=limit)
            self._definition_scan_cursor = int(cursor)
            keys = [raw.decode() if isinstance(raw, bytes) else str(raw) for raw in raw_keys]
        else:
            data = getattr(self._store, "_data", {})
            keys = [key for key in data if key.startswith(prefix)][:limit]
        values = [await self.load(key[len(prefix) :], now=now) for key in keys]
        return [item for item in values if item is not None]

    async def scan_tenants(self, limit: int = 128, *, now: float | None = None) -> list[str]:
        """Continue an authoritative global scan so missing hints self-heal."""
        if not self.available:
            return []
        prefix = f"{PREFIX}:definition:"
        client = self._client()
        instant = time.time() if now is None else now
        keys: list[str] = []
        if client is not None and hasattr(client, "scan"):
            await client.zremrangebyscore(self._tenant_index(), "-inf", instant)
            cursor, raw_keys = await client.scan(self._tenant_scan_cursor, match=f"{prefix}*", count=limit)
            self._tenant_scan_cursor = int(cursor)
            keys = [raw.decode() if isinstance(raw, bytes) else str(raw) for raw in raw_keys]
        else:
            data = getattr(self._store, "_data", {})
            keys = [key for key in data if key.startswith(prefix)][:limit]
        tenants: set[str] = set()
        for key in keys:
            rest = key[len(prefix) :]
            if ":saved-" in rest:
                tenant, search_id = rest.rsplit(":", 1)
                definition = await self.for_tenant(tenant).load(search_id, now=instant)
                if definition is not None:
                    tenants.add(tenant)
                    if client is not None and hasattr(client, "zadd"):
                        await client.zadd(self._tenant_index(), {tenant: definition.expires_at}, gt=True)
        return sorted(tenants)

    async def compare_and_set(
        self,
        definition: SavedDefinition,
        expected_revision: int,
        *,
        now: float,
        event: dict[str, Any] | None = None,
    ) -> SavedDefinition:
        """Persist a caller mutation only against its authoritative revision."""
        if not self.available:
            raise LookupError("unavailable")
        current = await self.load(definition.search_id, now=now)
        if current is None:
            raise LookupError(definition.search_id)
        if current.revision != expected_revision:
            raise RevisionConflictError(current.revision)
        definition.revision = expected_revision + 1
        definition.lease_token = None
        definition.lease_expires_at = 0
        payload = json.dumps(self._stored_payload(definition), separators=(",", ":"))
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            result = int(
                await client.eval(
                    _CAS,
                    5,
                    self._definition_key(definition.search_id),
                    self._due_index(),
                    self._baseline_key(definition.search_id),
                    self._lease_key(definition.search_id),
                    self._event_stream(),
                    str(now),
                    str(expected_revision),
                    str(self._ttl(definition, now)),
                    payload,
                    "paused" if definition.paused else "active",
                    definition.search_id,
                    str(definition.next_due),
                    json.dumps(event, separators=(",", ":")) if event else "",
                    f"{max(0, int((now - self._event_retention_seconds) * 1000))}-0",
                    str(self._event_capacity),
                    str(self._event_retention_seconds + STORE_TTL_MARGIN),
                )
            )
            if result == -1:
                raise LookupError(definition.search_id)
            if result == -3:
                raise OutboxCapacityError("saved-search event outbox is at capacity")
            if result != 0:
                raise RevisionConflictError(result)
        else:
            async with self._lock:
                latest = await self.load(definition.search_id, now=now)
                if latest is None:
                    raise LookupError(definition.search_id)
                if latest.revision != expected_revision:
                    raise RevisionConflictError(latest.revision)
                if event:
                    stream = await self._store.get(self._event_stream())  # type: ignore[union-attr]
                    entries = list(stream.get("entries", [])) if isinstance(stream, dict) else []
                    cutoff = now - self._event_retention_seconds
                    entries = [item for item in entries if float(item["event"]["occurred_at"]) > cutoff]
                    if len(entries) >= self._event_capacity:
                        raise OutboxCapacityError("saved-search event outbox is at capacity")
                    milliseconds = max(int(now * 1000), cursor_tuple(entries[-1]["cursor"])[0] if entries else 0)
                    sequence = (
                        cursor_tuple(entries[-1]["cursor"])[1] + 1
                        if entries and cursor_tuple(entries[-1]["cursor"])[0] == milliseconds
                        else 0
                    )
                    entries.append({"cursor": f"{milliseconds}-{sequence}", "event": event})
                data = getattr(self._store, "_data", None)
                if data is not None:
                    data.pop(self._baseline_key(definition.search_id), None)
                await self._store.set(  # type: ignore[union-attr]
                    self._definition_key(definition.search_id),
                    self._stored_payload(definition),
                    self._ttl(definition, now),
                )
                if event:
                    await self._store.set(  # type: ignore[union-attr]
                        self._event_stream(),
                        {"entries": entries},
                        self._event_retention_seconds + STORE_TTL_MARGIN,
                    )
        return definition

    async def delete(self, search_id: str, expected_revision: int, *, now: float) -> None:
        if not self.available:
            raise LookupError("unavailable")
        current = await self.load(search_id, now=now)
        if current is None:
            raise LookupError(search_id)
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            result = int(
                await client.eval(
                    _DELETE,
                    8,
                    self._definition_key(search_id),
                    self._definition_index(),
                    self._due_index(),
                    self._tombstone_key(search_id),
                    self._baseline_key(search_id),
                    self._report_index(search_id),
                    self._lease_key(search_id),
                    self._definition_count(),
                    str(expected_revision),
                    search_id,
                    str(max(1, int(current.expires_at - now) + STORE_TTL_MARGIN)),
                    self._report_prefix(search_id),
                )
            )
            if result == -1:
                raise LookupError(search_id)
            if result != 0:
                raise RevisionConflictError(result)
            return
        async with self._lock:
            latest = await self.load(search_id, now=now)
            if latest is None:
                raise LookupError(search_id)
            if latest.revision != expected_revision:
                raise RevisionConflictError(latest.revision)
            data = getattr(self._store, "_data", None)
            if data is not None:
                data.pop(self._definition_key(search_id), None)
                data.pop(self._baseline_key(search_id), None)
                for key in [key for key in data if key.startswith(self._report_prefix(search_id))]:
                    data.pop(key, None)
                data[self._tombstone_key(search_id)] = {"revision": expected_revision}
            else:
                raise LookupError("delete unsupported")

    async def due_ids(self, now: float, limit: int = 128) -> list[str]:
        """Read a bounded hint set; callers revalidate definitions on claim."""
        client = self._client()
        if client is not None and hasattr(client, "zrangebyscore"):
            raw = await client.zrangebyscore(self._due_index(), "-inf", now, start=0, num=limit)
            ids = [item.decode() if isinstance(item, bytes) else str(item) for item in raw]
            valid: list[str] = []
            for search_id in ids:
                definition = await self.load(search_id, now=now)
                if definition is None or definition.paused:
                    await client.zrem(self._due_index(), search_id)
                else:
                    valid.append(search_id)
            return valid
        return [
            item.search_id
            for item in await self.list_definitions(now=now, limit=limit)
            if not item.paused and item.next_due <= now
        ]

    async def claim(self, search_id: str, *, now: float) -> SavedDefinition | None:
        if not self.available:
            return None
        token = secrets.token_urlsafe(18)
        current = await self.load(search_id, now=now)
        if current is None or current.paused or current.next_due > now:
            return None
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            raw = await client.eval(
                _CLAIM,
                2,
                self._definition_key(search_id),
                self._lease_key(search_id),
                str(now),
                token,
                str(LEASE_SECONDS),
                str(self._ttl(current, now)),
            )
            if not raw:
                return None
            value = json.loads(raw)
            claimed = definition_from_payload(value)
            baseline = await self._store.get(self._baseline_key(search_id))  # type: ignore[union-attr]
            if baseline is not None and float(baseline.get("expires_at", 0)) > now:
                claimed.baseline = baseline
            return claimed
        async with self._lock:
            latest = await self.load(search_id, now=now)
            if latest is None or latest.paused or latest.next_due > now or latest.lease_expires_at > now:
                return None
            latest.lease_token = token
            latest.lease_expires_at = now + LEASE_SECONDS
            await self._store.set(  # type: ignore[union-attr]
                self._definition_key(search_id), self._stored_payload(latest), self._ttl(latest, now)
            )
            return latest

    async def commit(
        self,
        definition: SavedDefinition,
        report: dict[str, Any],
        replacement_baseline: dict[str, Any] | None,
        *,
        now: float,
        next_due: float,
        policy_fingerprint: str,
        event: dict[str, Any] | None = None,
    ) -> bool:
        """Atomically commit one report and baseline if token/revision/policy still match."""
        if not definition.lease_token or not self.available:
            return False
        current_payload = self._stored_payload(definition)
        current_payload.update(
            latest_run_id=report["run_id"],
            last_slot=int(report["slot"]),
            next_due=next_due,
            lease_token=None,
            lease_expires_at=0,
        )
        current = definition_from_payload(current_payload)
        definition_ttl = self._ttl(current, now)
        report_ttl = max(1, int(float(report["expires_at"]) - now) + STORE_TTL_MARGIN)
        baseline_payload = json.dumps(replacement_baseline, separators=(",", ":")) if replacement_baseline else ""
        baseline_ttl = (
            max(1, int(float(replacement_baseline["expires_at"]) - now) + STORE_TTL_MARGIN)
            if replacement_baseline
            else 1
        )
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            result = await client.eval(
                _COMMIT,
                7,
                self._definition_key(definition.search_id),
                self._lease_key(definition.search_id),
                self._report_key(definition.search_id, report["run_id"]),
                self._report_index(definition.search_id),
                self._due_index(),
                self._baseline_key(definition.search_id),
                self._event_stream(),
                definition.lease_token,
                str(definition.revision),
                str(now),
                policy_fingerprint,
                str(report_ttl),
                json.dumps(report, separators=(",", ":")),
                str(report["finished_at"]),
                report["run_id"],
                str(definition.max_reports),
                self._report_prefix(definition.search_id),
                json.dumps(self._stored_payload(current), separators=(",", ":")),
                str(definition_ttl),
                str(next_due),
                definition.search_id,
                baseline_payload,
                str(baseline_ttl),
                json.dumps(event, separators=(",", ":")) if event else "",
                f"{max(0, int((now - self._event_retention_seconds) * 1000))}-0",
                str(self._event_capacity),
                str(self._event_retention_seconds + STORE_TTL_MARGIN),
            )
            code = int(result[0])
            if code == -4:
                raise OutboxCapacityError("saved-search event outbox is at capacity")
            return code == 1
        async with self._lock:
            latest = await self.load(definition.search_id, now=now)
            if (
                latest is None
                or latest.revision != definition.revision
                or latest.paused
                or latest.policy_fingerprint != policy_fingerprint
                or latest.lease_token != definition.lease_token
            ):
                return False
            stream = await self._store.get(self._event_stream()) if event else None  # type: ignore[union-attr]
            entries = list(stream.get("entries", [])) if isinstance(stream, dict) else []
            cutoff = now - self._event_retention_seconds
            entries = [item for item in entries if float(item["event"]["occurred_at"]) > cutoff]
            if event and len(entries) >= self._event_capacity:
                raise OutboxCapacityError("saved-search event outbox is at capacity")
            if event:
                milliseconds = max(int(now * 1000), cursor_tuple(entries[-1]["cursor"])[0] if entries else 0)
                sequence = (
                    cursor_tuple(entries[-1]["cursor"])[1] + 1
                    if entries and cursor_tuple(entries[-1]["cursor"])[0] == milliseconds
                    else 0
                )
                entries.append({"cursor": f"{milliseconds}-{sequence}", "event": event})
            await self._store.set(  # type: ignore[union-attr]
                self._report_key(definition.search_id, report["run_id"]), report, report_ttl
            )
            if replacement_baseline is not None:
                await self._store.set(  # type: ignore[union-attr]
                    self._baseline_key(definition.search_id), replacement_baseline, baseline_ttl
                )
            data = getattr(self._store, "_data", None)
            if data is not None:
                prefix = self._report_prefix(definition.search_id)
                keys = sorted(
                    (key for key in data if key.startswith(prefix)),
                    key=lambda key: float(data[key].get("finished_at", 0)),
                    reverse=True,
                )
                for key in keys[definition.max_reports :]:
                    data.pop(key, None)
            await self._store.set(  # type: ignore[union-attr]
                self._definition_key(definition.search_id), self._stored_payload(current), definition_ttl
            )
            if event:
                await self._store.set(  # type: ignore[union-attr]
                    self._event_stream(),
                    {"entries": entries},
                    self._event_retention_seconds + STORE_TTL_MARGIN,
                )
            return True

    async def read_events(
        self,
        consumer_id: str,
        *,
        cursor: str | None,
        limit: int,
        now: float | None = None,
    ) -> dict[str, Any]:
        """Read one ordered batch without changing consumer acknowledgement."""
        validate_consumer_id(consumer_id)
        requested = validate_cursor(cursor) if cursor is not None else await self.acknowledged_cursor(consumer_id)
        instant = time.time() if now is None else now
        cutoff_cursor = f"{max(0, int((instant - self._event_retention_seconds) * 1000))}-0"
        client = self._client()
        entries: list[tuple[str, dict[str, Any]]] = []
        first_cursor: str | None = None
        if client is not None and hasattr(client, "eval"):
            first, raw_entries = await client.eval(
                _READ_EVENTS,
                1,
                self._event_stream(),
                cutoff_cursor,
                requested,
                str(limit),
            )
            if first:
                first_cursor = first.decode() if isinstance(first, bytes) else str(first)
            for raw_cursor, fields in raw_entries:
                value_cursor = raw_cursor.decode() if isinstance(raw_cursor, bytes) else str(raw_cursor)
                if isinstance(fields, dict):
                    raw_payload = fields.get(b"payload", fields.get("payload"))
                else:
                    field_map = dict(zip(fields[::2], fields[1::2], strict=True))
                    raw_payload = field_map.get(b"payload", field_map.get("payload"))
                if isinstance(raw_payload, bytes):
                    raw_payload = raw_payload.decode()
                entries.append((value_cursor, json.loads(str(raw_payload))))
        else:
            async with self._lock:
                stream = await self._store.get(self._event_stream())  # type: ignore[union-attr]
                stored = stream.get("entries", []) if isinstance(stream, dict) else []
                retained = [item for item in stored if cursor_tuple(str(item["cursor"])) >= cursor_tuple(cutoff_cursor)]
                if len(retained) != len(stored):
                    await self._store.set(  # type: ignore[union-attr]
                        self._event_stream(),
                        {"entries": retained},
                        self._event_retention_seconds + STORE_TTL_MARGIN,
                    )
                if retained:
                    first_cursor = str(retained[0]["cursor"])
                entries = [
                    (str(item["cursor"]), dict(item["event"]))
                    for item in retained
                    if cursor_tuple(str(item["cursor"])) > cursor_tuple(requested)
                ][:limit]
        gap = first_cursor is None and requested != "0-0"
        if first_cursor is not None and requested != "0-0":
            gap = cursor_tuple(requested) < cursor_tuple(first_cursor)
        return {
            "requested_cursor": requested,
            "acknowledged_cursor": await self.acknowledged_cursor(consumer_id),
            "events": [(value_cursor, event) for value_cursor, event in entries],
            "next_cursor": entries[-1][0] if entries else requested,
            "gap": {
                "detected": gap,
                "reason": "retention_expired" if gap else None,
                "first_available_cursor": first_cursor,
            },
        }

    async def acknowledged_cursor(self, consumer_id: str) -> str:
        validate_consumer_id(consumer_id)
        client = self._client()
        if client is not None and hasattr(client, "get"):
            raw = await client.get(self._event_ack_key(consumer_id))
        else:
            raw = await self._store.get(self._event_ack_key(consumer_id))  # type: ignore[union-attr]
        if raw is None:
            return "0-0"
        if isinstance(raw, dict):
            return validate_cursor(str(raw.get("cursor", "0-0")))
        if isinstance(raw, bytes):
            raw = raw.decode()
        return validate_cursor(str(raw))

    async def acknowledge_event(self, consumer_id: str, cursor: str, *, now: float) -> str:
        """Advance one tenant/consumer fence monotonically and idempotently."""
        validate_consumer_id(consumer_id)
        validate_cursor(cursor)
        client = self._client()
        ttl = self._event_retention_seconds + STORE_TTL_MARGIN
        if client is not None and hasattr(client, "eval"):
            result = int(
                await client.eval(
                    _ACK_EVENT,
                    3,
                    self._event_ack_key(consumer_id),
                    self._event_consumers_key(),
                    self._event_stream(),
                    str(now),
                    cursor,
                    str(ttl),
                    str(now + ttl),
                    consumer_id,
                    str(self._event_consumers),
                    f"{max(0, int((now - self._event_retention_seconds) * 1000))}-0",
                )
            )
            if result == -3:
                raise LookupError("event stream is unavailable or expired")
            if result == -4:
                raise LookupError("event cursor expired from the tenant stream")
            if result == -2:
                raise ValueError("cursor is ahead of the tenant stream")
            if result == -1:
                raise OutboxCapacityError("saved-search event consumer capacity reached")
            return await self.acknowledged_cursor(consumer_id)
        async with self._lock:
            stream = await self._store.get(self._event_stream())  # type: ignore[union-attr]
            stored = stream.get("entries", []) if isinstance(stream, dict) else []
            cutoff_cursor = f"{max(0, int((now - self._event_retention_seconds) * 1000))}-0"
            entries = [item for item in stored if cursor_tuple(str(item["cursor"])) >= cursor_tuple(cutoff_cursor)]
            if len(entries) != len(stored):
                await self._store.set(  # type: ignore[union-attr]
                    self._event_stream(),
                    {"entries": entries},
                    self._event_retention_seconds + STORE_TTL_MARGIN,
                )
            if not entries:
                raise LookupError("event stream is unavailable or expired")
            if cursor_tuple(cursor) > cursor_tuple(str(entries[-1]["cursor"])):
                raise ValueError("cursor is ahead of the tenant stream")
            if cursor_tuple(cursor) < cursor_tuple(str(entries[0]["cursor"])):
                raise LookupError("event cursor expired from the tenant stream")
            data = getattr(self._store, "_data", {})
            prefix = f"{PREFIX}:event-ack:{self._tenant}:"
            consumers = [key for key in data if key.startswith(prefix)]
            key = self._event_ack_key(consumer_id)
            if key not in data and len(consumers) >= self._event_consumers:
                raise OutboxCapacityError("saved-search event consumer capacity reached")
            current = await self.acknowledged_cursor(consumer_id)
            if cursor_tuple(cursor) > cursor_tuple(current):
                await self._store.set(key, {"cursor": cursor}, ttl)  # type: ignore[union-attr]
                return cursor
            return current

    async def publish_expired_events(self, *, now: float, limit: int = 128) -> int:
        """Publish each logically expired definition once while its margin record remains."""
        if not self.available:
            return 0
        prefix = f"{PREFIX}:definition:"
        client = self._client()
        keys: list[str]
        if client is not None and hasattr(client, "scan"):
            cursor, raw_keys = await client.scan(self._expiry_scan_cursor, match=f"{prefix}*", count=limit)
            self._expiry_scan_cursor = int(cursor)
            keys = [item.decode() if isinstance(item, bytes) else str(item) for item in raw_keys]
        else:
            data = getattr(self._store, "_data", {})
            keys = [key for key in data if key.startswith(prefix)][:limit]
        published = 0
        for key in keys:
            payload = await self._store.get(key)  # type: ignore[union-attr]
            if not isinstance(payload, dict):
                continue
            definition = definition_from_payload(payload)
            if definition.expires_at > now:
                continue
            target = self.for_tenant(definition.tenant)
            event = definition_event(definition, "definition_expired", definition.expires_at)
            marker = target._expiry_event_marker(definition.search_id)
            target_client = target._client()
            if target_client is not None and hasattr(target_client, "eval"):
                result = int(
                    await target_client.eval(
                        _PUBLISH_EXPIRY,
                        3,
                        key,
                        marker,
                        target._event_stream(),
                        str(now),
                        f"{max(0, int((now - target._event_retention_seconds) * 1000))}-0",
                        str(target._event_capacity),
                        json.dumps(event, separators=(",", ":")),
                        str(target._event_retention_seconds + STORE_TTL_MARGIN),
                    )
                )
                if result == -1:
                    raise OutboxCapacityError("saved-search event outbox is at capacity")
                published += int(result == 1)
                continue
            async with target._lock:
                data = getattr(target._store, "_data", {})
                if marker in data:
                    continue
                stream = await target._store.get(target._event_stream())  # type: ignore[union-attr]
                entries = list(stream.get("entries", [])) if isinstance(stream, dict) else []
                cutoff = now - target._event_retention_seconds
                entries = [item for item in entries if float(item["event"]["occurred_at"]) > cutoff]
                if len(entries) >= target._event_capacity:
                    raise OutboxCapacityError("saved-search event outbox is at capacity")
                milliseconds = max(int(now * 1000), cursor_tuple(entries[-1]["cursor"])[0] if entries else 0)
                sequence = (
                    cursor_tuple(entries[-1]["cursor"])[1] + 1
                    if entries and cursor_tuple(entries[-1]["cursor"])[0] == milliseconds
                    else 0
                )
                entries.append({"cursor": f"{milliseconds}-{sequence}", "event": event})
                ttl = target._event_retention_seconds + STORE_TTL_MARGIN
                await target._store.set(target._event_stream(), {"entries": entries}, ttl)  # type: ignore[union-attr]
                await target._store.set(marker, {"published": True}, ttl)  # type: ignore[union-attr]
                published += 1
        return published

    async def renew(self, definition: SavedDefinition) -> bool:
        """Renew only the claim token held by this execution."""
        token = definition.lease_token
        if not token or not self.available:
            return False
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            return bool(await client.eval(_RENEW, 1, self._lease_key(definition.search_id), token, str(LEASE_SECONDS)))
        latest = await self.load(definition.search_id)
        if latest is None or latest.lease_token != token:
            return False
        latest.lease_expires_at = time.time() + LEASE_SECONDS
        await self._store.set(  # type: ignore[union-attr]
            self._definition_key(definition.search_id),
            self._stored_payload(latest),
            self._ttl(latest, time.time()),
        )
        return True

    async def release(self, definition: SavedDefinition) -> bool:
        """Release a claim without mutating its durable definition."""
        token = definition.lease_token
        if not token or not self.available:
            return False
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            return bool(await client.eval(_RELEASE, 1, self._lease_key(definition.search_id), token))
        latest = await self.load(definition.search_id)
        if latest is None or latest.lease_token != token:
            return False
        latest.lease_token = None
        latest.lease_expires_at = 0
        await self._store.set(  # type: ignore[union-attr]
            self._definition_key(definition.search_id),
            self._stored_payload(latest),
            self._ttl(latest, time.time()),
        )
        return True

    async def reports(self, search_id: str, *, now: float, limit: int = 20) -> list[dict[str, Any]]:
        definition = await self.load(search_id, now=now)
        if definition is None:
            return []
        client = self._client()
        ids: list[str] = []
        if client is not None and hasattr(client, "zrevrange"):
            raw = await client.zrevrange(self._report_index(search_id), 0, min(limit, definition.max_reports) - 1)
            ids = [item.decode() if isinstance(item, bytes) else str(item) for item in raw]
        else:
            data = getattr(self._store, "_data", {})
            prefix = self._report_prefix(search_id)
            ids = [key[len(prefix) :] for key in data if key.startswith(prefix)][-limit:]
            ids.reverse()
        values = [await self._store.get(self._report_key(search_id, item)) for item in ids]  # type: ignore[union-attr]
        return [item for item in values if item and float(item.get("expires_at", 0)) > now]

    async def repair_indexes(self, *, now: float, limit: int = 128) -> int:
        """Restore due hints from bounded authoritative record scans."""
        repaired = 0
        for definition in await self.scan_definitions(now=now, limit=limit):
            client = self._client()
            if client is not None and hasattr(client, "zadd"):
                await client.zadd(self._definition_index(), {definition.search_id: definition.expires_at})
                if not definition.paused:
                    await client.zadd(self._due_index(), {definition.search_id: definition.next_due})
                ttl = self._ttl(definition, now)
                await client.expire(self._definition_index(), ttl)
                await client.expire(self._due_index(), ttl)
                await client.zadd(self._tenant_index(), {self._tenant: definition.expires_at}, gt=True)
                repaired += 1
        return repaired
