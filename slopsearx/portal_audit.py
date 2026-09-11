"""Bounded, redacted audit events for protected browser workflows."""

from __future__ import annotations

import hashlib
import hmac
import time
from typing import Any

_ACTIONS = frozenset(
    {
        "login",
        "tenant_switch",
        "logout",
        "workflow.read",
        "workflow.research.cancel",
        "workflow.research.retry",
        "workflow.saved.control",
        "workflow.saved.ack",
        "workflow.compose",
    }
)
_OUTCOMES = frozenset({"allowed", "denied", "succeeded", "conflict", "failed"})
_REASONS = frozenset(
    {
        "ok",
        "authentication_required",
        "authorization_denied",
        "csrf_denied",
        "object_unavailable",
        "revision_conflict",
        "duplicate",
        "store_unavailable",
        "callback_rejected",
    }
)


class PortalAuditLogger:
    """Write closed-vocabulary pseudonymous events to a bounded Valkey stream."""

    def __init__(self, store: Any, *, pseudonym_key: bytes) -> None:
        if len(pseudonym_key) < 32:
            raise ValueError("portal audit key must contain at least 32 bytes")
        self._store = store
        self._key = pseudonym_key

    def _hash(self, value: str | None) -> str:
        return hmac.new(self._key, (value or "none").encode(), hashlib.sha256).hexdigest()[:24]

    async def record(
        self,
        *,
        request_id: str,
        action: str,
        outcome: str,
        reason: str,
        principal_id: str | None = None,
        tenant_id: str | None = None,
        session_generation: int | None = None,
        object_kind: str | None = None,
        object_id: str | None = None,
    ) -> None:
        if action not in _ACTIONS or outcome not in _OUTCOMES or reason not in _REASONS:
            raise ValueError("portal audit vocabulary is closed")
        client = getattr(self._store, "_client", None)
        if not self._store.is_connected:
            return
        fields = {
            "timestamp": str(time.time()),
            "request_id": request_id[:128],
            "action": action,
            "outcome": outcome,
            "reason": reason,
            "principal": self._hash(principal_id),
            "tenant": self._hash(tenant_id),
            "session_generation": self._hash(str(session_generation) if session_generation is not None else None),
            "object_kind": object_kind
            if object_kind in {"research", "dependency_dossier", "saved_search", "staged_search"}
            else "none",
            "object": self._hash(object_id),
        }
        key = "portal:audit:v1:" + time.strftime("%Y-%m-%d", time.gmtime())
        if client is None:
            if not self._store.is_connected:
                return
            current = await self._store.get(key) or {}
            entries = list(current.get("entries", []))[-9_999:]
            entries.append(fields)
            await self._store.set(key, {"entries": entries}, 7_776_000)
            return
        pipe = client.pipeline()
        pipe.execute_command("XADD", key, "MAXLEN", "~", "10000", "*", *[v for pair in fields.items() for v in pair])
        pipe.expire(key, 7_776_000)
        await pipe.execute()
