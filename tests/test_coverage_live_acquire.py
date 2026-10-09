"""Synthetic contract tests for admitted, bounded coverage acquisition."""

from __future__ import annotations

import asyncio
import hashlib
import json
import uuid
from urllib.parse import parse_qs

import httpx
import pytest

from engines.arxiv import ArxivAdapter
from scripts import coverage_live_acquire as live
from scripts import coverage_study_acquire as offline
from scripts.coverage_study_core import ACQUISITION_SCHEMA


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def canonical(value: object) -> bytes:
    payload = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
    return (payload + "\n").encode()


class FakePermitVerifier:
    def __init__(self):
        self.calls = 0

    def verify(self, manifest, manifest_sha256, receipt_bytes, expected_receipt_sha256):
        self.calls += 1
        return live.VerifiedAcquisitionPermit(
            status="verified",
            scope="source-acquisition",
            stage_uuid=manifest["stage_uuid"],
            source_revision=manifest["source_revision"],
            source_closure_sha256=manifest["source_closure_sha256"],
            protocol_sha256=manifest["protocol_sha256"],
            cohorts_sha256=manifest["cohorts_sha256"],
            input_manifest_sha256=manifest["input_manifest_sha256"],
            acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
            stage_manifest_sha256=manifest_sha256,
            receipt_sha256=expected_receipt_sha256,
        )


class OneShot:
    def __init__(self):
        self.calls = 0

    def consume_once(self, permit):
        self.calls += 1
        return {"status": "consumed", "stage_uuid": permit.stage_uuid, "receipt_sha256": "a" * 64}


class FakePacer:
    def __init__(self):
        self.sleeps = []
        self.elapsed = 0.0

    async def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.elapsed += seconds

    def monotonic(self):
        return self.elapsed


def _pool_plan(engines: list[str], band: str | None = None) -> dict:
    plan = {
        "engines": engines,
        "searx_categories": ["reference"],
        "engine_configs": {engine: {"max_results": 20} for engine in engines},
    }
    if band is not None:
        plan["band"] = band
    return plan


def make_manifest(*, source_revision: str = "b" * 40, include_arxiv: bool = False) -> tuple[dict, bytes]:
    stage_uuid = str(uuid.UUID(int=98765))
    research = [
        {
            "task_id": f"R{index:02d}",
            "search_query": f"synthetic query {index}",
            "pool_plan": _pool_plan(
                ["arxiv", "openalex"] if include_arxiv and index == 1 else ["github", "openalex"],
                "research_le_40",
            ),
        }
        for index in range(1, 9)
    ]
    navigation = [
        {
            "target_id": f"N{index:02d}",
            "search_query": f"synthetic nav {index}",
            "target_url": f"https://github.com/sample/nav-{index}",
            "navigation_pool_plan": _pool_plan(["github"]),
        }
        for index in range(1, 6)
    ]
    manifest = {
        "schema": "coverage-live-acquisition-manifest/1",
        "stage": "development",
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "source_closure_sha256": "1" * 64,
        "protocol_sha256": "2" * 64,
        "cohorts_sha256": "3" * 64,
        "input_manifest_sha256": "4" * 64,
        "acquisition_plan_sha256": "5" * 64,
        "research_cases": research,
        "navigation_targets": navigation,
    }
    rows = []
    for row in research:
        rows.append(
            {
                "task_id": row["task_id"],
                "engine_calls": {
                    "arxiv": 2 if include_arxiv and row["task_id"] == "R01" else 0,
                    "github": 0 if include_arxiv and row["task_id"] == "R01" else 1,
                    "openalex": 1,
                    "wikipedia": 0,
                },
            }
        )
    for row in navigation:
        rows.append(
            {
                "task_id": row["target_id"],
                "engine_calls": {"arxiv": 0, "github": 1, "openalex": 0, "wikipedia": 0},
            }
        )
    plan = canonical({"schema": ACQUISITION_SCHEMA, "stage_uuid": stage_uuid, "tasks": rows})
    manifest["acquisition_plan_sha256"] = sha(plan)
    return manifest, plan


def _mock_handler(*, status=200, oversized=False, large=False, calls=None, arxiv_redirect=False):
    def handle(request: httpx.Request) -> httpx.Response:
        if calls is not None:
            calls.append(request)
        if status != 200:
            return httpx.Response(status, content=b"synthetic failure", request=request)
        if oversized:
            return httpx.Response(200, content=b"x" * (2_000_001), request=request)
        query = parse_qs(request.url.query.decode())
        term = query.get("q", query.get("search", query.get("action", [""])))[0]
        if request.url.host == "export.arxiv.org":
            if arxiv_redirect and query.get("redirected") != ["1"]:
                return httpx.Response(
                    302,
                    headers={"location": "/api/query?redirected=1"},
                    request=request,
                )
            body = (
                '<feed xmlns="http://www.w3.org/2005/Atom">'
                "<entry><id>https://arxiv.org/abs/2601.12345</id>"
                "<title>Synthetic paper</title><summary>Evidence</summary>"
                "<published>2026-01-01T00:00:00Z</published></entry></feed>"
            )
            return httpx.Response(200, content=body.encode(), request=request)
        if request.url.host == "api.github.com":
            if term.startswith("synthetic nav "):
                number = term.removeprefix("synthetic nav ")
                items = [
                    {
                        "full_name": f"sample/nav-{number}",
                        "html_url": f"https://github.com/sample/nav-{number}",
                        "description": "synthetic repository",
                        "stargazers_count": 1,
                        "language": "Python",
                    }
                ]
            else:
                items = [
                    {
                        "full_name": f"sample/{term.replace(' ', '-')}-{index}",
                        "html_url": f"https://github.com/sample/{term.replace(' ', '-')}-{index}",
                        "description": f"synthetic repository {index}" + ("x" * 30_000 if large else ""),
                        "stargazers_count": index,
                        "language": "Python",
                    }
                    for index in range(20)
                ]
            return httpx.Response(200, json={"total_count": len(items), "items": items}, request=request)
        items = [
            {
                "id": f"https://openalex.org/W{index}",
                "doi": f"https://doi.org/10.1234/{index}",
                "title": f"OpenAlex {term} {index}" + ("x" * 30_000 if large else ""),
                "publication_date": "2025-01-01",
                "relevance_score": 10.0 - index / 100,
                "abstract_inverted_index": None,
            }
            for index in range(20)
        ]
        return httpx.Response(200, json={"results": items}, request=request)

    return handle


async def _run(
    tmp_path,
    *,
    handler=None,
    verifier=None,
    lease=None,
    receipt=b"sealed fixture",
    pacer=None,
    include_arxiv=False,
):
    manifest, plan = make_manifest(include_arxiv=include_arxiv)
    verifier = verifier or FakePermitVerifier()
    lease = lease or OneShot()
    response_calls = []
    transport = httpx.MockTransport(handler or _mock_handler(calls=response_calls))
    pacer = pacer or FakePacer()
    receipt_hash = sha(receipt)
    result = await live.acquire_live_coverage_stage(
        stage_manifest=manifest,
        acquisition_plan_bytes=plan,
        permit_receipt_bytes=receipt,
        expected_permit_receipt_sha256=receipt_hash,
        permit_verifier=verifier,
        one_shot_lease=lease,
        receipt_directory=tmp_path / "receipt-sink",
        test_transport=transport,
        pacer=pacer,
    )
    return result, verifier, lease, pacer, response_calls


@pytest.mark.asyncio
async def test_full_synthetic_stage_uses_pinned_scope_and_durable_private_receipts(tmp_path):
    result, verifier, lease, pacer, calls = await _run(tmp_path)
    assert verifier.calls == lease.calls == 1
    assert result.stage.status == "complete"
    assert len(result.stage.operations) == 13
    assert result.stage.physical_request_count == 21
    assert len(calls) == 21
    assert pacer.sleeps == [7.0] * 12
    assert [item.status for item in result.stage.operations] == ["complete"] * 13
    assert [item.band_valid for item in result.stage.operations[:8]] == [True] * 8
    assert [item.target_found_at_rank1 for item in result.stage.operations[8:]] == [True] * 5

    root = result.receipt_directory
    assert root.stat().st_mode & 0o777 == 0o700
    assert (root / "admission.json").stat().st_mode & 0o777 == 0o600
    assert (root / "stage-summary.json").stat().st_mode & 0o777 == 0o600
    snapshots = live.verify_pool_snapshot_index(
        root,
        expected_index_sha256=result.pool_snapshot_index_sha256,
        expected_stage_manifest_sha256=result.stage_manifest_sha256,
        expected_stage_uuid=make_manifest()[0]["stage_uuid"],
        expected_source_revision="b" * 40,
    )
    assert len(snapshots["operations"]) == 13
    first_snapshot = snapshots["operations"]["R01"]
    assert first_snapshot["native_order"] == [f"c{index}" for index in range(40)]
    assert first_snapshot["task_plan"]["task_id"] == "R01"
    assert len(first_snapshot["operation"]["canonical_response"]["results"]) == 40
    assert "payload" in first_snapshot["operation"]["canonical_response"]["results"][0]
    summary = json.loads((root / "stage-summary.json").read_bytes())
    assert summary["quality_credit"] is False
    assert summary["replacement_or_rescue"] is False
    final_rows = sorted(root.glob("http-*-final.json"))
    assert len(final_rows) == 21
    receipt_text = b"".join(path.read_bytes() for path in final_rows)
    assert b"synthetic query" not in receipt_text
    assert b"response_sha256" in receipt_text
    assert all(path.stat().st_mode & 0o777 == 0o600 for path in root.glob("http-*.json"))
    controls = summary["execution_control_attestation"]
    assert controls["control_identity"] == "coverage-acquisition-one-shot/1"
    assert controls["transport_retries_configured"] == 0
    assert controls["pacer_identity"] == "injected-mock-only"
    invocations = controls["operation_invocations"]
    assert len(invocations) == 13
    assert all(row["end_offset_us"] >= row["start_offset_us"] for row in invocations)
    assert all(
        right["start_offset_us"] - left["start_offset_us"] >= 7_000_000
        for left, right in zip(invocations, invocations[1:])
    )


@pytest.mark.asyncio
async def test_arxiv_redirect_dispatches_are_physically_paced_within_request_deadline(tmp_path):
    pacer = FakePacer()
    dispatched_at = []
    arxiv_requests = 0

    async def handler(request):
        nonlocal arxiv_requests
        dispatched_at.append(pacer.monotonic())
        arxiv_requests += 1
        if arxiv_requests == 1:
            return httpx.Response(302, headers={"location": "/api/query?redirected=1"}, request=request)
        return httpx.Response(
            200,
            content=b'<feed xmlns="http://www.w3.org/2005/Atom"></feed>',
            request=request,
        )

    sink = live.PrivateReceiptSink(tmp_path / "arxiv-pacing")
    transport = live._BoundedTransport(
        httpx.MockTransport(handler),
        sink,
        {"R01": {"arxiv": 2}, "R02": {"arxiv": 2}},
        pacer,
        pacer.monotonic,
    )
    adapter = ArxivAdapter(
        config={"base_url": "https://export.arxiv.org/api/query", "max_results": 1},
        rate_limiter=None,
    )
    adapter.set_http_transport(transport)
    op_token = offline._CURRENT_OPERATION.set("R01")
    engines_token = offline._CURRENT_ENGINES.set(("arxiv",))
    try:
        first = await adapter.search("synthetic query")
        assert first.status.value == "ok"
        assert arxiv_requests == 2
        await pacer.sleep(live.QUERY_PACING_SECONDS)
        offline._CURRENT_OPERATION.set("R02")
        await adapter.search("synthetic follow-up")
    finally:
        offline._CURRENT_ENGINES.reset(engines_token)
        offline._CURRENT_OPERATION.reset(op_token)
        await transport.aclose()

    assert arxiv_requests == 3
    assert len(transport.exchanges) == 3
    assert transport.exchanges[0].redirect_allowed is True
    assert dispatched_at[1] - dispatched_at[0] >= live.ARXIV_PHYSICAL_PACING_SECONDS
    assert dispatched_at[2] - dispatched_at[1] >= live.QUERY_PACING_SECONDS
    assert dispatched_at == [0.0, 3.0, 10.0]
    dispatch_receipts = [
        json.loads((sink.path / f"http-{index:04d}-final.json").read_bytes()) for index in range(1, 4)
    ]
    offsets = [row["dispatch_offset_us"] for row in dispatch_receipts]
    assert offsets == [0, 3_000_000, 10_000_000]


@pytest.mark.asyncio
async def test_arxiv_physical_wait_consumes_the_per_request_deadline(tmp_path, monkeypatch):
    dispatched = []

    async def handler(request):
        dispatched.append(request)
        return httpx.Response(
            302,
            headers={"location": "/api/query?redirected=1"},
            request=request,
        )

    class SlowPacer:
        async def sleep(self, seconds):
            await asyncio.sleep(0.08)

    monkeypatch.setattr(live, "REQUEST_TIMEOUT_SECONDS", 0.05)
    sink = live.PrivateReceiptSink(tmp_path / "arxiv-deadline")
    transport = live._BoundedTransport(
        httpx.MockTransport(handler),
        sink,
        {"R01": {"arxiv": 2}},
        SlowPacer(),
    )
    operation = offline._CURRENT_OPERATION.set("R01")
    engines = offline._CURRENT_ENGINES.set(("arxiv",))
    timeout = {key: 0.05 for key in ("connect", "read", "write", "pool")}
    try:
        first = await transport.handle_async_request(
            httpx.Request("GET", "https://export.arxiv.org/api/query", extensions={"timeout": timeout})
        )
        await first.aclose()
        with pytest.raises(httpx.ReadTimeout):
            await transport.handle_async_request(
                httpx.Request(
                    "GET",
                    "https://export.arxiv.org/api/query?redirected=1",
                    extensions={"timeout": timeout},
                )
            )
    finally:
        offline._CURRENT_ENGINES.reset(engines)
        offline._CURRENT_OPERATION.reset(operation)
        await transport.aclose()

    receipt = json.loads((sink.path / "http-0002-final.json").read_bytes())
    assert len(dispatched) == 1
    assert transport.physical_dispatches == 1
    assert receipt["failure_code"] == "request_deadline_before_dispatch"
    assert receipt["dispatched"] is False


@pytest.mark.asyncio
async def test_stage_aggregate_may_exceed_two_mb_when_each_response_is_bounded(tmp_path):
    result, _verifier, _lease, _pacer, _calls = await _run(tmp_path, handler=_mock_handler(large=True))
    assert result.stage.status == "complete"
    assert result.stage.transfer_bytes > 2_000_000
    assert live.MAX_STAGE_TRANSFER_BYTES == live.MAX_PHYSICAL_REQUESTS * (
        live.MAX_RESPONSE_BYTES + live.MAX_REQUEST_MATERIAL_BYTES
    )
    final_receipts = [json.loads(path.read_bytes()) for path in result.receipt_directory.glob("http-*-final.json")]
    assert len(final_receipts) == result.stage.physical_request_count
    assert all(row["response_bytes"] <= live.MAX_RESPONSE_BYTES for row in final_receipts)


@pytest.mark.asyncio
async def test_derived_stage_transfer_limit_still_rejects_true_overflow(tmp_path):
    sink = live.PrivateReceiptSink(tmp_path / "transfer-cap")
    inner = httpx.MockTransport(lambda request: httpx.Response(200, content=b"x" * 100, request=request))
    transport = live._BoundedTransport(inner, sink, {"R": {"github": 1}}, FakePacer())
    transport._transfer_bytes = live.MAX_STAGE_TRANSFER_BYTES - 64
    op_token = offline._CURRENT_OPERATION.set("R")
    engine_token = offline._CURRENT_ENGINES.set(("github",))
    request = httpx.Request(
        "GET",
        "https://api.github.com/search/repositories?q=test",
        extensions={"timeout": {"connect": 10.0, "read": 10.0, "write": 10.0, "pool": 10.0}},
    )
    try:
        with pytest.raises(httpx.ReadError):
            await transport.handle_async_request(request)
    finally:
        offline._CURRENT_ENGINES.reset(engine_token)
        offline._CURRENT_OPERATION.reset(op_token)
        await transport.aclose()
    final = json.loads((sink.path / "http-0001-final.json").read_bytes())
    assert final["failure_code"] == "stage_transfer_byte_cap"
    assert final["dispatched"] is True


@pytest.mark.asyncio
async def test_bad_external_receipt_stops_before_verifier_lease_sink_or_transport(tmp_path):
    manifest, plan = make_manifest()
    calls = []
    verifier, lease = FakePermitVerifier(), OneShot()
    with pytest.raises(live.LiveAcquisitionError, match="permit-receipt-digest-mismatch"):
        await live.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan,
            permit_receipt_bytes=b"wrong",
            expected_permit_receipt_sha256="a" * 64,
            permit_verifier=verifier,
            one_shot_lease=lease,
            receipt_directory=tmp_path / "should-not-exist",
            test_transport=httpx.MockTransport(_mock_handler(calls=calls)),
            pacer=FakePacer(),
        )
    assert verifier.calls == lease.calls == 0
    assert calls == []
    assert not (tmp_path / "should-not-exist").exists()


@pytest.mark.asyncio
async def test_production_path_rejects_custom_pacer_before_consuming_lease(tmp_path):
    manifest, plan = make_manifest()
    lease = OneShot()
    receipt = b"sealed fixture"
    with pytest.raises(live.LiveAcquisitionError, match="pacer-injection-requires-mock-transport"):
        await live.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan,
            permit_receipt_bytes=receipt,
            expected_permit_receipt_sha256=sha(receipt),
            permit_verifier=FakePermitVerifier(),
            one_shot_lease=lease,
            receipt_directory=tmp_path / "no-custom-production-pacer",
            pacer=FakePacer(),
        )
    assert lease.calls == 0
    assert not (tmp_path / "no-custom-production-pacer").exists()


def test_transport_operation_ledger_rejects_duplicate_invocation(tmp_path):
    sink = live.PrivateReceiptSink(tmp_path / "duplicate-operation")
    transport = live._BoundedTransport(
        httpx.MockTransport(_mock_handler()),
        sink,
        {"R01": {"github": 1}},
        FakePacer(),
        monotonic=FakePacer().monotonic,
        expected_operation_ids=("R01",),
    )
    transport.begin_operation("R01")
    with pytest.raises(live.LiveAcquisitionError, match="acquisition-operation-not-one-shot"):
        transport.begin_operation("R01")


@pytest.mark.asyncio
async def test_permit_binding_drift_stops_before_lease_and_dispatch(tmp_path):
    class DriftVerifier(FakePermitVerifier):
        def verify(self, manifest, manifest_sha256, receipt_bytes, expected_receipt_sha256):
            permit = super().verify(manifest, manifest_sha256, receipt_bytes, expected_receipt_sha256)
            return live.VerifiedAcquisitionPermit(**{**permit.__dict__, "protocol_sha256": "f" * 64})

    manifest, plan = make_manifest()
    lease, calls = OneShot(), []
    receipt = b"sealed fixture"
    with pytest.raises(live.LiveAcquisitionError, match="external-permit-binding-mismatch"):
        await live.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan,
            permit_receipt_bytes=receipt,
            expected_permit_receipt_sha256=sha(receipt),
            permit_verifier=DriftVerifier(),
            one_shot_lease=lease,
            receipt_directory=tmp_path / "should-not-exist",
            test_transport=httpx.MockTransport(_mock_handler(calls=calls)),
            pacer=FakePacer(),
        )
    assert lease.calls == 0
    assert calls == []
    assert not (tmp_path / "should-not-exist").exists()


@pytest.mark.asyncio
async def test_incomplete_pool_failure_is_terminal_and_remaining_slots_are_not_invoked(tmp_path):
    calls = []
    result, _verifier, _lease, _pacer, _ = await _run(tmp_path, handler=_mock_handler(status=503, calls=calls))
    assert result.stage.status == "inconclusive"
    assert result.stage.operations[0].status == "inconclusive"
    assert all(op.status == "not_invoked_after_terminal_failure" for op in result.stage.operations[1:])
    assert len(calls) <= 2
    assert result.stage.physical_request_count == len(calls)


@pytest.mark.asyncio
async def test_oversized_response_is_retained_as_failed_without_retry(tmp_path):
    calls = []
    result, _verifier, _lease, _pacer, _ = await _run(tmp_path, handler=_mock_handler(oversized=True, calls=calls))
    assert result.stage.status == "inconclusive"
    # The shared search fan-out can already have dispatched every engine in
    # the current operation when its first oversized response is observed;
    # the exact study then terminates and no later operation is invoked.
    assert len(calls) == 2
    final = json.loads((result.receipt_directory / "http-0001-final.json").read_bytes())
    assert final["failure_code"] == "response_body_cap"
    assert final["dispatched"] is True
    assert final["response_bytes"] > live.MAX_RESPONSE_BYTES
    assert all(op.status == "not_invoked_after_terminal_failure" for op in result.stage.operations[1:])


@pytest.mark.asyncio
async def test_pool_snapshot_write_failure_is_terminal_before_next_operation(tmp_path, monkeypatch):
    original = live.PrivateReceiptSink.write_once

    def fail_pool_snapshot(self, name, value):
        if name.startswith("pool-") and name != "pool-snapshot-index.json":
            raise live.LiveAcquisitionError("synthetic snapshot disk failure")
        return original(self, name, value)

    monkeypatch.setattr(live.PrivateReceiptSink, "write_once", fail_pool_snapshot)
    result, _verifier, _lease, _pacer, _calls = await _run(tmp_path)
    assert result.stage.status == "inconclusive"
    assert result.stage.operations[0].status == "pool_snapshot_write_failure"
    assert all(op.status == "not_invoked_after_terminal_failure" for op in result.stage.operations[1:])
    index = json.loads((result.receipt_directory / "pool-snapshot-index.json").read_bytes())
    assert index["status"] == "incomplete"
    assert index["operations"][0]["status"] == "pool_snapshot_write_failure"
    with pytest.raises(live.LiveAcquisitionError, match="pool-snapshot-index-incomplete"):
        live.verify_pool_snapshot_index(
            result.receipt_directory,
            expected_index_sha256=result.pool_snapshot_index_sha256,
            expected_stage_manifest_sha256=result.stage_manifest_sha256,
            expected_stage_uuid=make_manifest()[0]["stage_uuid"],
            expected_source_revision="a" * 40,
        )


@pytest.mark.asyncio
async def test_snapshot_verifier_stops_reading_when_deadline_expires(tmp_path, monkeypatch):
    result, _verifier, _lease, _pacer, _calls = await _run(tmp_path)
    clock = [0.0]
    monkeypatch.setattr(live.time, "monotonic", lambda: clock[0])
    original = live._read_canonical_artifact
    reads = []

    def expire_after_first_snapshot(path, *, max_bytes, check_deadline=None):
        value = original(path, max_bytes=max_bytes, check_deadline=check_deadline)
        reads.append(path.name)
        if path.name != "pool-snapshot-index.json" and len(reads) == 2:
            clock[0] = 10.0
        return value

    monkeypatch.setattr(live, "_read_canonical_artifact", expire_after_first_snapshot)
    with pytest.raises(live.LiveAcquisitionError, match="pool-snapshot-deadline-exceeded"):
        live.verify_pool_snapshot_index(
            result.receipt_directory,
            expected_index_sha256=result.pool_snapshot_index_sha256,
            expected_stage_manifest_sha256=result.stage_manifest_sha256,
            expected_stage_uuid=make_manifest()[0]["stage_uuid"],
            expected_source_revision="b" * 40,
            deadline_monotonic=10.0,
        )
    assert reads[0] == "pool-snapshot-index.json"
    assert len(reads) == 2
    assert reads[1].startswith("pool-")


@pytest.mark.asyncio
async def test_source_scope_drift_and_plan_scope_drift_fail_before_lease(tmp_path):
    manifest, plan = make_manifest()
    manifest["research_cases"][0]["pool_plan"]["engines"] = ["github", "brave"]
    lease, calls = OneShot(), []
    receipt = b"sealed fixture"
    with pytest.raises(ValueError):
        await live.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan,
            permit_receipt_bytes=receipt,
            expected_permit_receipt_sha256=sha(receipt),
            permit_verifier=FakePermitVerifier(),
            one_shot_lease=lease,
            receipt_directory=tmp_path / "invalid-scope",
            test_transport=httpx.MockTransport(_mock_handler(calls=calls)),
            pacer=FakePacer(),
        )
    assert lease.calls == 0 and calls == []
    assert not (tmp_path / "invalid-scope").exists()


@pytest.mark.asyncio
async def test_external_lease_failure_makes_no_provider_request(tmp_path):
    class FailedLease:
        def __init__(self):
            self.calls = 0

        def consume_once(self, permit):
            self.calls += 1
            raise RuntimeError("synthetic lease refusal")

    manifest, plan = make_manifest()
    lease, calls = FailedLease(), []
    receipt = b"sealed fixture"
    with pytest.raises(RuntimeError, match="synthetic lease refusal"):
        await live.acquire_live_coverage_stage(
            stage_manifest=manifest,
            acquisition_plan_bytes=plan,
            permit_receipt_bytes=receipt,
            expected_permit_receipt_sha256=sha(receipt),
            permit_verifier=FakePermitVerifier(),
            one_shot_lease=lease,
            receipt_directory=tmp_path / "lease-failed",
            test_transport=httpx.MockTransport(_mock_handler(calls=calls)),
            pacer=FakePacer(),
        )
    assert lease.calls == 1 and calls == []
    assert list((tmp_path / "lease-failed").iterdir()) == []


@pytest.mark.asyncio
async def test_public_offline_entrypoint_cannot_accept_live_transport(tmp_path):
    manifest, plan = make_manifest()
    sink = live.PrivateReceiptSink(tmp_path / "sink")
    bounded = live._BoundedTransport(httpx.MockTransport(_mock_handler()), sink, {}, FakePacer())
    with pytest.raises(TypeError, match="RecordedMockTransport"):
        await offline.acquire_coverage_stage(manifest, {}, bounded, FakePacer())
