"""Isolated F10 source observations. Native gates/runtime never execute this file."""

import argparse
import asyncio
import copy
import hashlib
import json
import math
import os
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

BASELINE = "9ab1c7a22de933762c42469145c1f50365220de2"
SOURCES = [
    "slopsearx/ratelimit.py",
    "slopsearx/adapter.py",
    "slopsearx/capabilities.py",
    "slopsearx/service.py",
    "engines/semanticscholar.py",
    "tests/test_ratelimit.py",
    "tests/test_observed_health.py",
    "tests/test_interactive_deadline.py",
    "tests/test_semanticscholar_cooldown.py",
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 14, 7):
        raise SystemExit("Python3.14.7 required")
    root = args.source_root.resolve()
    sources = []
    for name in SOURCES:
        raw = subprocess.check_output(["git", "show", f"{BASELINE}:{name}"], cwd=root)
        if (root / name).read_bytes() != raw:
            raise SystemExit(f"source drift: {name}")
        sources.append(
            {
                "path": name,
                "git_blob": subprocess.check_output(
                    ["git", "rev-parse", f"{BASELINE}:{name}"], cwd=root, text=True
                ).strip(),
                "sha256": hashlib.sha256(raw).hexdigest(),
            }
        )
    sys.path.insert(0, str(root))
    import engines  # noqa: F401
    from engines.semanticscholar import _parse_retry_after
    from slopsearx import adapter, capabilities, ratelimit
    from slopsearx.service import AppContext, SearchRequest, SearchService

    for name in ["ENGINE_CIRCUIT_THRESHOLD", "ENGINE_CIRCUIT_TIMEOUT", "OBSERVED_HEALTH_STALE_SECONDS"]:
        os.environ.pop(name, None)
    clock = [100.0]
    fake_time = SimpleNamespace(time=lambda: clock[0], monotonic=lambda: clock[0])
    adapter.time = capabilities.time = ratelimit.time = fake_time

    class FakeStrategy(ratelimit.RateLimitStrategy):
        def __init__(self, allowed):
            self.allowed = iter(allowed)
            self.calls = 0
            self.shared = 0.0

        async def acquire(self, engine, cost=1):
            self.calls += 1
            return next(self.allowed)

        async def upstream_cooldown_remaining(self, engine):
            return self.shared

        async def set_upstream_cooldown(self, engine, seconds):
            self.shared = seconds

    class FakeEngine(adapter.EngineAdapter):
        name = "fakeeng"
        categories = ["general"]

        def __init__(self):
            super().__init__()
            self.status = adapter.EngineStatus.OK
            self.count = 0
            self.latency = 0.0
            self.synthetic = False
            self.calls = 0
            self.hang = False

        async def search(self, query, params=None):
            self.calls += 1
            if self.hang:
                await asyncio.sleep(60)
            return adapter.AdapterResponse(
                status=self.status,
                results=[
                    adapter.SearchResult(
                        url="https://public.fixture/", title="fixture", content="fixture", engine=self.name
                    )
                    for _ in range(self.count)
                ],
                latency_ms=self.latency,
                synthetic=self.synthetic,
            )

    async def capture():
        local = []
        for rate, burst, ops in [
            (1, 2, [[0, "a", 1], [0, "a", 1], [0, "a", 1], [0.5, "a", 1], [1, "a", 1], [1, "b", 2], [10, "a", 2]]),
            (0, 0, [[0, "a", 0], [1, "a", 1]]),
            (0.5, 1.5, [[0, "a", 1], [1, "a", 1], [2, "a", 1]]),
        ]:
            bucket = ratelimit.LocalTokenBucket(rate, burst)
            outcomes = []
            for now, engine, cost in ops:
                clock[0] = now
                outcomes.append(await bucket.acquire(engine, cost))
            local.append({"rate": rate, "burst": burst, "operations": ops, "expected": outcomes})
        sequences = []
        for name, allowed, ops in [
            ("three-denials", [False, False, False], ["acquire"] * 4 + ["reactivate"]),
            ("success-resets", [False, True, False], ["acquire"] * 3),
            (
                "provider-cooldown-no-strikes",
                [True],
                [["publish", 60], "acquire", "remaining", "reactivate", ["shared", 0], "acquire"],
            ),
            ("extend-only-local", [], [["publish", 90], ["publish", 2], "remaining"]),
        ]:
            clock[0] = 100.0
            strategy = FakeStrategy(allowed)
            limiter = ratelimit.RateLimiter(strategy)
            outputs = []
            for operation in ops:
                kind = operation if isinstance(operation, str) else operation[0]
                value = None
                if kind == "acquire":
                    value = await limiter.acquire("fakeeng")
                elif kind == "reactivate":
                    limiter.reactivate("fakeeng")
                elif kind == "remaining":
                    value = await limiter.upstream_cooldown_remaining("fakeeng")
                elif kind == "publish":
                    value = await limiter.set_upstream_cooldown("fakeeng", operation[1])
                elif kind == "shared":
                    strategy.shared = operation[1]
                state = limiter._states.get("fakeeng")
                outputs.append(
                    {
                        "value": value,
                        "state": vars(state) if state else None,
                        "strategy_calls": strategy.calls,
                        "deactivated": sorted(limiter.deactivated_engines),
                    }
                )
                outputs[-1] = copy.deepcopy(outputs[-1])
            sequences.append({"name": name, "allowed": allowed, "operations": ops, "outputs": outputs})
        quota_law = []

        class Counter:
            count = 0

            async def incrby(self, key, cost):
                self.count += cost
                return self.count

            async def expire(self, key, seconds):
                pass

        for rate, costs in [(3, [1, 1, 1, 1, 0, 2]), (2.5, [1, 1, 1]), (0, [0, 1, 0]), (3, [2, 2])]:
            strategy = ratelimit.ValkeySlidingWindow(default_rate=rate)
            strategy._client = Counter()
            strategy._connected = True
            outputs = []
            for cost in costs:
                outputs.append({"allowed": await strategy.acquire("fakeeng", cost), "count": strategy._client.count})
            quota_law.append({"rate": rate, "costs": costs, "outputs": outputs})
        ttl_law = []

        class ExpiringCounter(Counter):
            async def expire(self, key, seconds):
                self.expiry = seconds

        for window in [1, 0.75, 0.25, 0.001]:
            strategy = ratelimit.ValkeySlidingWindow(window_seconds=window)
            strategy._client = ExpiringCounter()
            strategy._connected = True
            await strategy.acquire("fakeeng", 1)
            ttl_law.append({"window": window, "source_expiry_seconds": strategy._client.expiry})
        health = []
        for status in adapter.EngineStatus:
            for latency in [0.0, 3.25, -1.5]:
                clock[0] = 1_700_000_000.25
                engine = FakeEngine()
                engine.status, engine.latency, engine.count = (
                    status,
                    latency,
                    2 if status == adapter.EngineStatus.OK else 0,
                )
                engine.consecutive_errors = 2
                await SearchService(
                    AppContext(active_engines={engine.name: engine}, tier1_engines={engine.name})
                ).search(SearchRequest(query="redacted fixture"))
                health.append(
                    {
                        "name": f"{status.value}-{latency}",
                        "origin": "adapter",
                        "status": status.value,
                        "latency": latency,
                        "result_count": engine.count,
                        "initial_errors": 2,
                        "synthetic": False,
                        "now": clock[0],
                        "expected": capabilities.build_engine_health(engine, now=clock[0], stale_after=300),
                    }
                )
        for name, status, interactive in [
            ("dispatched-synthetic-timeout", adapter.EngineStatus.TIMEOUT, False),
            ("caller-cancelled", adapter.EngineStatus.UNAVAILABLE, True),
        ]:
            engine = FakeEngine()
            engine.status, engine.synthetic, engine.latency = status, True, 9999
            request = SearchRequest(query="redacted", interactive_timeout_ms=100 if interactive else None)
            await SearchService(AppContext(active_engines={engine.name: engine})).search(request)
            health.append(
                {
                    "name": name,
                    "origin": "caller_cancelled" if interactive else "synthetic",
                    "status": status.value,
                    "latency": 9999,
                    "result_count": 0,
                    "initial_errors": 0,
                    "synthetic": True,
                    "now": clock[0],
                    "expected": capabilities.build_engine_health(engine, now=clock[0], stale_after=300),
                }
            )
        # Actual queued service tasks never reach their adapter and caller deadline
        # cancellation does not invent upstream errors/latency.
        first, queued = FakeEngine(), FakeEngine()
        first.name, queued.name, first.hang, queued.hang = "first", "queued", True, True
        await SearchService(
            AppContext(active_engines={"first": first, "queued": queued}, engine_semaphore=asyncio.Semaphore(1))
        ).search(SearchRequest(query="redacted", interactive_timeout_ms=10))
        assert first.calls == 1 and queued.calls == 0
        health.append(
            {
                "name": "never-dispatched",
                "origin": "undispatched",
                "status": "timeout",
                "latency": 9999,
                "result_count": 0,
                "initial_errors": 0,
                "synthetic": True,
                "now": clock[0],
                "expected": capabilities.build_engine_health(queued, now=clock[0], stale_after=300),
                "adapter_calls": queued.calls,
            }
        )
        views = []
        engine = FakeEngine()
        for at in [0, 0.25, -0.25, 1_700_000_000.123456, 1_700_000_000.0]:
            clock[0] = at
            engine.record_observation(adapter.EngineStatus.OK, latency_ms=3.25, result_count=2)
            for age in [0, 300, 300.0001, -1]:
                clock[0] = at + age
                for configured, auth_class, auth_configured in [
                    (True, None, None),
                    (False, "api_key", False),
                    (True, "none", True),
                ]:
                    cap = SimpleNamespace(enabled=configured, auth_class=auth_class, auth_configured=auth_configured)
                    views.append(
                        {
                            "at": at,
                            "now": clock[0],
                            "context": [configured, auth_class, auth_configured],
                            "expected": capabilities.build_engine_health(engine, cap, now=clock[0], stale_after=300),
                        }
                    )
        circuit = []
        for threshold, timeout in [(5, 300), (1, 2), (0, 0), (-1, -2)]:
            clock[0] = 100.0
            engine = FakeEngine()
            engine._circuit_threshold, engine._circuit_timeout = threshold, timeout
            outputs = []
            for kind, now in [["failure", 100]] * 6 + [["check", 399.999], ["check", 400], ["success", 401]]:
                clock[0] = now
                if kind == "failure":
                    engine.record_failure()
                elif kind == "success":
                    engine.record_success()
                outputs.append(
                    {
                        "operation": kind,
                        "now": now,
                        "errors": engine.consecutive_errors,
                        "open_until": engine.circuit_open_until,
                        "allowed": engine.circuit_allowed(),
                        "open": engine.circuit_open,
                    }
                )
            circuit.append({"threshold": threshold, "timeout": timeout, "outputs": outputs})
        retry = []
        inputs = [
            None,
            "",
            " ",
            "0",
            "0.25",
            "1",
            "1.5",
            "45",
            "999999",
            "-1",
            "nan",
            "inf",
            "1e309",
            "1_0",
            "１.５",
            "1__0",
            "nonsense",
            "Wed, 15 Nov 2023 00:00:45 GMT",
            "Wednesday, 15-Nov-23 00:00:45 GMT",
            "Wed Nov 15 00:00:45 2023",
            "15 November 2023 00:00:45 +0000",
            "15 Nov 2023 00:00:45",
            "15 Nov 2023 00:00:45 XYZ",
            "15 Nov 2023 01:00:45 +0100",
            "15 Nov 2023 00.00.45 GMT",
            "Wed,15 Nov 2023 00:00:45 GMT",
            "Wed, 31 Feb 2023 00:00:45 GMT",
            "15 Nov 2023 00:00:45 +2400",
        ]
        for value in inputs:
            for now in [1_700_006_400, 1_700_006_445]:
                retry.append({"input": value, "now": now, "expected": _parse_retry_after(value, now=now)})
        for value in [
            "15 Nov ² 00:00:45 2023",
            "15 Nov 2023,, 00:00:45 GMT",
            "14 Nov 2023 19:00:45 EſT",
            "\x1c45",
            "\x1f45",
        ]:
            retry.append(
                {"input": value, "now": 1_700_006_400, "expected": _parse_retry_after(value, now=1_700_006_400)}
            )
        settings = []
        for threshold in [None, "", "bad", "0", "-2", "２", "\x1c2", "1_0"]:
            for timeout in [None, "", "-1", "2.0", "２"]:
                for key, value in [("ENGINE_CIRCUIT_THRESHOLD", threshold), ("ENGINE_CIRCUIT_TIMEOUT", timeout)]:
                    if value is None:
                        os.environ.pop(key, None)
                    else:
                        os.environ[key] = value
                engine = FakeEngine()
                settings.append(
                    {
                        "threshold": threshold,
                        "timeout": timeout,
                        "expected": [engine._circuit_threshold, engine._circuit_timeout],
                    }
                )
        freshness = []
        for raw in [None, "", "bad", "0", "-2", "２", "1_0", "inf", "nan", "0.0001", "\x1c2", "\x1f2"]:
            if raw is None:
                os.environ.pop("OBSERVED_HEALTH_STALE_SECONDS", None)
            else:
                os.environ["OBSERVED_HEALTH_STALE_SECONDS"] = raw
            expected = capabilities.observed_health_stale_seconds()
            freshness.append({"input": raw, "expected": "infinite" if expected == float("inf") else expected})
        bounds = []
        for value in [0, -1, 0.25, 1, 1.5, 300, 301]:
            clock[0] = 100
            strategy = FakeStrategy([])
            limiter = ratelimit.RateLimiter(strategy)
            bounds.append({"input": value, "expected": await limiter.set_upstream_cooldown("fakeeng", value)})
        url_resolution = []
        for explicit in ["", "redis://explicit"]:
            for environment in [None, "", "redis://injected"]:
                if environment is None:
                    os.environ.pop("VALKEY_URL", None)
                else:
                    os.environ["VALKEY_URL"] = environment
                strategy = ratelimit.ValkeySlidingWindow(valkey_url=explicit)
                url_resolution.append({"explicit": explicit, "environment": environment, "expected": strategy._url})
        os.environ.pop("VALKEY_URL", None)
        fail_settings = []
        for fail_closed in [False, True]:
            strategy = ratelimit.ValkeySlidingWindow(fail_closed=fail_closed)
            warmup = await strategy.warmup()
            direct = await strategy.acquire("fakeeng")
            wrapped = ratelimit.RateLimiter(strategy)
            allowed = [await wrapped.acquire("fakeeng") for _ in range(4)]
            shutdown = await strategy.shutdown()
            fail_settings.append(
                {
                    "fail_closed": fail_closed,
                    "direct_allowed": direct,
                    "wrapped_allowed": allowed,
                    "warmup": warmup,
                    "shutdown": shutdown,
                }
            )
        timestamp_limits = []
        for at in [
            -62_135_596_800.0,
            math.nextafter(-62_135_596_800.0, -math.inf),
            math.nextafter(-62_135_596_800.0, math.inf),
            253_402_300_800.0,
            math.nextafter(253_402_300_800.0, -math.inf),
            math.nextafter(253_402_300_800.0, math.inf),
            -0.0000005,
            0.0000005,
            0.0000015,
            0.9999995,
            1.9999995,
        ]:
            try:
                expected = capabilities._iso_timestamp(at)
            except (ValueError, OverflowError, OSError):
                expected = None
            timestamp_limits.append({"at": at, "expected": expected})
        return {
            "local": local,
            "sequences": sequences,
            "quota_law": quota_law,
            "ttl_law": ttl_law,
            "settings": settings,
            "freshness": freshness,
            "health": health,
            "views": views,
            "circuit": circuit,
            "retry_after": retry,
            "bounds": bounds,
            "registry": sorted(adapter.list_engines()),
            "url_resolution": url_resolution,
            "fail_settings": fail_settings,
            "timestamp_limits": timestamp_limits,
            "cooldown_script_sha256": hashlib.sha256(
                ratelimit._SET_OR_EXTEND_UPSTREAM_COOLDOWN.strip().encode()
            ).hexdigest(),
        }

    output = asyncio.run(capture())
    output.update(
        schema_version=1,
        baseline=BASELINE,
        python="3.14.7",
        sources=sources,
        exporter_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    )
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
