from __future__ import annotations

import json

import pytest

from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_audit import PortalAuditLogger


async def test_audit_is_bounded_pseudonymous_and_rejects_open_vocabulary() -> None:
    store = InMemoryStore()
    audit = PortalAuditLogger(store, pseudonym_key=b"a" * 32)
    await audit.record(
        request_id="request",
        action="workflow.saved.ack",
        outcome="denied",
        reason="csrf_denied",
        principal_id="raw-principal",
        tenant_id="raw-tenant",
        session_generation=4,
        object_kind="dependency_dossier",
        object_id="raw-object",
    )
    encoded = json.dumps(store._data)
    assert "raw-principal" not in encoded
    assert "raw-tenant" not in encoded
    assert "raw-object" not in encoded
    entry = next(iter(store._data.values()))["entries"][0]
    assert entry["object_kind"] == "dependency_dossier"
    assert "csrf" not in encoded.lower() or "csrf_denied" in encoded
    with pytest.raises(ValueError):
        await audit.record(request_id="x", action="arbitrary", outcome="allowed", reason="ok")
