"""Deterministic Semantic Scholar 429 and upstream-cooldown contracts."""

from __future__ import annotations

import datetime as _dt
from email.utils import format_datetime

import httpx
import pytest

import slopsearx.ratelimit as ratelimit_module
from engines.semanticscholar import SemanticScholarAdapter, _parse_retry_after
from slopsearx.adapter import EngineStatus
from slopsearx.ratelimit import (
    DEFAULT_UPSTREAM_COOLDOWN_SECONDS,
    MAX_UPSTREAM_COOLDOWN_SECONDS,
    LocalTokenBucket,
    RateLimiter,
    RateLimitStrategy,
    ValkeySlidingWindow,
)
from slopsearx.service import AppContext, SearchRequest, SearchService


def test_retry_after_is_bounded_and_invalid_values_are_safe() -> None:
    now = 1_700_000_000.0
    future = format_datetime(_dt.datetime.fromtimestamp(now + 45, tz=_dt.UTC), usegmt=True)
    past = format_datetime(_dt.datetime.fromtimestamp(now - 45, tz=_dt.UTC), usegmt=True)

    assert _parse_retry_after("45", now=now) == 45.0
    assert _parse_retry_after(future, now=now) == pytest.approx(45.0, abs=1.0)
    assert _parse_retry_after("999999", now=now) == MAX_UPSTREAM_COOLDOWN_SECONDS
    assert _parse_retry_after("not-a-duration", now=now) == DEFAULT_UPSTREAM_COOLDOWN_SECONDS
    assert _parse_retry_after(past, now=now) == DEFAULT_UPSTREAM_COOLDOWN_SECONDS
    assert _parse_retry_after("-5", now=now) == DEFAULT_UPSTREAM_COOLDOWN_SECONDS


class _MemorySharedCooldown(RateLimitStrategy):
    """Small deterministic stand-in for a shared cooldown store."""

    def __init__(self) -> None:
        self.remaining: dict[str, float] = {}
        self.acquire_calls = 0

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        del engine, cost
        self.acquire_calls += 1
        return True

    async def set_upstream_cooldown(self, engine: str, seconds: float) -> None:
        self.remaining[engine] = seconds

    async def upstream_cooldown_remaining(self, engine: str) -> float:
        return self.remaining.get(engine, 0.0)


async def test_upstream_cooldown_is_shared_without_counting_local_strikes() -> None:
    shared = _MemorySharedCooldown()
    first = RateLimiter(shared)
    second = RateLimiter(shared)

    await first.set_upstream_cooldown("semanticscholar", 60)

    assert await second.upstream_cooldown_remaining("semanticscholar") == 60
    assert await second.acquire("semanticscholar") is False
    assert second.deactivated_engines == set()
    assert shared.acquire_calls == 0


async def test_valkey_publishes_bounded_cooldown_key() -> None:
    class _FakeValkey:
        def __init__(self) -> None:
            self.values: dict[str, int] = {}
            self.eval_calls: list[tuple[str, int]] = []

        async def eval(self, script: str, numkeys: int, key: str, seconds: str) -> int:
            """Model the cooldown Lua script as one atomic Valkey operation."""
            assert "redis.call('TTL'" in script
            assert numkeys == 1
            requested = int(float(seconds) + 0.999999)
            current = self.values.get(key, -2)
            if current < requested:
                self.values[key] = requested
                self.eval_calls.append((key, requested))
            return self.values.get(key, current)

        async def ttl(self, key: str) -> int:
            return self.values.get(key, -2)

    fake = _FakeValkey()
    strategy = ValkeySlidingWindow(valkey_url="redis://fixture")
    strategy._client = fake
    strategy._connected = True

    await strategy.set_upstream_cooldown("semanticscholar", 60)
    await strategy.set_upstream_cooldown("semanticscholar", 30)
    assert await strategy.upstream_cooldown_remaining("semanticscholar") == 60

    await strategy.set_upstream_cooldown("semanticscholar", 120)
    assert await strategy.upstream_cooldown_remaining("semanticscholar") == 120

    await strategy.set_upstream_cooldown("semanticscholar", 301)
    assert fake.eval_calls == [
        ("ratelimit:upstream-cooldown:semanticscholar", 60),
        ("ratelimit:upstream-cooldown:semanticscholar", 120),
        ("ratelimit:upstream-cooldown:semanticscholar", 300),
    ]
    assert await strategy.upstream_cooldown_remaining("semanticscholar") == 300


async def test_semantic_scholar_429_cools_down_and_recovers(monkeypatch: pytest.MonkeyPatch) -> None:
    clock = [1_700_000_000.0]
    monkeypatch.setattr(ratelimit_module.time, "monotonic", lambda: clock[0])
    calls = 0

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        if calls == 1:
            return httpx.Response(429, headers={"Retry-After": "2"})
        return httpx.Response(200, json={"data": []})

    adapter = SemanticScholarAdapter({"base_url": "https://fixture.invalid/search"})
    limiter = RateLimiter(LocalTokenBucket(max_rate=10, burst=10))
    adapter.rate_limiter = limiter
    adapter.set_http_transport(httpx.MockTransport(handler))

    first = await adapter.search("graph neural networks")
    assert first.status == EngineStatus.RATE_LIMITED
    assert first.results == []
    assert "cooldown 2s" in (first.error_message or "")
    assert 2.0 <= await limiter.upstream_cooldown_remaining(adapter.name) <= 2.0

    during = await adapter.search("graph neural networks")
    assert during.status == EngineStatus.RATE_LIMITED
    assert calls == 1

    clock[0] += 2.0
    recovered = await adapter.search("graph neural networks")
    assert recovered.status == EngineStatus.OK
    assert recovered.results == []
    assert calls == 2


@pytest.mark.parametrize(
    ("retry_after", "expected"),
    [
        ("45", 45.0),
        ("not-a-duration", DEFAULT_UPSTREAM_COOLDOWN_SECONDS),
        ("Wed, 01 Jan 2020 00:00:00 GMT", DEFAULT_UPSTREAM_COOLDOWN_SECONDS),
        ("999999", MAX_UPSTREAM_COOLDOWN_SECONDS),
    ],
)
async def test_429_header_variants_use_bounded_cooldown(retry_after: str, expected: float) -> None:
    adapter = SemanticScholarAdapter({"base_url": "https://fixture.invalid/search"})
    limiter = RateLimiter(LocalTokenBucket())
    adapter.rate_limiter = limiter
    adapter.set_http_transport(
        httpx.MockTransport(lambda request: httpx.Response(429, headers={"Retry-After": retry_after}))
    )

    result = await adapter.search("quota fixture")

    assert result.status == EngineStatus.RATE_LIMITED
    remaining = await limiter.upstream_cooldown_remaining(adapter.name)
    assert expected - 0.1 <= remaining <= expected


async def test_service_skips_provider_during_cooldown_and_health_stays_rate_limited() -> None:
    calls = 0

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        return httpx.Response(429, headers={"Retry-After": "60"})

    adapter = SemanticScholarAdapter({"base_url": "https://fixture.invalid/search"})
    limiter = RateLimiter(LocalTokenBucket())
    adapter.rate_limiter = limiter
    adapter.consecutive_errors = 2
    adapter.set_http_transport(httpx.MockTransport(handler))
    service = SearchService(
        AppContext(
            active_engines={adapter.name: adapter},
            rate_limiter=limiter,
            tier1_engines={adapter.name},
        )
    )
    request = SearchRequest(query="graph neural networks", engines=[adapter.name])

    first = await service.search(request)
    second = await service.search(request)

    assert first.results == []
    assert first.engine_outcomes[0].status == EngineStatus.RATE_LIMITED.value
    assert adapter.last_observed_status == EngineStatus.RATE_LIMITED.value
    assert adapter.consecutive_errors == 2
    assert second.engine_outcomes[0].status == EngineStatus.RATE_LIMITED.value
    assert "cooldown active" in (second.engine_outcomes[0].message or "")
    assert calls == 1
