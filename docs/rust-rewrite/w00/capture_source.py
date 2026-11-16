"""Temporary offline W00 source oracle; retire with G08. Never executes a search."""

from __future__ import annotations

import asyncio
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import valkey

from slopsearx.mcp import tools as t
from slopsearx.mcp.state import set_state
from slopsearx.mcp.tools import _saved_policy_fingerprint, _saved_positive
from slopsearx.saved_models import SavedDefinition, definition_from_payload, definition_to_payload, stable_id
from slopsearx.saved_runner import SavedSearchRunner
from slopsearx.saved_store import _CLAIM, LEASE_SECONDS, STORE_TTL_MARGIN, SavedSearchStore
from tests.test_research_retry_followup import _build_state


async def capture_controls():
    rows = []
    with (
        patch.dict(os.environ, {}, clear=True),
        patch.object(t.time, "time", lambda: 1000.0),
        patch.object(t, "generate_search_id", lambda: "saved-control"),
    ):
        for scenario in ["revoked_engine_control", "reduced_numeric_limits"]:
            state = _build_state(["brave"])
            state.policy.enabled_tools["saved_searches"] = True
            state.saved_store = SavedSearchStore(state.ctx.cache)
            state.saved_runner = SimpleNamespace()
            set_state(state)
            created = await t.slopsearx_create_saved_search("q", ["brave"], 60, expires_in_seconds=600)
            assert "error" not in created, created
            if scenario == "revoked_engine_control":
                state.ctx.active_engines.pop("brave")
                first = await t.slopsearx_pause_saved_search("saved-control", 1, True)
                second = await t.slopsearx_pause_saved_search("saved-control", 2, False)
                state.policy.enabled_tools["saved_searches"] = False
                third = await t.slopsearx_pause_saved_search("saved-control", 3, True)
            else:
                state.policy.saved_max_results = 1
                first = await t.slopsearx_update_saved_search("saved-control", 1, query="changed")
                second = await t.slopsearx_update_saved_search("saved-control", 2, max_results=100)
                third = await t.slopsearx_update_saved_search("saved-control", 2, max_results=1)
            rows.append(
                {
                    "name": scenario,
                    "created": created,
                    "outcomes": [first, second, third],
                    "final": definition_to_payload(await state.saved_store.load("saved-control", now=1000.0)),
                }
            )
            assert all(engine.calls == 0 for engine in state.ctx.active_engines.values())
            set_state(None)
    return rows


async def capture_arithmetic_error():
    definition = SavedDefinition(
        "saved-overflow", "tenant", "q", ["brave"], 60, 600, 10, 20, -sys.float_info.max, sys.float_info.max, 1000.0
    )

    class Store:
        def for_tenant(self, tenant):
            return self

        async def load(self, identity, **kwargs):
            return definition

        async def claim(self, identity, **kwargs):
            return definition

    calls = []

    async def forbidden(*args, **kwargs):
        calls.append(True)
        raise RuntimeError("source arithmetic probe reached dispatch")

    runner = SavedSearchRunner(
        SimpleNamespace(search=forbidden),
        Store(),
        SimpleNamespace(saved_max_concurrent_runs=2),
        policy_check=lambda _: None,
        policy_fingerprint=lambda _: "",
        clock=lambda: sys.float_info.max,
    )
    try:
        await runner.run_one("tenant", "saved-overflow", now=sys.float_info.max)
    except ValueError as error:
        if calls:
            raise RuntimeError("source arithmetic probe dispatched") from error
        return {
            "now": sys.float_info.max,
            "created_at": -sys.float_info.max,
            "error_type": type(error).__name__,
            "message": str(error),
            "dispatch_calls": len(calls),
            "captured_by": "SavedSearchRunner.run_one with no-dispatch stub store; source slot conversion",
        }
    raise RuntimeError("source overflow behavior changed")


def capture_claims():
    binary = os.environ.get("SLOPSEARX_TEST_VALKEY_SERVER") or shutil.which("valkey-server")
    if binary is None:
        raise RuntimeError("explicit disposable Valkey binary required for source capture")
    version = subprocess.check_output([binary, "--version"], text=True)
    if not version.startswith("Valkey server v=8.1.10 "):
        raise RuntimeError("source oracle requires pinned Valkey8.1.10")
    rows = []
    with tempfile.TemporaryDirectory(prefix="w00-", dir="/private/tmp") as folder:
        socket = str(Path(folder) / "v.sock")
        process = subprocess.Popen(
            [
                binary,
                "--port",
                "0",
                "--save",
                "",
                "--appendonly",
                "no",
                "--unixsocket",
                socket,
                "--unixsocketperm",
                "700",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            client = valkey.Valkey(unix_socket_path=socket)
            for _ in range(200):
                try:
                    client.ping()
                    break
                except valkey.ConnectionError:
                    time.sleep(0.01)
            for name, due in [
                ("fractional", 1700000000.123456),
                ("exponent", 1.234567890123456e-8),
                ("minimum_normal", float.fromhex("0x1.0p-1022")),
                ("minimum_subnormal", float.fromhex("0x0.0000000000001p-1022")),
                ("large_negative", -1e15),
                ("stale_clock", 1000.0),
            ]:
                expiry = 1600.0 if name == "stale_clock" else 1e12
                definition = SavedDefinition(
                    "saved-precision", "a:b", "q", ["brave"], 60, 600, 10, 20, due - 60.0, expiry, due
                )
                payload = definition_to_payload(definition)
                client.setex("source-record", 300, json.dumps(payload, separators=(",", ":")))
                client.delete("source-lease")
                caller_now = 1000.0 if name == "stale_clock" else 1700000000.123456
                committed = client.eval(
                    _CLAIM, 2, "source-record", "source-lease", str(caller_now), "fixture-token", "60", "300"
                )
                seconds, micros = client.time()
                decoded = json.loads(committed)
                rehydrated = definition_from_payload(decoded)
                rows.append(
                    {
                        "name": name,
                        "input": payload,
                        "caller_now": caller_now,
                        "committed": decoded,
                        "run_id_before": "run-"
                        + stable_id(definition.search_id, definition.revision, definition.next_due)[:24],
                        "run_id_after": "run-"
                        + stable_id(rehydrated.search_id, rehydrated.revision, rehydrated.next_due)[:24],
                        "logical_expired_at_server": expiry <= seconds + micros / 1e6,
                        "lease_seconds": client.ttl("source-lease"),
                    }
                )
        finally:
            process.terminate()
            process.wait(timeout=5)
    return rows


ROOT = Path(__file__).resolve().parents[3]


async def capture_review_boundaries(socket):
    import valkey.asyncio as async_valkey

    from tests.test_research_queue_integration import Store

    client = async_valkey.Valkey(unix_socket_path=socket)
    backend = Store(client)
    store = SavedSearchStore(backend, "review")
    rows = []
    now = 1000.0
    with patch("slopsearx.saved_store.secrets.token_urlsafe", return_value="review-owner"):
        for name, baseline in [
            ("live", {"expires_at": 1300.0, "records": {"previous": "opaque"}}),
            ("expired", {"expires_at": 999.0, "records": {}}),
            ("missing", None),
            ("malformed_expiry", {"expires_at": "invalid", "records": {}}),
        ]:
            identity = "saved-baseline-" + name
            d = SavedDefinition(identity, "review", "q", ["brave"], 60, 600, 10, 20, now, 1600.0, now)
            assert await store.create(d, 20, now=now)
            if baseline is not None:
                await backend.set(store._baseline_key(identity), baseline, 600)
            try:
                claimed = await store.claim(identity, now=now)
                outcome = {"baseline": claimed.baseline, "revision": claimed.revision}
            except Exception as error:
                outcome = {"error_type": type(error).__name__}
            rows.append(
                {
                    "name": "baseline_" + name,
                    "baseline_input": baseline,
                    "outcome": outcome,
                    "lease_exists": bool(await client.exists(store._lease_key(identity))),
                }
            )
        for revision in [2**53 - 1, 2**53, 2**53 + 1, 2**63 - 2, 2**63 - 1]:
            identity = "saved-revision-" + str(revision)
            d = SavedDefinition(
                identity, "review", "q", ["brave"], 60, 600, 10, 20, now, 1600.0, now, revision=revision
            )
            await client.set(store._definition_key(identity), json.dumps(definition_to_payload(d)))
            d.query = "changed"
            updated = await store.compare_and_set(d, revision, now=now)
            rows.append({"name": "revision_increment", "revision": revision, "updated_revision": updated.revision})
    with (
        patch.dict(os.environ, {}, clear=True),
        patch.object(t.time, "time", lambda: now),
        patch.object(t, "generate_search_id", lambda: "saved-query-review"),
    ):
        state = _build_state(["brave"])
        state.policy.enabled_tools["saved_searches"] = True
        state.policy.max_query_length = 1000
        state.saved_store = SavedSearchStore(state.ctx.cache)
        state.saved_runner = SimpleNamespace()
        set_state(state)
        try:
            created = await t.slopsearx_create_saved_search("q" * 600, ["brave"], 60)
            rows.append({"name": "configured_query_create", "maximum": 1000, "raw_length": 600, "outcome": created})
            rejected = await t.slopsearx_create_saved_search(" " * 401 + "q" * 600, ["brave"], 60)
            rows.append({"name": "raw_query_before_strip", "maximum": 1000, "raw_length": 1001, "outcome": rejected})
            state.policy.max_query_length = 10
            updated = await t.slopsearx_update_saved_search(created["search_id"], 1, max_reports=1)
            rows.append({"name": "untouched_query_lowered_limit", "maximum": 10, "outcome": updated})
            rejected = await t.slopsearx_update_saved_search(created["search_id"], 2, query="q" * 600)
            rows.append({"name": "supplied_query_lowered_limit", "maximum": 10, "outcome": rejected})
            assert all(engine.calls == 0 for engine in state.ctx.active_engines.values())
        finally:
            set_state(None)
    await client.aclose()
    return rows


async def capture_final_review_boundaries(socket):
    import valkey.asyncio as async_valkey

    from tests.test_research_queue_integration import Store

    client = async_valkey.Valkey(unix_socket_path=socket)
    backend = Store(client)
    store = SavedSearchStore(backend, "final-review")
    now = 1000.0
    rows = []
    with patch("slopsearx.saved_store.secrets.token_urlsafe", return_value="shape-owner"):
        for name, value, stored in [
            ("empty_array", [], True),
            ("array", [1], True),
            ("integer", 0, True),
            ("boolean", True, True),
            ("string", "", True),
            ("json_null", None, True),
            ("object_missing_expiry", {}, True),
            ("absent_key", None, False),
        ]:
            identity = "saved-shape-" + name
            d = SavedDefinition(identity, "final-review", "q", ["brave"], 60, 600, 10, 20, now, 1600.0, now)
            assert await store.create(d, 20, now=now)
            if stored:
                await client.set(store._baseline_key(identity), json.dumps(value))
            try:
                claimed = await store.claim(identity, now=now)
                outcome = {"claimed": claimed is not None, "baseline": claimed.baseline}
            except Exception as error:
                outcome = {"error_type": type(error).__name__}
            rows.append(
                {
                    "name": "baseline_shape_" + name,
                    "stored": stored,
                    "input": value,
                    "outcome": outcome,
                    "lease_exists": bool(await client.exists(store._lease_key(identity))),
                }
            )
    lifecycle = SavedSearchStore(backend, "max-lifecycle")
    d = SavedDefinition(
        "saved-max", "max-lifecycle", "q", ["brave"], 60, 600, 10, 20, now, 1600.0, now, revision=2**63 - 2
    )
    assert await lifecycle.create(d, 2, now=now)
    updated = await lifecycle.compare_and_set(d, d.revision, now=now)
    repaired = await lifecycle.repair_indexes(now=now)
    await lifecycle.delete(d.search_id, updated.revision, now=now)
    tombstone = (await client.get(lifecycle._tombstone_key(d.search_id))).decode()
    admitted = []
    for identity in ["saved-ordinary", "saved-replacement"]:
        candidate = SavedDefinition(identity, "max-lifecycle", "q", ["brave"], 60, 600, 10, 20, now, 1600.0, now)
        admitted.append(await lifecycle.create(candidate, 2, now=now))
    rows.append(
        {
            "name": "maximum_revision_lifecycle",
            "updated_revision": updated.revision,
            "repaired": repaired,
            "deleted": True,
            "tombstone": tombstone,
            "subsequent_admitted": admitted,
            "final_count": await lifecycle.repair_indexes(now=now),
        }
    )
    await client.aclose()
    return rows


def capture_review(capture_fn=capture_review_boundaries):
    binary = os.environ.get("SLOPSEARX_TEST_VALKEY_SERVER", "valkey-server")
    with tempfile.TemporaryDirectory(prefix="w00-review-", dir="/private/tmp") as folder:
        socket = str(Path(folder) / "v.sock")
        process = subprocess.Popen(
            [
                binary,
                "--port",
                "0",
                "--save",
                "",
                "--appendonly",
                "no",
                "--unixsocket",
                socket,
                "--unixsocketperm",
                "700",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            client = valkey.Valkey(unix_socket_path=socket)
            for _ in range(200):
                try:
                    client.ping()
                    break
                except valkey.ConnectionError:
                    time.sleep(0.01)
            return asyncio.run(capture_fn(socket))
        finally:
            process.terminate()
            process.wait(timeout=5)


def capture():
    if sys.version_info[:3] != (3, 14, 7):
        raise RuntimeError("source oracle requires pinned CPython 3.14.7")
    observations = []
    for name, created, due, instant, last_slot in [
        ("first", 1000.0, 1060.0, 1060.0, -1),
        ("immediate", 1000.0, 1000.0, 1000.0, -1),
        ("missed", 1000.0, 1060.0, 1305.5, -1),
        ("recovered", 1000.0, 1060.0, 1421.0, 2),
        ("fractional", 1000.125, 1060.125, 1120.124, -1),
        ("boundary", 1000.125, 1060.125, 1120.125, 0),
    ]:
        definition = SavedDefinition(
            "saved-é",
            "tenant:a[*]",
            "雪 query",
            ["z", "a"],
            60,
            600,
            10,
            20,
            created,
            1600.5,
            due,
            policy_fingerprint="f" * 64,
            last_slot=last_slot,
        )
        slot = max(0, int((instant - definition.created_at) // definition.interval_seconds))
        observations.append(
            {
                "name": name,
                "input": definition_to_payload(definition),
                "now": instant,
                "window": definition.window(),
                "slot": slot,
                "missed_slots": max(0, slot - last_slot - 1) if last_slot >= 0 else max(0, slot - 1),
                "run_id": "run-" + stable_id(definition.search_id, definition.revision, definition.next_due)[:24],
                "following_due": created + (slot + 1) * 60,
                "ttl": SavedSearchStore(None)._ttl(definition, instant),
            }
        )
    base = observations[0]["input"]
    for name, changes in [
        ("legacy_defaults", {}),
        (
            "source_coercions",
            {"query": 123, "engines": [True, 2], "interval_seconds": "60", "paused": "false", "lease_token": 0},
        ),
        ("integer_time_rehydration", {"created_at": 1000, "next_due": 1060}),
    ]:
        payload = dict(base)
        for field in (
            "revision",
            "paused",
            "policy_fingerprint",
            "last_slot",
            "latest_run_id",
            "baseline",
            "lease_token",
            "lease_expires_at",
            "deleted",
        ):
            payload.pop(field, None)
        payload.update(changes)
        decoded = definition_from_payload(payload)
        observations.append(
            {
                "name": name,
                "input": payload,
                "decoded": definition_to_payload(decoded),
                "run_id": "run-" + stable_id(decoded.search_id, decoded.revision, decoded.next_due)[:24],
            }
        )
    capabilities = {
        name: SimpleNamespace(enabled=True, auth_configured=False, categories=categories)
        for name, categories in [("brave", ["news", "general"]), ("arxiv", ["science"])]
    }
    state = SimpleNamespace(
        catalog=capabilities,
        ctx=SimpleNamespace(active_engines={"brave": object(), "arxiv": object()}),
        policy=SimpleNamespace(
            tool_enabled=lambda _: True, targeted_sensitive_allowed=False, sensitive_engines={"hibp", "dehashed"}
        ),
    )
    definition = SavedDefinition("saved-a", "a:b", "q", ["brave", "arxiv"], 60, 600, 100, 20, 1000.0, 1600.0, 1060.0)
    policies = []
    for name in ("public_keyless", "configured_key", "inactive"):
        if name == "configured_key":
            capabilities["brave"].auth_configured = True
        if name == "inactive":
            state.ctx.active_engines.pop("arxiv")
        policies.append({"name": name, "fingerprint": _saved_policy_fingerprint(state, definition)})
    strict = []
    for value in [True, False, 59, 60, 86400, 86401, 60.0, "60", None, -1]:
        try:
            outcome = {"ok": _saved_positive(value, "interval_seconds", 60, 86400)}
        except ValueError as error:
            outcome = {"error_type": type(error).__name__, "message": str(error)}
        strict.append({"input": value, "outcome": outcome})
    capabilities["brave"].auth_configured = False
    capabilities["brave"].categories = ["x" * (4 * 1024 * 1024 + 1)]
    definition.engines = ["brave"]
    oversized_policy = {
        "category_repeat": 4 * 1024 * 1024 + 1,
        "character": "x",
        "fingerprint": _saved_policy_fingerprint(state, definition),
    }
    source_paths = [
        "slopsearx/saved_models.py",
        "slopsearx/saved_store.py",
        "slopsearx/saved_runner.py",
        "slopsearx/mcp/tools.py",
        "tests/test_research_retry_followup.py",
    ]
    return {
        "format": "w00-source-v1",
        "schema_version": 1,
        "plan_id": "W00",
        "baseline": "66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5",
        "python": "3.14.7",
        "valkey_version": "8.1.10",
        "provenance": (
            "actual offline source functions and private Valkey; no live provider calls; "
            "temporary developer oracle retires at G08"
        ),
        "exporter_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "sources": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in source_paths},
        "constants": {"retention_margin": STORE_TTL_MARGIN, "lease_seconds": LEASE_SECONDS},
        "observations": observations,
        "policy_observations": policies,
        "strict_interval_observations": strict,
        "control_observations": asyncio.run(capture_controls()),
        "claim_observations": capture_claims(),
        "review_observations": capture_review(),
        "final_review_observations": capture_review(capture_final_review_boundaries),
        "oversized_policy_observation": oversized_policy,
        "arithmetic_error_observation": asyncio.run(capture_arithmetic_error()),
        "oversized_identity_observation": {
            "repeat": 4 * 1024 * 1024 + 1,
            "character": "x",
            "revision": 1,
            "next_due": 1060.0,
            "run_id": "run-" + stable_id("x" * (4 * 1024 * 1024 + 1), 1, 1060.0)[:24],
            "source_window_query_length": len(
                SavedDefinition(
                    "saved", "tenant", "x" * (4 * 1024 * 1024 + 1), ["brave"], 60, 600, 10, 20, 1000.0, 1600.0, 1060.0
                ).window()["query"]
            ),
        },
    }


if __name__ == "__main__":
    output = Path(__file__).with_name("source.json")
    output.write_text(json.dumps(capture(), ensure_ascii=False, sort_keys=True, indent=2) + "\n")
