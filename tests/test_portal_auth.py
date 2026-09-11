from __future__ import annotations

import asyncio

from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_auth import (
    PortalMembership,
    PortalPrincipal,
    PortalSessionStore,
    StaticMembershipResolver,
    safe_return_to,
)


def principal(*, revision: int = 1, membership_revision: int = 1) -> PortalPrincipal:
    return PortalPrincipal(
        principal_id="principal-1",
        issuer="https://id.example",
        subject="subject-1",
        revision=revision,
        memberships=(
            PortalMembership(
                "tenant-a",
                "Acme Research",
                frozenset({"workflow.read", "workflow.research.cancel"}),
                revision=membership_revision,
            ),
        ),
    )


async def test_session_is_opaque_tenant_bound_and_revision_checked() -> None:
    now = [1000.0]
    current = principal()
    resolver = StaticMembershipResolver((current,))
    store = InMemoryStore()
    sessions = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"x" * 32,
        clock=lambda: now[0],
        token_source=lambda _size: "opaque-value-with-enough-entropy-for-tests",
    )
    handle, _record = await sessions.create(current, "tenant-a")

    assert current.principal_id not in handle
    assert all(current.principal_id not in key for key in store._data)
    resolved = await sessions.resolve(handle, frozenset({"workflow.read"}))
    assert resolved.context is not None
    assert resolved.context.tenant_id == "tenant-a"
    assert resolved.context.grants == frozenset({"workflow.read"})

    resolver._by_id[current.principal_id] = principal(revision=2)
    revoked = await sessions.resolve(handle, frozenset({"workflow.read"}))
    assert revoked.revoked is True
    assert await sessions.resolve(handle, frozenset({"workflow.read"})) == type(revoked)()


async def test_session_expiry_and_tenant_rotation_invalidate_old_handle() -> None:
    now = [1000.0]
    item = PortalPrincipal(
        principal_id="p",
        issuer="https://id.example",
        subject="s",
        memberships=(
            PortalMembership("a", "A", frozenset({"workflow.read"})),
            PortalMembership("b", "B", frozenset({"workflow.read"})),
        ),
    )
    resolver = StaticMembershipResolver((item,))
    store = InMemoryStore()
    tokens = iter(("first-handle", "csrf-2", "second-handle", "csrf-3"))
    sessions = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"y" * 32,
        clock=lambda: now[0],
        token_source=lambda _size: next(tokens),
        idle_seconds=60,
        absolute_seconds=120,
    )
    old, pending = await sessions.create(item, None)
    new, _active = await sessions.rotate(old, pending, "b")
    assert (await sessions.resolve(old, frozenset({"workflow.read"}))).context is None
    assert (await sessions.resolve(new, frozenset({"workflow.read"}))).context is not None
    now[0] = 1121
    assert (await sessions.resolve(new, frozenset({"workflow.read"}))).expired is True


def test_return_paths_are_restricted_to_workflow_namespace() -> None:
    assert safe_return_to("/workflows/research/job") == "/workflows/research/job"
    assert safe_return_to("https://evil.example") == "/workflows"
    assert safe_return_to("//evil.example/workflows") == "/workflows"
    assert safe_return_to("/workflows\\evil") == "/workflows"
    assert safe_return_to("/search") == "/workflows"


async def test_parallel_sliding_refresh_does_not_false_revoke() -> None:
    now = [1000.0]
    item = principal()
    resolver = StaticMembershipResolver((item,))
    store = InMemoryStore()
    sessions = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"r" * 32,
        clock=lambda: now[0],
        token_source=lambda _size: "parallel-refresh-handle-with-enough-entropy",
    )
    handle, _ = await sessions.create(item, "tenant-a")
    now[0] += 10
    first, second = await asyncio.gather(
        sessions.resolve(handle, frozenset({"workflow.read"})),
        sessions.resolve(handle, frozenset({"workflow.read"})),
    )
    assert first.context is not None
    assert second.context is not None
    assert not first.revoked and not second.revoked


async def test_previous_key_is_verification_only_and_bounded() -> None:
    now = [1000.0]
    item = principal()
    resolver = StaticMembershipResolver((item,))
    store = InMemoryStore()
    old = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"o" * 32,
        clock=lambda: now[0],
        token_source=lambda _size: "old-key-session-handle-with-enough-entropy",
    )
    handle, _ = await old.create(item, "tenant-a")
    rotated = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"n" * 32,
        previous_hmac_key=b"o" * 32,
        previous_key_drain_seconds=60,
        clock=lambda: now[0],
        token_source=lambda _size: "new-key-session-handle-with-enough-entropy",
    )
    assert (await rotated.resolve(handle, frozenset({"workflow.read"}))).context is not None
    new_handle, _ = await rotated.create(item, "tenant-a")
    assert rotated._key(new_handle) in store._data
    now[0] = 1061
    assert (await rotated.resolve(handle, frozenset({"workflow.read"}))).context is None


async def test_tenant_rotation_cannot_extend_absolute_session_lifetime() -> None:
    now = [1000.0]
    item = PortalPrincipal(
        principal_id="p",
        issuer="https://id.example",
        subject="s",
        memberships=(
            PortalMembership("a", "A", frozenset({"workflow.read"})),
            PortalMembership("b", "B", frozenset({"workflow.read"})),
        ),
    )
    resolver = StaticMembershipResolver((item,))
    store = InMemoryStore()
    counter = iter(range(20))
    sessions = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"t" * 32,
        clock=lambda: now[0],
        idle_seconds=60,
        absolute_seconds=120,
        token_source=lambda _size: f"rotation-token-{next(counter):03d}-abcdefghijklmnopqrstuvwxyz",
    )
    handle, record = await sessions.create(item, None)
    original_absolute = record.absolute_expires_at
    for tenant in ("a", "b", "a"):
        now[0] += 30
        handle, record = await sessions.rotate(handle, record, tenant)
        assert record.authenticated_at == 1000.0
        assert record.created_at == 1000.0
        assert record.absolute_expires_at == original_absolute
    now[0] = original_absolute
    assert (await sessions.resolve(handle, frozenset({"workflow.read"}))).expired is True


async def test_login_replacement_collision_never_deletes_presented_session() -> None:
    item = principal()
    resolver = StaticMembershipResolver((item,))
    store = InMemoryStore()
    tokens = iter(("csrf-one", "old-handle", "csrf-two", "old-handle", "old-handle", "old-handle"))
    sessions = PortalSessionStore(
        store,
        resolver,
        hmac_key=b"u" * 32,
        token_source=lambda _size: next(tokens),
    )
    old_handle, _ = await sessions.create(item, "tenant-a")
    try:
        await sessions.replace(old_handle, item, "tenant-a")
    except RuntimeError:
        pass
    else:
        raise AssertionError("three colliding handles must fail")
    assert (await sessions.resolve(old_handle, frozenset({"workflow.read"}))).context is not None
