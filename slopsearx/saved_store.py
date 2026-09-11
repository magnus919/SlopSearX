"""Tenant-scoped Valkey persistence for scheduled saved searches."""

from __future__ import annotations

import asyncio
import json
import secrets
import time
from typing import Any

from slopsearx.saved_models import SavedDefinition, definition_from_payload, definition_to_payload
from slopsearx.snapshot import KeyValueStore

PREFIX = "mcp:saved:v1"
STORE_TTL_MARGIN = 300
LEASE_SECONDS = 60

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
if redis.call('GET', KEYS[2]) ~= ARGV[1] then return -2 end
local raw = redis.call('GET', KEYS[1])
if not raw then return -1 end
local current = cjson.decode(raw)
if tonumber(current.revision) ~= tonumber(ARGV[2]) or current.paused or current.deleted or
   tonumber(current.expires_at) <= tonumber(ARGV[3]) or current.policy_fingerprint ~= ARGV[4] then return -3 end
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


class SavedSearchStore:
    """Durable authority; indexes are bounded, repairable scheduling hints."""

    def __init__(self, store: KeyValueStore | None, tenant: str = "default") -> None:
        self._store = store
        self._tenant = tenant
        self._lock = asyncio.Lock()
        self._children: dict[str, SavedSearchStore] = {}
        self._definition_scan_cursor = 0
        self._tenant_scan_cursor = 0

    @property
    def available(self) -> bool:
        return self._store is not None and self._store.is_connected

    def for_tenant(self, tenant: str) -> "SavedSearchStore":
        if tenant == self._tenant:
            return self
        if tenant not in self._children:
            self._children[tenant] = SavedSearchStore(self._store, tenant)
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
        self, definition: SavedDefinition, expected_revision: int, *, now: float
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
                    4,
                    self._definition_key(definition.search_id),
                    self._due_index(),
                    self._baseline_key(definition.search_id),
                    self._lease_key(definition.search_id),
                    str(now),
                    str(expected_revision),
                    str(self._ttl(definition, now)),
                    payload,
                    "paused" if definition.paused else "active",
                    definition.search_id,
                    str(definition.next_due),
                )
            )
            if result == -1:
                raise LookupError(definition.search_id)
            if result != 0:
                raise RevisionConflictError(result)
        else:
            async with self._lock:
                latest = await self.load(definition.search_id, now=now)
                if latest is None:
                    raise LookupError(definition.search_id)
                if latest.revision != expected_revision:
                    raise RevisionConflictError(latest.revision)
                data = getattr(self._store, "_data", None)
                if data is not None:
                    data.pop(self._baseline_key(definition.search_id), None)
                await self._store.set(  # type: ignore[union-attr]
                    self._definition_key(definition.search_id),
                    self._stored_payload(definition),
                    self._ttl(definition, now),
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
                6,
                self._definition_key(definition.search_id),
                self._lease_key(definition.search_id),
                self._report_key(definition.search_id, report["run_id"]),
                self._report_index(definition.search_id),
                self._due_index(),
                self._baseline_key(definition.search_id),
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
            )
            return bool(result == 1)
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
            return True

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
