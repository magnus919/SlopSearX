"""Bounded scheduled execution for saved searches."""

from __future__ import annotations

import asyncio
import contextlib
import time
from collections.abc import Callable
from typing import Any

from slopsearx import metrics as m
from slopsearx.capabilities import MCPPolicy
from slopsearx.saved_events import event_for_report
from slopsearx.saved_models import SavedDefinition, compare_observations, observe, stable_id
from slopsearx.saved_store import OutboxCapacityError, SavedSearchStore
from slopsearx.service import SearchRequest, SearchService


class SavedSearchRunner:
    """Execute due first-page observations; Valkey owns coordination."""

    def __init__(
        self,
        service: SearchService,
        store: SavedSearchStore,
        policy: MCPPolicy,
        *,
        policy_check: Callable[[SavedDefinition], str | None],
        policy_fingerprint: Callable[[SavedDefinition], str],
        clock: Callable[[], float] = time.time,
        poll_interval: float = 1.0,
    ) -> None:
        self._service = service
        self._store = store
        self._policy = policy
        self._policy_check = policy_check
        self._policy_fingerprint = policy_fingerprint
        self._clock = clock
        self._poll_interval = poll_interval
        self._semaphore = asyncio.Semaphore(policy.saved_max_concurrent_runs)

    async def run_one(self, tenant: str, search_id: str, *, now: float | None = None) -> dict[str, Any] | None:
        """Claim and execute one due definition; return its committed report."""
        instant = self._clock() if now is None else now
        store = self._store.for_tenant(tenant)
        before = await store.load(search_id, now=instant)
        definition = await store.claim(search_id, now=instant)
        if definition is None:
            return None
        recovered = before is not None and bool(before.lease_token) and before.lease_expires_at <= instant
        if recovered:
            m.record_workflow_recovery("saved_search")
            # Clear a possibly stale local observation before accounting for
            # the replacement lease; this also rebuilds an empty post-restart
            # gauge to exactly one running attempt.
            m.transition_workflow("saved_search", "running", "interrupted")
        m.transition_workflow("saved_search", "interrupted" if recovered else None, "running")
        slot = max(0, int((instant - definition.created_at) // definition.interval_seconds))
        scheduled_at = definition.next_due
        missed_slots = max(0, slot - definition.last_slot - 1) if definition.last_slot >= 0 else max(0, slot - 1)
        run_id = f"run-{stable_id(definition.search_id, definition.revision, definition.next_due)[:24]}"
        started_at = self._clock()
        m.workflow_queue_wait.observe({"workflow": "saved_search"}, max(0.0, started_at - scheduled_at))
        rejection = self._policy_check(definition)
        current_fingerprint = self._policy_fingerprint(definition)
        if rejection or current_fingerprint != definition.policy_fingerprint:
            # Do not retain source evidence under changed/revoked policy.
            await store.release(definition)
            m.record_workflow_rejection("saved_search", "policy")
            m.transition_workflow("saved_search", "running", "failed")
            m.record_workflow_terminal("saved_search", "failed", max(0.0, self._clock() - started_at))
            return None
        lease_lost = asyncio.Event()

        async def keep_alive() -> None:
            while True:
                await asyncio.sleep(20)
                if not await store.renew(definition):
                    lease_lost.set()
                    return

        renewal = asyncio.create_task(keep_alive())
        try:
            try:
                async with asyncio.timeout(self._policy.saved_dispatch_timeout_seconds):
                    response = await self._service.search(
                        SearchRequest(
                            query=definition.query,
                            engines=list(definition.engines),
                            page=1,
                            max_results=None,
                            include={"results", "engine_status"},
                            freshness="prefer_fresh",
                            client_identifier=f"mcp-saved:{tenant}",
                        )
                    )
                observation = observe(definition, response, self._clock())
            except Exception as exc:  # noqa: BLE001 - failure becomes an incomparable observation
                observation = {
                    "window": definition.window(),
                    "observed_at": self._clock(),
                    "query_id": "",
                    "cached": False,
                    "coverage": {},
                    "discovered_count": 0,
                    "records": {},
                    "incomparable_reasons": ["dispatch_failed"],
                    "operational_error": type(exc).__name__,
                }
        finally:
            renewal.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await renewal
        if lease_lost.is_set():
            m.record_workflow_rejection("saved_search", "lease_lost")
            m.transition_workflow("saved_search", "running", "failed")
            m.record_workflow_terminal("saved_search", "failed", max(0.0, self._clock() - started_at))
            return None
        finished_at = self._clock()
        report, baseline = compare_observations(
            definition,
            observation,
            run_id=run_id,
            scheduled_at=scheduled_at,
            started_at=started_at,
            finished_at=finished_at,
            missed_slots=missed_slots,
        )
        report["slot"] = slot
        # Revocation/config change while upstream I/O was in flight blocks commit.
        rejection = self._policy_check(definition)
        current_fingerprint = self._policy_fingerprint(definition)
        if rejection or current_fingerprint != definition.policy_fingerprint:
            await store.release(definition)
            m.record_workflow_rejection("saved_search", "policy")
            m.transition_workflow("saved_search", "running", "failed")
            m.record_workflow_terminal("saved_search", "failed", max(0.0, self._clock() - started_at))
            return None
        next_due = definition.created_at + (slot + 1) * definition.interval_seconds
        outbox_event = (
            event_for_report(definition, report) if self._policy.tool_enabled("saved_search_events") else None
        )
        try:
            committed = await store.commit(
                definition,
                report,
                baseline,
                now=finished_at,
                next_due=next_due,
                policy_fingerprint=current_fingerprint,
                event=outbox_event,
            )
        except OutboxCapacityError:
            await store.release(definition)
            m.record_workflow_rejection("saved_search", "capacity")
            m.record_saved_event_capacity("stream")
            m.transition_workflow("saved_search", "running", "failed")
            m.record_workflow_terminal("saved_search", "failed", max(0.0, self._clock() - started_at))
            return None
        if committed:
            outcome = "failed" if report["status"] == "incomparable" else "succeeded"
            duration = max(0.0, finished_at - started_at)
            m.transition_workflow("saved_search", "running", outcome)
            m.record_workflow_terminal("saved_search", outcome, duration)
            m.workflow_report_generation.observe({"workflow": "saved_search"}, duration)
            m.workflow_admitted_results.observe(
                {"workflow": "saved_search"}, float(report.get("observation", {}).get("discovered_count", 0))
            )
            if outbox_event is not None:
                m.record_saved_event_publication(str(outbox_event["event_type"]))
            return report
        m.record_workflow_rejection("saved_search", "lease_lost")
        m.transition_workflow("saved_search", "running", "failed")
        m.record_workflow_terminal("saved_search", "failed", max(0.0, self._clock() - started_at))
        return None

    async def run_due(self, tenant: str, *, now: float | None = None) -> list[dict[str, Any]]:
        """Execute at most one bounded due-index batch for a tenant."""
        instant = self._clock() if now is None else now
        store = self._store.for_tenant(tenant)
        await store.repair_indexes(now=instant, limit=128)
        ids = await store.due_ids(instant, limit=128)

        async def run(search_id: str) -> dict[str, Any] | None:
            async with self._semaphore:
                return await self.run_one(tenant, search_id, now=instant)

        values = await asyncio.gather(*(run(item) for item in ids))
        return [item for item in values if item is not None]

    async def run_forever(self) -> None:
        """Poll bounded tenant/due batches; errors never terminate the worker."""
        while True:
            try:
                if self._policy.tool_enabled("saved_search_events"):
                    try:
                        expired = await self._store.publish_expired_events(now=self._clock(), limit=128)
                    except OutboxCapacityError:
                        m.record_saved_event_capacity("stream")
                        raise
                    m.record_saved_event_publication("definition_expired", expired)
                for tenant in await self._store.scan_tenants(limit=128):
                    await self.run_due(tenant)
            except asyncio.CancelledError:
                raise
            except Exception:
                pass
            await asyncio.sleep(self._poll_interval)

    async def close(self) -> None:
        """Compatibility seam for lifespan shutdown."""
        with contextlib.suppress(Exception):
            return None
