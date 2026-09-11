from __future__ import annotations

import asyncio
import time

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_auth import PortalAuthContext
from slopsearx.research import ResearchJob
from slopsearx.research_store import MAX_RECENT_JOBS, ResearchJobStore
from slopsearx.saved_models import SavedDefinition
from slopsearx.saved_store import MAX_RECENT_DEFINITIONS, SavedSearchStore
from slopsearx.staged import StagedSearchStore
from slopsearx.workflow_console import WorkflowConsoleService


def saved(search_id: str, created_at: float, expires_at: float = 100_000.0) -> SavedDefinition:
    return SavedDefinition(
        search_id=search_id,
        tenant="tenant",
        query="query",
        engines=["wikipedia"],
        interval_seconds=60,
        retention_seconds=300,
        max_results=10,
        max_reports=5,
        created_at=created_at,
        expires_at=expires_at,
        next_due=created_at + 60,
    )


async def test_research_recent_index_is_concurrency_bounded_and_fills_around_stale_entries() -> None:
    shared = InMemoryStore()
    store = ResearchJobStore(shared).for_tenant("tenant")
    base = time.time()
    jobs = [
        ResearchJob(job_id=f"job-{index:03d}", question="q", strategy="broad", created_at=base + index, tenant="tenant")
        for index in range(240)
    ]
    await asyncio.gather(*(store.save(job) for job in jobs))
    recent = await shared.get(store._recent_key())
    assert recent is not None and len(recent) == MAX_RECENT_JOBS

    newest = sorted(recent, key=lambda key: (recent[key], key), reverse=True)[:15]
    for job_id in newest:
        await shared.delete(store._key(job_id))
    page = await store.list_recent(limit=50)
    assert len(page) == 50
    assert not {item.job_id for item in page} & set(newest)


async def test_saved_recent_index_pages_past_fifty_and_retains_definition_horizon() -> None:
    shared = InMemoryStore()
    store = SavedSearchStore(shared).for_tenant("tenant")
    base = time.time()
    definitions = [saved(f"saved-{index:03d}", base + index, base + 100_000) for index in range(90)]
    for definition in definitions:
        await shared.set(store._definition_key(definition.search_id), store._stored_payload(definition), 100_000)
    await asyncio.gather(*(store._record_recent(definition) for definition in definitions))
    recent = await shared.get(store._recent_index())
    assert recent is not None and len(recent) <= MAX_RECENT_DEFINITIONS
    assert max(shared.set_ttls) > 90_000

    first = await store.list_recent(limit=50, now=base)
    marker = (first[-1].created_at, first[-1].search_id)
    second = await store.list_recent(before=marker, limit=50, now=base)
    assert len(first) == 50
    assert len(second) == 40
    assert {item.search_id for item in first}.isdisjoint(item.search_id for item in second)

    newest = [item.search_id for item in first[:10]]
    for search_id in newest:
        await shared.delete(store._definition_key(search_id))
    filled = await store.list_recent(limit=50, now=base)
    assert len(filled) == 50
    assert not {item.search_id for item in filled} & set(newest)


async def test_saved_recent_fallback_does_not_shorten_existing_index_horizon() -> None:
    shared = InMemoryStore()
    store = SavedSearchStore(shared).for_tenant("tenant")
    base = time.time()
    long_lived = saved("long-lived", base, base + 100_000)
    short_lived = saved("short-lived", base + 1, base + 1_000)
    for definition in (long_lived, short_lived):
        await shared.set(store._definition_key(definition.search_id), store._stored_payload(definition), 100_000)
        await store._record_recent(definition)
    assert shared.set_ttls[-1] > 90_000


async def test_console_real_store_fills_past_fifty_policy_revoked_definitions() -> None:
    shared = InMemoryStore()
    saved_store = SavedSearchStore(shared)
    tenant_store = saved_store.for_tenant("tenant")
    base = time.time()
    definitions = [saved(f"revoked-{index:03d}", base + 100 - index, base + 100_000) for index in range(60)]
    for definition in definitions:
        definition.engines = ["secret"]
    allowed = saved("allowed", base, base + 100_000)
    for definition in [*definitions, allowed]:
        await shared.set(
            tenant_store._definition_key(definition.search_id),
            tenant_store._stored_payload(definition),
            100_000,
        )
        await tenant_store._record_recent(definition)
    policy = MCPPolicy(
        enabled_tools={"saved_searches": True},
        sensitive_engines={"secret"},
    )
    console = WorkflowConsoleService(
        policy=policy,
        jobs=ResearchJobStore(shared),
        research_runner=object(),  # type: ignore[arg-type]
        staged=StagedSearchStore(shared),
        saved=saved_store,
        cursor_key=b"c" * 32,
    )
    context = PortalAuthContext("principal", "tenant", "Tenant", frozenset({"workflow.read"}), 1, 1, 1, "csrf")
    page = await console.list(context, None, limit=1)
    assert [(item["kind"], item["id"]) for item in page.items] == [("saved_search", "allowed")]
