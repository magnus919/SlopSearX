"""Interactive budgets bound waiting without degrading normal search coverage."""

from __future__ import annotations

import asyncio
import time

import pytest

from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.mcp.harness import InMemoryStore
from slopsearx.service import AppContext, SearchRequest, SearchService, build_response_meta


class TimedEngine(EngineAdapter):
    categories = ["general"]

    def __init__(self, name, delay):
        super().__init__()
        self.name = name
        self.delay = delay
        self.calls = 0
        self.cancelled = False

    async def search(self, query, params=None):
        self.calls += 1
        try:
            await asyncio.sleep(self.delay)
        except asyncio.CancelledError:
            self.cancelled = True
            raise
        return AdapterResponse(
            status=EngineStatus.OK,
            results=[
                SearchResult(
                    url=f"https://{self.name}.example", title=self.name, content="test result", engine=self.name
                )
            ],
        )


@pytest.mark.asyncio
async def test_partial_budget_does_not_poison_cache_health_or_full_search():
    fast, slow = TimedEngine("fast", 0), TimedEngine("slow", 0.15)
    ctx = AppContext(active_engines={e.name: e for e in (fast, slow)}, cache=InMemoryStore())
    service = SearchService(ctx)
    start = time.monotonic()
    partial = await service.search(SearchRequest("probe", interactive_timeout_ms=20))
    bounded_elapsed = time.monotonic() - start
    assert [r.title for r in partial.results] == ["fast"]
    assert partial.partial and partial.deadline_exceeded
    assert slow.cancelled and slow.consecutive_errors == 0
    assert slow.last_observed_at is None
    assert partial.engine_outcomes[1].status == "unavailable"
    assert build_response_meta(partial)["deadline_exceeded"] is True
    assert not ctx.cache._data

    start = time.monotonic()
    full = await service.search(SearchRequest("probe"))
    full_elapsed = time.monotonic() - start
    assert len(full.results) == 2
    assert not full.deadline_exceeded and not full.partial
    assert bounded_elapsed < full_elapsed
    print(f"interactive={bounded_elapsed:.4f}s (1 result); full={full_elapsed:.4f}s (2 results)")
    assert (await service.search(SearchRequest("probe"))).cached
    assert not (await service.search(SearchRequest("probe", interactive_timeout_ms=20))).cached


@pytest.mark.asyncio
async def test_different_budgets_do_not_coalesce():
    engine = TimedEngine("slow", 0.08)
    service = SearchService(AppContext(active_engines={engine.name: engine}))
    short, full = await asyncio.gather(
        service.search(SearchRequest("probe", interactive_timeout_ms=10)),
        service.search(SearchRequest("probe")),
    )
    assert engine.calls == 2
    assert short.deadline_exceeded and not short.results
    assert full.results and not full.deadline_exceeded
    assert not service._inflight


@pytest.mark.asyncio
async def test_waiting_engine_is_not_started_or_marked_failed():
    engines = [TimedEngine("first", 0.15), TimedEngine("queued", 0.15)]
    service = SearchService(
        AppContext(active_engines={e.name: e for e in engines}, engine_semaphore=asyncio.Semaphore(1))
    )
    response = await service.search(SearchRequest("probe", interactive_timeout_ms=10))
    assert response.deadline_exceeded and response.all_unresponsive
    assert engines[1].calls == 0
    assert all(e.consecutive_errors == 0 for e in engines)


@pytest.mark.asyncio
async def test_suggestions_cannot_extend_budget():
    class SlowSuggestions:
        cancelled = False

        async def fetch(self, query):
            try:
                await asyncio.sleep(10)
            except asyncio.CancelledError:
                self.cancelled = True
                raise
            return ["late"]

    suggestions = SlowSuggestions()
    engine = TimedEngine("fast", 0)
    service = SearchService(AppContext(active_engines={engine.name: engine}, suggestion_service=suggestions))
    response = await asyncio.wait_for(service.search(SearchRequest("probe", interactive_timeout_ms=10)), timeout=0.5)
    assert response.results and response.deadline_exceeded
    assert suggestions.cancelled and response.suggestions == []


@pytest.mark.asyncio
@pytest.mark.parametrize("budget", [0, -1, 30001, True, 1.5, "10"])
async def test_invalid_budgets_do_not_dispatch(budget):
    engine = TimedEngine("fast", 0)
    service = SearchService(AppContext(active_engines={engine.name: engine}))
    with pytest.raises(ValueError, match="interactive_timeout_ms"):
        await service.search(SearchRequest("probe", interactive_timeout_ms=budget))
    assert engine.calls == 0


@pytest.mark.asyncio
async def test_upstream_timeout_is_not_caller_deadline():
    fast, slow = TimedEngine("fast", 0), TimedEngine("slow", 0.1)
    slow.config["timeout_ms"] = 5
    service = SearchService(AppContext(active_engines={e.name: e for e in (fast, slow)}))
    response = await service.search(SearchRequest("probe", interactive_timeout_ms=100))
    assert response.partial and not response.deadline_exceeded
    assert response.engine_outcomes[1].status == "timeout"
    assert slow.consecutive_errors == 1
