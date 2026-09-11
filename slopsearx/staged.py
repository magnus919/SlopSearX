"""Durable, bounded two-stage MCP search operations.

The workflow stores its state in the shared cache and executes each captured
engine scope through :class:`SearchService`.  Presentation options never form
part of operation identity and ordinary HTTP searches do not use this module.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import secrets
import time
import uuid
from dataclasses import dataclass
from typing import Any, Callable

from slopsearx import metrics as m
from slopsearx.artifacts import artifact_ref
from slopsearx.service import SearchRequest, SearchService
from slopsearx.snapshot import SnapshotStore

CONTRACT = "slopsearx.staged_search"
VERSION = 1
PREFIX = "mcp:staged:v1"
RETENTION_SECONDS = 3600
STORE_MARGIN_SECONDS = 300
MAX_OPERATIONS_PER_TENANT = 32
LEASE_SECONDS = 60

_ADMIT_SCRIPT = """
local existing = redis.call('GET', KEYS[2])
if existing then
  local mapped = cjson.decode(existing)
  local raw = redis.call('GET', mapped.record_key)
  if raw then
    local current = cjson.decode(raw)
    if current.plan_digest == ARGV[1] then return {'replay', raw} end
    return {'conflict', ''}
  end
end
redis.call('ZREMRANGEBYSCORE', KEYS[3], '-inf', ARGV[2])
if redis.call('ZCARD', KEYS[3]) == 0 and redis.call('EXISTS', KEYS[5]) == 1 then
  local page=redis.call('SCAN','0','MATCH',ARGV[7],'COUNT',256)
  if page[1] ~= '0' then return {'unavailable',''} end
  for _, key in ipairs(page[2]) do
    local candidate=redis.call('GET',key)
    if candidate then
      local ok,item=pcall(cjson.decode,candidate)
      if not ok then return {'unavailable',''} end
      if tonumber(item.expires_at) > tonumber(ARGV[2]) then
        redis.call('ZADD',KEYS[3],item.expires_at,item.operation_id)
      end
    end
  end
end
if redis.call('ZCARD', KEYS[3]) >= tonumber(ARGV[3]) then return {'quota', ''} end
if redis.call('EXISTS', KEYS[1]) == 1 then return {'conflict', ''} end
redis.call('SETEX', KEYS[1], ARGV[4], ARGV[5])
redis.call('SETEX', KEYS[2], ARGV[4], cjson.encode({record_key=KEYS[1]}))
redis.call('ZADD', KEYS[3], ARGV[6], ARGV[8])
redis.call('EXPIRE', KEYS[3], ARGV[4])
redis.call('SETEX', KEYS[5], ARGV[4], '1')
redis.call('ZADD', KEYS[4], ARGV[2], KEYS[1])
return {'created', ARGV[5]}
"""

_CLAIM_SCRIPT = """
local raw = redis.call('GET', KEYS[1])
if not raw then redis.call('ZREM', KEYS[3], KEYS[1]); return {'missing',''} end
local current = cjson.decode(raw)
if current.state ~= 'queued' then redis.call('ZREM', KEYS[3], KEYS[1]); return {'not_queued',''} end
if tonumber(current.execution_deadline_at) <= tonumber(ARGV[1]) or
   tonumber(current.expires_at) <= tonumber(ARGV[1]) then
  current.state='failed'; current.stop_reason='deadline_expired'
  current.objectives.unmet={{objective='deadline_ms',reason='deadline_expired'}}
  local encoded=cjson.encode(current)
  redis.call('SETEX', KEYS[1], ARGV[3], encoded)
  redis.call('ZREM', KEYS[3], KEYS[1]); return {'deadline',encoded}
end
if redis.call('SET', KEYS[2], ARGV[2], 'NX', 'EX', ARGV[4]) == false then return {'busy',''} end
local stage = current.stages[tonumber(current.next_stage) + 1]
local cost = #stage.scope.selected_engines
if tonumber(current.budget.reserved) + cost > tonumber(current.budget.limit) then
  current.state='failed'; current.stop_reason='budget_exhausted'
  current.objectives.unmet={{objective='max_engine_calls',reason='budget_exhausted'}}
  local encoded=cjson.encode(current)
  redis.call('SETEX', KEYS[1], ARGV[3], encoded)
  redis.call('DEL', KEYS[2]); redis.call('ZREM', KEYS[3], KEYS[1]); return {'budget',encoded}
end
current.state='running'; current.owner_token=ARGV[2]
current.budget.reserved=tonumber(current.budget.reserved)+cost
stage.state='running'
local attempt={attempt_id=ARGV[5],started_at=tonumber(ARGV[6]),finished_at=cjson.null,
  state='running',reserved_engine_calls=cost,observed_engine_calls=cjson.null,cached=false,
  query_id=cjson.null,cursor=cjson.null,result_count=0,engine_outcomes={},
  enforcement=stage.enforcement,error=cjson.null,deadline_exceeded=false}
stage.attempts[#stage.attempts+1]=attempt
redis.call('SETEX', KEYS[1], ARGV[3], cjson.encode(current))
redis.call('ZREM', KEYS[3], KEYS[1])
redis.call('ZADD', KEYS[4], tonumber(ARGV[1])+tonumber(ARGV[4]), KEYS[1])
return {'claimed',cjson.encode(current)}
"""

_OWNED_SAVE_SCRIPT = """
if redis.call('GET', KEYS[2]) ~= ARGV[1] then return 0 end
redis.call('SETEX', KEYS[1], ARGV[2], ARGV[3])
redis.call('ZREM', KEYS[4], KEYS[1])
if ARGV[4] == 'queued' then redis.call('ZADD', KEYS[3], ARGV[5], KEYS[1]) end
redis.call('DEL', KEYS[2])
return 1
"""

_RENEW_SCRIPT = """
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
redis.call('EXPIRE', KEYS[1], ARGV[2])
redis.call('ZADD', KEYS[2], ARGV[3], ARGV[4])
return 1
"""

_INTERRUPT_SCRIPT = """
if redis.call('EXISTS', KEYS[2]) == 1 then return 0 end
local raw=redis.call('GET', KEYS[1]); if not raw then redis.call('ZREM', KEYS[3], KEYS[1]); return 0 end
local current=cjson.decode(raw)
if current.state ~= 'running' then redis.call('ZREM', KEYS[3], KEYS[1]); return 0 end
local stage=current.stages[tonumber(current.next_stage)+1]
local attempt=stage.attempts[#stage.attempts]
attempt.state='interrupted'; attempt.finished_at=tonumber(ARGV[1]); attempt.observed_engine_calls=cjson.null
stage.state='interrupted'; current.state='interrupted'; current.stop_reason='interrupted'
current.owner_token=cjson.null; current.budget.observed=cjson.null
redis.call('SETEX', KEYS[1], ARGV[2], cjson.encode(current)); redis.call('ZREM', KEYS[3], KEYS[1]); return 1
"""

_RETRY_SCRIPT = """
local raw=redis.call('GET', KEYS[1]); if not raw then return {'missing',''} end
local current=cjson.decode(raw)
if tonumber(current.expires_at) <= tonumber(ARGV[1]) then return {'expired',''} end
for _, key in ipairs(current.retry_keys) do if key == ARGV[2] then return {'replay',raw} end end
if current.state == 'queued' or current.state == 'running' then return {'busy',raw} end
if current.state ~= 'failed' and current.state ~= 'interrupted' then return {'none',raw} end
local target=nil
for i=#current.stages,1,-1 do
  if current.stages[i].state == 'failed' or current.stages[i].state == 'interrupted' then
    target=current.stages[i]; break
  end
end
if not target then return {'none',raw} end
local cost=#target.scope.selected_engines
if tonumber(current.budget.reserved)+cost > tonumber(current.budget.limit) then return {'budget',raw} end
if tonumber(current.execution_deadline_at) <= tonumber(ARGV[1]) then return {'deadline',raw} end
current.retry_keys[#current.retry_keys+1]=ARGV[2]
current.state='queued'; current.stop_reason=cjson.null; current.next_stage=target.index; current.owner_token=cjson.null
local encoded=cjson.encode(current)
redis.call('SETEX', KEYS[1], ARGV[3], encoded)
redis.call('ZADD', KEYS[2], ARGV[1], KEYS[1])
return {'queued',encoded}
"""


def plan_digest(plan: dict[str, Any]) -> str:
    """Return the stable identity of a resolved execution plan."""
    encoded = json.dumps(plan, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(encoded.encode()).hexdigest()


def _normalize_record(record: dict[str, Any]) -> dict[str, Any]:
    """Restore array fields that Valkey Lua cjson serializes as empty objects."""
    if isinstance(record.get("retry_keys"), dict):
        record["retry_keys"] = []
    objectives = record.get("objectives") or {}
    if isinstance(objectives.get("unmet"), dict):
        objectives["unmet"] = []
    plan = record.get("plan") or {}
    for name in ("initial_scope", "fallback_scope"):
        planned_scope = plan.get(name)
        if isinstance(planned_scope, dict) and isinstance(planned_scope.get("excluded_engines"), dict):
            planned_scope["excluded_engines"] = []
    for stage in record.get("stages") or []:
        scope = stage.get("scope") or {}
        if isinstance(scope.get("excluded_engines"), dict):
            scope["excluded_engines"] = []
        for entry in (stage.get("enforcement") or {}).values():
            if isinstance(entry, dict) and isinstance(entry.get("enforced_by"), dict):
                entry["enforced_by"] = []
        if isinstance(stage.get("attempts"), dict):
            stage["attempts"] = []
        for attempt in stage.get("attempts") or []:
            if isinstance(attempt.get("engine_outcomes"), dict):
                attempt["engine_outcomes"] = []
            for entry in (attempt.get("enforcement") or {}).values():
                if isinstance(entry, dict) and isinstance(entry.get("enforced_by"), dict):
                    entry["enforced_by"] = []
    return record


@dataclass
class StoreRead:
    record: dict[str, Any] | None = None
    unavailable: bool = False
    expired: bool = False
    expires_at: float | None = None


class StagedSearchStore:
    """Tenant-scoped operation store with fixed logical retention."""

    _locks: dict[int, asyncio.Lock] = {}

    def __init__(self, store: Any) -> None:
        self._store = store
        self._lock = self._locks.setdefault(id(store), asyncio.Lock())
        self._scan_cursor = 0

    @property
    def available(self) -> bool:
        return self._store is not None and bool(self._store.is_connected)

    @staticmethod
    def _tenant_hash(tenant: str) -> str:
        return hashlib.sha256(tenant.encode()).hexdigest()[:24]

    def _key(self, tenant: str, operation_id: str) -> str:
        return f"{PREFIX}:record:{self._tenant_hash(tenant)}:{operation_id}"

    def _index_key(self, tenant: str) -> str:
        return f"{PREFIX}:index:{self._tenant_hash(tenant)}"

    def _idem_key(self, tenant: str, idempotency_key: str) -> str:
        digest = hashlib.sha256(idempotency_key.encode()).hexdigest()
        return f"{PREFIX}:idem:{self._tenant_hash(tenant)}:{digest}"

    @staticmethod
    def _ready_key() -> str:
        return f"{PREFIX}:ready"

    @staticmethod
    def _running_key() -> str:
        return f"{PREFIX}:running"

    def _lease_key(self, tenant: str, operation_id: str) -> str:
        return f"{PREFIX}:lease:{self._tenant_hash(tenant)}:{operation_id}"

    def _marker_key(self, tenant: str) -> str:
        return f"{PREFIX}:tenant:{self._tenant_hash(tenant)}"

    def _record_pattern(self, tenant: str) -> str:
        return f"{PREFIX}:record:{self._tenant_hash(tenant)}:*"

    def _client(self) -> Any:
        return getattr(self._store, "_client", None)

    @staticmethod
    def _decode(value: Any) -> str:
        return value.decode() if isinstance(value, bytes) else str(value)

    async def _read_index(self, tenant: str) -> dict[str, float]:
        payload = await self._store.get(self._index_key(tenant))
        return {str(k): float(v) for k, v in (payload or {}).items()}

    async def _write_index(self, tenant: str, index: dict[str, float]) -> None:
        await self._store.set(self._index_key(tenant), index, STORE_MARGIN_SECONDS + RETENTION_SECONDS)

    async def find_by_idempotency(self, tenant: str, idempotency_key: str) -> StoreRead:
        """Read an accepted operation without recomputing its captured routing."""
        if not self.available:
            return StoreRead(unavailable=True)
        client = self._client()
        operation_id = ""
        raw = await self._store.get(self._idem_key(tenant, idempotency_key))
        if raw:
            record_key = str(raw.get("record_key", ""))
            payload = await self._store.get(record_key) if record_key else None
            if payload:
                operation_id = str(payload.get("operation_id", ""))
        if not operation_id and client is None:
            index = await self._read_index(tenant)
            digest = hashlib.sha256(idempotency_key.encode()).hexdigest()
            prefix = f"idem:{digest}:"
            operation_id = next((key[len(prefix) :] for key in index if key.startswith(prefix)), "")
        return await self.read(tenant, operation_id) if operation_id else StoreRead()

    async def admit(
        self, tenant: str, idempotency_key: str, digest: str, record: dict[str, Any]
    ) -> tuple[str, dict[str, Any] | None]:
        if not self.available:
            return "unavailable", None
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            now = time.time()
            ttl = max(1, int(float(record["expires_at"]) - now) + STORE_MARGIN_SECONDS)
            raw = json.dumps(record, separators=(",", ":"))
            try:
                result = await client.eval(
                    _ADMIT_SCRIPT,
                    5,
                    self._key(tenant, str(record["operation_id"])),
                    self._idem_key(tenant, idempotency_key),
                    self._index_key(tenant),
                    self._ready_key(),
                    self._marker_key(tenant),
                    digest,
                    str(now),
                    str(MAX_OPERATIONS_PER_TENANT),
                    str(ttl),
                    raw,
                    str(record["expires_at"]),
                    self._record_pattern(tenant),
                    str(record["operation_id"]),
                )
            except Exception:  # noqa: BLE001 - fail closed on authority error
                return "unavailable", None
            status = self._decode(result[0])
            payload = self._decode(result[1])
            return status, (_normalize_record(json.loads(payload)) if payload else None)
        async with self._lock:
            now = time.time()
            index = {key: expiry for key, expiry in (await self._read_index(tenant)).items() if expiry > now}
            idem_hash = hashlib.sha256(idempotency_key.encode()).hexdigest()
            idem_slot = f"idem:{idem_hash}"
            idem_prefix = idem_slot + ":"
            existing_id = next((key[len(idem_prefix) :] for key in index if key.startswith(idem_prefix)), None)
            if existing_id:
                existing = await self._store.get(self._key(tenant, existing_id))
                if existing is not None:
                    return ("replay" if existing.get("plan_digest") == digest else "conflict"), existing
            operations = [key for key in index if not key.startswith("idem:")]
            if len(operations) >= MAX_OPERATIONS_PER_TENANT:
                return "quota", None
            operation_id = str(record["operation_id"])
            expires_at = float(record["expires_at"])
            ttl = max(1, int(expires_at - now) + STORE_MARGIN_SECONDS)
            await self._store.set(self._key(tenant, operation_id), record, ttl)
            await self._store.set(
                self._idem_key(tenant, idempotency_key),
                {"record_key": self._key(tenant, operation_id)},
                ttl,
            )
            index[operation_id] = expires_at
            index[f"{idem_slot}:{operation_id}"] = expires_at
            await self._write_index(tenant, index)
            return "created", record

    async def claim(self, tenant: str, operation_id: str) -> tuple[dict[str, Any], str] | None:
        """Atomically reserve a stage and claim its execution lease."""
        read = await self.read(tenant, operation_id)
        if read.record is None or read.record.get("state") != "queued":
            return None
        token = secrets.token_hex(24)
        record = read.record
        stage = record["stages"][int(record.get("next_stage", 0))]
        engines = list(stage["scope"]["selected_engines"])
        attempt_id = uuid.uuid4().hex
        started_at = time.time()
        attempt = {
            "attempt_id": attempt_id,
            "started_at": started_at,
            "finished_at": None,
            "state": "running",
            "reserved_engine_calls": len(engines),
            "observed_engine_calls": None,
            "cached": False,
            "query_id": None,
            "cursor": None,
            "result_count": 0,
            "engine_outcomes": [],
            "enforcement": stage["enforcement"],
            "error": None,
            "deadline_exceeded": False,
        }
        client = self._client()
        ttl = max(1, int(float(record["expires_at"]) - time.time()) + STORE_MARGIN_SECONDS)
        if client is not None and hasattr(client, "eval"):
            raw = await client.eval(
                _CLAIM_SCRIPT,
                4,
                self._key(tenant, operation_id),
                self._lease_key(tenant, operation_id),
                self._ready_key(),
                self._running_key(),
                str(time.time()),
                token,
                str(ttl),
                str(LEASE_SECONDS),
                attempt_id,
                str(started_at),
            )
            if isinstance(raw, (list, tuple)):
                status = self._decode(raw[0])
                decoded = self._decode(raw[1])
                if status in {"deadline", "budget"}:
                    if status == "deadline":
                        m.record_workflow_expiry("staged_search", "operation")
                    else:
                        m.record_workflow_rejection("staged_search", "budget")
                    m.transition_workflow("staged_search", "queued", "failed")
                    m.record_workflow_terminal("staged_search", "failed")
                    return None
                if status != "claimed":
                    return None
            else:  # Legacy/injected clients may return the pre-status payload.
                decoded = self._decode(raw)
            if not decoded:
                return None
            claimed = _normalize_record(json.loads(decoded))
            m.transition_workflow("staged_search", "queued", "running")
            m.workflow_queue_wait.observe(
                {"workflow": "staged_search"}, max(0.0, started_at - float(claimed["accepted_at"]))
            )
            return claimed, token
        async with self._lock:
            fresh = await self.read(tenant, operation_id)
            fresh_record = fresh.record
            if fresh_record is None or fresh_record.get("state") != "queued":
                return None
            record = fresh_record
            stage = record["stages"][int(record.get("next_stage", 0))]
            cost = len(stage["scope"]["selected_engines"])
            if record["budget"]["reserved"] + cost > record["budget"]["limit"]:
                record.update(state="failed", stop_reason="budget_exhausted")
                await self.save(tenant, record)
                m.record_workflow_rejection("staged_search", "budget")
                m.transition_workflow("staged_search", "queued", "failed")
                m.record_workflow_terminal("staged_search", "failed")
                return None
            record["state"] = "running"
            record["owner_token"] = token
            record["budget"]["reserved"] += cost
            stage["state"] = "running"
            stage["attempts"].append(attempt)
            await self.save(tenant, record)
            m.transition_workflow("staged_search", "queued", "running")
            m.workflow_queue_wait.observe(
                {"workflow": "staged_search"}, max(0.0, started_at - float(record["accepted_at"]))
            )
            return record, token

    async def claim_next(self) -> tuple[str, str] | None:
        """Return a durable ready candidate from any tenant."""
        client = self._client()
        if client is None or not hasattr(client, "zrangebyscore"):
            return None
        for _ in range(16):
            candidates = await client.zrangebyscore(self._ready_key(), "-inf", time.time(), start=0, num=1)
            if not candidates:
                return None
            record_key = self._decode(candidates[0])
            payload = await self._store.get(record_key)
            if (
                payload is None
                or float(payload.get("expires_at", 0)) <= time.time()
                or payload.get("state") != "queued"
            ):
                await client.zrem(self._ready_key(), record_key)
                continue
            return str(payload.get("tenant", "")), str(payload.get("operation_id", ""))
        return None

    async def save_owned(self, tenant: str, record: dict[str, Any], token: str) -> bool:
        """Finalize only while the supplied lease token remains authoritative."""
        client = self._client()
        ttl = max(1, int(float(record["expires_at"]) - time.time()) + STORE_MARGIN_SECONDS)
        if client is not None and hasattr(client, "eval"):
            result = await client.eval(
                _OWNED_SAVE_SCRIPT,
                4,
                self._key(tenant, str(record["operation_id"])),
                self._lease_key(tenant, str(record["operation_id"])),
                self._ready_key(),
                self._running_key(),
                token,
                str(ttl),
                json.dumps(record, separators=(",", ":")),
                str(record["state"]),
                str(time.time()),
            )
            return bool(result)
        if record.get("owner_token") != token:
            return False
        record["owner_token"] = None
        return await self.save(tenant, record)

    async def renew(self, tenant: str, operation_id: str, token: str) -> bool:
        client = self._client()
        if client is None or not hasattr(client, "eval"):
            return True
        result = await client.eval(
            _RENEW_SCRIPT,
            2,
            self._lease_key(tenant, operation_id),
            self._running_key(),
            token,
            str(LEASE_SECONDS),
            str(time.time() + LEASE_SECONDS),
            self._key(tenant, operation_id),
        )
        return bool(result)

    async def recover_orphans(self) -> int:
        """Classify expired-lease attempts as interrupted without retrying."""
        client = self._client()
        if client is None or not hasattr(client, "zrangebyscore"):
            return 0
        keys = await client.zrangebyscore(self._running_key(), "-inf", time.time(), start=0, num=32)
        recovered = 0
        for raw_key in keys:
            record_key = self._decode(raw_key)
            payload = await self._store.get(record_key)
            if payload is None:
                await client.zrem(self._running_key(), record_key)
                continue
            tenant = str(payload.get("tenant", ""))
            operation_id = str(payload.get("operation_id", ""))
            ttl = max(1, int(float(payload.get("expires_at", 0)) - time.time()) + STORE_MARGIN_SECONDS)
            result = await client.eval(
                _INTERRUPT_SCRIPT,
                3,
                record_key,
                self._lease_key(tenant, operation_id),
                self._running_key(),
                str(time.time()),
                str(ttl),
            )
            recovered += int(bool(result))
        m.record_workflow_recovery("staged_search", recovered)
        for _ in range(recovered):
            m.transition_workflow("staged_search", "running", "interrupted")
            m.record_workflow_terminal("staged_search", "interrupted")
        return recovered

    async def reconcile_indexes(self) -> None:
        """Repair bounded ready/running hints from authoritative records."""
        client = self._client()
        if client is None or not hasattr(client, "scan"):
            return
        cursor, keys = await client.scan(self._scan_cursor, match=f"{PREFIX}:record:*", count=128)
        self._scan_cursor = int(cursor)
        now = time.time()
        for raw_key in keys:
            record_key = self._decode(raw_key)
            payload = await self._store.get(record_key)
            if payload is None or float(payload.get("expires_at", 0)) <= now:
                continue
            tenant = str(payload.get("tenant", ""))
            operation_id = str(payload.get("operation_id", ""))
            if payload.get("state") == "queued":
                await client.zadd(self._ready_key(), {record_key: now}, nx=True)
            elif payload.get("state") == "running":
                lease_key = self._lease_key(tenant, operation_id)
                lease_ttl = await client.ttl(lease_key)
                if lease_ttl > 0:
                    await client.zadd(self._running_key(), {record_key: now + lease_ttl})
                else:
                    ttl = max(1, int(float(payload["expires_at"]) - now) + STORE_MARGIN_SECONDS)
                    await client.eval(
                        _INTERRUPT_SCRIPT,
                        3,
                        record_key,
                        lease_key,
                        self._running_key(),
                        str(now),
                        str(ttl),
                    )

    async def read(self, tenant: str, operation_id: str) -> StoreRead:
        if not self.available:
            return StoreRead(unavailable=True)
        payload = await self._store.get(self._key(tenant, operation_id))
        if payload is None:
            return StoreRead()
        payload = _normalize_record(payload)
        expires_at = float(payload.get("expires_at", 0))
        if time.time() > expires_at:
            return StoreRead(expired=True, expires_at=expires_at)
        return StoreRead(record=payload)

    async def list_recent(
        self, tenant: str, *, before: tuple[float, str] | None = None, limit: int = 20
    ) -> list[dict[str, Any]]:
        """Read a bounded deterministic page from the maintained tenant index."""
        if not self.available or not 1 <= limit <= 50:
            return []
        now = time.time()
        client = self._client()
        if client is not None and hasattr(client, "zrevrange"):
            raw = await client.zrevrange(self._index_key(tenant), 0, MAX_OPERATIONS_PER_TENANT - 1, withscores=True)
            index = {self._decode(key): float(expiry) for key, expiry in raw if float(expiry) > now}
        else:
            index = {key: expiry for key, expiry in (await self._read_index(tenant)).items() if expiry > now}
        record_prefix = f"{PREFIX}:record:{self._tenant_hash(tenant)}:"
        index = {
            (key[len(record_prefix) :] if key.startswith(record_prefix) else key): expiry
            for key, expiry in index.items()
        }
        operation_ids = [key for key in index if not key.startswith("idem:")]
        reads = await asyncio.gather(*(self.read(tenant, operation_id) for operation_id in operation_ids))
        records = [item.record for item in reads if item.record is not None]
        records.sort(
            key=lambda item: (float(item.get("accepted_at", 0)), str(item.get("operation_id", ""))), reverse=True
        )
        if before is not None:
            records = [
                item
                for item in records
                if (float(item.get("accepted_at", 0)), str(item.get("operation_id", ""))) < before
            ]
        return records[:limit]

    async def save(self, tenant: str, record: dict[str, Any]) -> bool:
        """Save without extending the accepted retention horizon."""
        if not self.available:
            return False
        ttl = int(float(record["expires_at"]) - time.time()) + STORE_MARGIN_SECONDS
        if ttl <= STORE_MARGIN_SECONDS:
            return False
        await self._store.set(self._key(tenant, str(record["operation_id"])), record, ttl)
        return True

    async def request_retry(self, tenant: str, operation_id: str, retry_key: str) -> tuple[str, dict[str, Any] | None]:
        if not self.available:
            return "unavailable", None
        client = self._client()
        if client is not None and hasattr(client, "eval"):
            read = await self.read(tenant, operation_id)
            if read.expired:
                return "expired", None
            if read.record is None:
                return "missing", None
            ttl = max(1, int(float(read.record["expires_at"]) - time.time()) + STORE_MARGIN_SECONDS)
            result = await client.eval(
                _RETRY_SCRIPT,
                2,
                self._key(tenant, operation_id),
                self._ready_key(),
                str(time.time()),
                retry_key,
                str(ttl),
            )
            status = self._decode(result[0])
            payload = self._decode(result[1])
            return status, (_normalize_record(json.loads(payload)) if payload else None)
        async with self._lock:
            read = await self.read(tenant, operation_id)
            if read.expired:
                return "expired", None
            record = read.record
            if record is None:
                return "missing", None
            keys = record.setdefault("retry_keys", [])
            if retry_key in keys:
                return "replay", record
            if record["state"] in {"queued", "running"}:
                return "busy", record
            if record["state"] not in {"failed", "interrupted"}:
                return "none", record
            stage = next(
                (item for item in reversed(record["stages"]) if item["state"] in {"failed", "interrupted"}), None
            )
            if stage is None:
                return "none", record
            cost = len(stage["scope"]["selected_engines"])
            if record["budget"]["reserved"] + cost > record["budget"]["limit"]:
                return "budget", record
            if time.time() >= record["execution_deadline_at"]:
                return "deadline", record
            keys.append(retry_key)
            record["state"] = "queued"
            record["stop_reason"] = None
            record["next_stage"] = stage["index"]
            await self.save(tenant, record)
            return "queued", record


PolicyCheck = Callable[[dict[str, Any]], dict[str, Any] | None]


class StagedSearchRunner:
    """Executes accepted operations and preserves per-attempt evidence."""

    def __init__(
        self,
        service: SearchService,
        store: StagedSearchStore,
        snapshots: SnapshotStore,
        policy_check: PolicyCheck,
    ) -> None:
        self.service = service
        self.store = store
        self.snapshots = snapshots
        self.policy_check = policy_check
        self._queue: asyncio.Queue[tuple[str, str]] = asyncio.Queue()

    async def enqueue(self, tenant: str, operation_id: str) -> None:
        await self._queue.put((tenant, operation_id))

    async def run_forever(self) -> None:
        while True:
            try:
                local = True
                try:
                    tenant, operation_id = self._queue.get_nowait()
                except asyncio.QueueEmpty:
                    local = False
                    await self.store.reconcile_indexes()
                    await self.store.recover_orphans()
                    candidate = await self.store.claim_next()
                    if candidate is None:
                        await asyncio.sleep(1)
                        continue
                    tenant, operation_id = candidate
                await self.run_one(tenant, operation_id)
            except asyncio.CancelledError:
                raise
            except Exception:  # noqa: BLE001 - transient authority errors must not kill the worker
                await asyncio.sleep(1)
            finally:
                if locals().get("local", False):
                    self._queue.task_done()

    async def run_one(self, tenant: str, operation_id: str) -> None:
        claimed = await self.store.claim(tenant, operation_id)
        if claimed is None:
            return
        record, lease_token = claimed
        execution_started = time.monotonic()
        stage_index = int(record.get("next_stage", 0))
        stage = record["stages"][stage_index]
        attempt = stage["attempts"][-1]
        rejection = self.policy_check(record)
        if rejection:
            attempt.update(
                state="failed",
                finished_at=time.time(),
                observed_engine_calls=0,
                error={"code": "policy_rejected", "message": rejection["error"]["message"]},
            )
            stage["state"] = "failed"
            record.update(state="failed", stop_reason="policy_rejected", error=rejection.get("error"))
            record["objectives"]["unmet"].append({"objective": "fallback", "reason": "policy_rejected"})
            if await self.store.save_owned(tenant, record, lease_token):
                m.record_workflow_rejection("staged_search", "policy")
                m.transition_workflow("staged_search", "running", "failed")
                m.record_workflow_terminal("staged_search", "failed", time.monotonic() - execution_started)
            return
        engines = list(stage["scope"]["selected_engines"])
        remaining_ms = int((record["execution_deadline_at"] - time.time()) * 1000)
        if remaining_ms <= 0:
            attempt.update(
                state="failed",
                finished_at=time.time(),
                observed_engine_calls=0,
                error={"code": "deadline_exceeded", "message": "deadline expired before dispatch"},
                deadline_exceeded=True,
            )
            stage["state"] = "failed"
            record.update(state="failed", stop_reason="deadline_expired")
            record["objectives"]["unmet"].append({"objective": "deadline_ms", "reason": "deadline_expired"})
            if await self.store.save_owned(tenant, record, lease_token):
                m.record_workflow_expiry("staged_search", "operation")
                m.transition_workflow("staged_search", "running", "failed")
                m.record_workflow_terminal("staged_search", "failed", time.monotonic() - execution_started)
            return
        attempt_id = str(attempt["attempt_id"])
        filters = record["plan"]["filters"]
        if not await self.store.renew(tenant, operation_id, lease_token):
            return

        async def keep_alive() -> None:
            while True:
                await asyncio.sleep(20)
                if not await self.store.renew(tenant, operation_id, lease_token):
                    return

        renewal = asyncio.create_task(keep_alive())
        try:
            remaining_ms = int((record["execution_deadline_at"] - time.time()) * 1000)
            if remaining_ms <= 0:
                attempt.update(
                    state="failed",
                    finished_at=time.time(),
                    observed_engine_calls=0,
                    error={"code": "deadline_exceeded", "message": "deadline expired before dispatch"},
                    deadline_exceeded=True,
                )
                stage["state"] = "failed"
                record.update(state="failed", stop_reason="deadline_expired")
                record["objectives"]["unmet"].append({"objective": "deadline_ms", "reason": "deadline_expired"})
                if await self.store.save_owned(tenant, record, lease_token):
                    m.transition_workflow("staged_search", "running", "failed")
                    m.record_workflow_terminal("staged_search", "failed", time.monotonic() - execution_started)
                return
            response = await self.service.search(
                SearchRequest(
                    query=record["plan"]["query"],
                    engines=engines,
                    language=filters["language"],
                    time_range=filters["time_range"],
                    safesearch={"off": 0, "moderate": 1, "strict": 2}[filters["safesearch"]],
                    freshness=filters["freshness"],
                    client_identifier=f"mcp:{tenant}",
                    interactive_timeout_ms=max(1, remaining_ms),
                    generate_suggestions=False,
                    execution_isolation_key=f"{operation_id}:{attempt_id}",
                )
            )
        except Exception as exc:  # noqa: BLE001 - terminalize durable workflow
            attempt.update(
                state="failed", finished_at=time.time(), error={"code": "execution_failed", "message": str(exc)}
            )
            stage["state"] = "failed"
            record["budget"]["observed"] = None
            record.update(state="failed", stop_reason="execution_failed")
            if stage_index == 0 and len(record["stages"]) > 1:
                record["objectives"]["unmet"].append({"objective": "fallback", "reason": "initial_stage_failed"})
            if await self.store.save_owned(tenant, record, lease_token):
                m.transition_workflow("staged_search", "running", "failed")
                m.record_workflow_terminal("staged_search", "failed", time.monotonic() - execution_started)
            return
        finally:
            renewal.cancel()
            try:
                await renewal
            except asyncio.CancelledError:
                pass
        cursor = await self.snapshots.for_tenant(tenant).create(
            response.query,
            response.query_id,
            response.results,
            response.scope,
            ranking_explanation=response.ranking_explanation,
            derived_from=[artifact_ref("staged_search", operation_id)],
        )
        outcomes = [
            {
                "engine": o.engine,
                "status": o.status,
                "result_count": o.result_count,
                "latency_ms": o.latency_ms,
                "message": o.message,
            }
            for o in response.engine_outcomes
        ]
        observed = 0 if response.cached else response.dispatched_engine_count
        if record["budget"].get("observed") is not None:
            record["budget"]["observed"] = int(record["budget"]["observed"]) + observed
        attempt.update(
            state="done",
            finished_at=time.time(),
            observed_engine_calls=observed,
            cached=response.cached,
            query_id=response.query_id,
            cursor=cursor,
            result_count=len(response.results),
            engine_outcomes=outcomes,
            deadline_exceeded=response.deadline_exceeded,
        )
        stage["state"] = "done"
        if cursor:
            record["selected_result_attempt_id"] = attempt_id
        clean = (
            not response.results
            and not response.cached
            and not response.partial
            and not response.deadline_exceeded
            and len(outcomes) == len(engines)
            and all(item["status"] == "ok" for item in outcomes)
        )
        absolute_deadline_exceeded = time.time() >= record["execution_deadline_at"]
        if absolute_deadline_exceeded:
            attempt["deadline_exceeded"] = True
            record["objectives"]["unmet"].append({"objective": "deadline_ms", "reason": "deadline_expired"})
        if (
            absolute_deadline_exceeded
            or response.deadline_exceeded
            or response.partial
            or any(item["status"] != "ok" for item in outcomes)
        ):
            attempt["state"] = "failed"
            stage["state"] = "failed"
            record.update(
                state="failed",
                stop_reason="deadline_expired"
                if absolute_deadline_exceeded or response.deadline_exceeded
                else "execution_failed",
            )
            if stage_index == 0 and len(record["stages"]) > 1:
                record["objectives"]["unmet"].append({"objective": "fallback", "reason": "initial_stage_failed"})
        elif response.results:
            record.update(
                state="completed", stop_reason="initial_nonempty" if stage_index == 0 else "fallback_nonempty"
            )
            for pending in record["stages"][stage_index + 1 :]:
                pending.update(state="skipped", skipped_reason="initial_nonempty")
        elif clean and stage_index + 1 < len(record["stages"]):
            record.update(state="queued", stop_reason=None, next_stage=stage_index + 1)
        elif clean:
            record.update(state="completed", stop_reason="clean_empty")
        else:
            attempt["state"] = "failed"
            stage["state"] = "failed"
            record.update(state="failed", stop_reason="execution_failed")
            if stage_index == 0 and len(record["stages"]) > 1:
                record["objectives"]["unmet"].append({"objective": "fallback", "reason": "initial_stage_not_clean"})
        if not await self.store.save_owned(tenant, record, lease_token):
            return
        if record["state"] == "queued":
            m.transition_workflow("staged_search", "running", "queued")
            await self.enqueue(tenant, operation_id)
        else:
            outcome = "succeeded" if record["state"] == "completed" else "failed"
            m.transition_workflow("staged_search", "running", outcome)
            m.record_workflow_terminal("staged_search", outcome, time.monotonic() - execution_started)
            m.workflow_admitted_results.observe({"workflow": "staged_search"}, float(attempt["result_count"]))
