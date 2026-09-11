from __future__ import annotations

import json
from pathlib import Path

import pytest

from slopsearx.capabilities import CapabilityCatalog, MCPPolicy
from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_auth import (
    PortalAuthContext,
    PortalMembership,
    PortalPrincipal,
    StaticMembershipResolver,
)
from slopsearx.saved_models import SavedDefinition
from slopsearx.service import AppContext
from slopsearx.workflow_portal import _load_directory, build_workflow_runtime_from_env


def valid_payload() -> dict[str, object]:
    return {
        "principals": [
            {
                "principal_id": "principal",
                "issuer": "https://id.example",
                "subject": "subject",
                "revision": 1,
                "enabled": True,
                "memberships": [
                    {
                        "tenant_id": "tenant",
                        "display_name": "Tenant",
                        "grants": ["workflow.read"],
                        "revision": 1,
                        "enabled": True,
                    }
                ],
            }
        ]
    }


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("principals", 0, "enabled"), "false"),
        (("principals", 0, "revision"), "1"),
        (("principals", 0, "memberships", 0, "grants"), "workflow.read"),
        (("principals", 0, "memberships", 0, "tenant_id"), ""),
    ],
)
def test_membership_directory_rejects_coercible_or_unstable_values(
    tmp_path: Path, path: tuple[object, ...], value: object
) -> None:
    payload = valid_payload()
    target: object = payload
    for part in path[:-1]:
        target = target[part]  # type: ignore[index]
    target[path[-1]] = value  # type: ignore[index]
    location = tmp_path / "directory.json"
    location.write_text(json.dumps(payload))
    with pytest.raises(ValueError):
        _load_directory(str(location), "https://id.example")


def test_membership_resolver_rejects_duplicate_principal_subject_and_tenant() -> None:
    membership = PortalMembership("tenant", "Tenant", frozenset({"workflow.read"}))
    first = PortalPrincipal("one", "https://id.example", "subject", (membership,))
    with pytest.raises(ValueError):
        StaticMembershipResolver((first, PortalPrincipal("two", "https://id.example", "subject", (membership,))))
    with pytest.raises(ValueError):
        StaticMembershipResolver((PortalPrincipal("one", "https://id.example", "other", (membership,)), first))
    with pytest.raises(ValueError):
        StaticMembershipResolver((PortalPrincipal("one", "https://id.example", "subject", (membership, membership)),))


async def test_production_runtime_wires_authoritative_saved_runner_and_receipt_store(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    location = tmp_path / "directory.json"
    location.write_text(json.dumps(valid_payload()))
    settings = {
        "SLOPSEARX_WORKFLOW_PORTAL_ENABLED": "1",
        "SLOPSEARX_PORTAL_EXTERNAL_ORIGIN": "https://portal.example",
        "SLOPSEARX_PORTAL_OIDC_ISSUER": "https://id.example",
        "SLOPSEARX_PORTAL_OIDC_CLIENT_ID": "client",
        "SLOPSEARX_PORTAL_OIDC_CLIENT_SECRET": "client-secret",
        "SLOPSEARX_PORTAL_SESSION_KEY": "s" * 32,
        "SLOPSEARX_PORTAL_MEMBERSHIPS_FILE": str(location),
        "SLOPSEARX_WORKFLOW_PORTAL_ACTIONS": "workflow.saved.control",
    }
    for name, value in settings.items():
        monkeypatch.setenv(name, value)
    store = InMemoryStore()
    catalog = CapabilityCatalog(adapters={})
    ctx = AppContext(active_engines={}, cache=store, catalog=catalog)  # type: ignore[arg-type]
    policy = MCPPolicy(enabled_tools={"saved_searches": True}, sensitive_engines=set())
    runtime = build_workflow_runtime_from_env(ctx, policy)
    assert runtime is not None
    state = runtime.workflows.composition_state
    assert state is not None and state.saved_runner is not None and state.receipt_store is not None
    now = 1000.0
    definition = SavedDefinition(
        "saved",
        "tenant",
        "q",
        ["wikipedia"],
        60,
        300,
        10,
        5,
        now,
        10_000_000_000.0,
        now + 60,
    )
    assert await state.saved_store.for_tenant("tenant").create(definition, 20, now=now) == "created"  # type: ignore[union-attr]
    portal_context = PortalAuthContext(
        "principal",
        "tenant",
        "Tenant",
        frozenset({"workflow.saved.control"}),
        1,
        1,
        1,
        "csrf",
    )
    revision = await runtime.workflows.set_saved_paused(portal_context, "saved", expected_revision=1, paused=True)
    updated = await state.saved_store.for_tenant("tenant").load("saved")  # type: ignore[union-attr]
    assert revision == 2 and updated is not None and updated.paused
    assert updated.created_at > now and updated.policy_fingerprint
