"""Mock-only contract tests for the separately collected frozen search pools."""

from __future__ import annotations

import hashlib
import json
import uuid
from dataclasses import replace
from datetime import datetime, timedelta, timezone
from urllib.parse import parse_qs

import httpx
import pytest

from scripts import coverage_frozen_pool_collect as collector
from scripts import coverage_study_acquire as offline
from scripts.coverage_study_core import ACQUISITION_SCHEMA


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    ).encode()


class _PermitVerifier:
    def verify_collection(
        self,
        manifest,
        manifest_sha256,
        pool_input_mode,
        stage_clock_sha256,
        candidate_identity_sha256,
        candidate_endpoint_sha256,
        receipt_bytes,
        expected_receipt_sha256,
    ):
        return collector.VerifiedFrozenCollectionPermit(
            status="verified",
            scope="frozen-pool-collection",
            stage_uuid=manifest["stage_uuid"],
            stage_kind=manifest["stage"],
            source_revision=manifest["source_revision"],
            source_closure_sha256=manifest["source_closure_sha256"],
            protocol_sha256=manifest["protocol_sha256"],
            cohorts_sha256=manifest["cohorts_sha256"],
            input_manifest_sha256=manifest["input_manifest_sha256"],
            acquisition_plan_sha256=manifest["acquisition_plan_sha256"],
            legacy_manifest_sha256=manifest_sha256,
            pool_input_mode=pool_input_mode,
            stage_clock_sha256=stage_clock_sha256,
            candidate_identity_sha256=candidate_identity_sha256,
            candidate_endpoint_sha256=candidate_endpoint_sha256,
            receipt_sha256=expected_receipt_sha256,
        )


class _Lease:
    def consume_once(self, permit):
        return {"status": "consumed", "stage_uuid": permit.stage_uuid, "receipt_sha256": "a" * 64}


class _Pacer:
    def __init__(self):
        self.elapsed = 0.0
        self.sleeps = []

    async def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.elapsed += seconds

    def monotonic(self):
        return self.elapsed


def _manifest():
    stage_uuid = str(uuid.UUID(int=71357))
    low = ["github", "openalex"]
    high = ["arxiv", "github", "openalex", "wikipedia"]
    research = []
    plans = []
    for i in range(1, 9):
        engines = low if i <= 4 else high
        band = "research_le_40" if i <= 4 else "research_41_80"
        research.append(
            {
                "task_id": f"R{i:02}",
                "search_query": f"collector topic {i}",
                "pool_plan": {
                    "engines": engines,
                    "searx_categories": ["reference"],
                    "engine_configs": {engine: {"max_results": 20} for engine in engines},
                    "band": band,
                },
            }
        )
        plans.append(
            {
                "task_id": f"R{i:02}",
                "engine_calls": {
                    "arxiv": 2 if "arxiv" in engines else 0,
                    "github": 1 if "github" in engines else 0,
                    "openalex": 1 if "openalex" in engines else 0,
                    "wikipedia": 2 if "wikipedia" in engines else 0,
                },
            }
        )
    navigation = []
    for i in range(1, 6):
        navigation.append(
            {
                "target_id": f"N{i:02}",
                "search_query": f"navigation target {i}",
                "target_url": f"https://github.com/fixture/nav-{i}",
                "navigation_pool_plan": {
                    "engines": ["github"],
                    "searx_categories": ["reference"],
                    "engine_configs": {"github": {"max_results": 20}},
                },
            }
        )
        plans.append(
            {
                "task_id": f"N{i:02}",
                "engine_calls": {
                    "arxiv": 0,
                    "github": 1,
                    "openalex": 0,
                    "wikipedia": 0,
                },
            }
        )
    plan = _canonical({"schema": ACQUISITION_SCHEMA, "stage_uuid": stage_uuid, "tasks": plans})
    manifest = {
        "schema": offline.CONTINUE_ACQUISITION_PROFILE.manifest_schema,
        "execution_profile": offline.CONTINUE_ACQUISITION_PROFILE.identifier,
        "stage": "development",
        "stage_uuid": stage_uuid,
        "source_revision": "b" * 40,
        "source_closure_sha256": "1" * 64,
        "protocol_sha256": "2" * 64,
        "cohorts_sha256": "3" * 64,
        "input_manifest_sha256": "4" * 64,
        "acquisition_plan_sha256": _sha(plan),
        "research_cases": research,
        "navigation_targets": navigation,
    }
    return manifest, plan


def _handler(calls, *, rate_limit_openalex=False):
    def handle(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        query = parse_qs(request.url.query.decode())
        if request.url.host == "export.arxiv.org":
            body = (
                '<feed xmlns="http://www.w3.org/2005/Atom">'
                + "".join(
                    f"<entry><id>https://arxiv.org/abs/2601.{i:05}</id>"
                    f"<title>Paper {i}</title><summary>Evidence</summary>"
                    "<published>2026-01-01T00:00:00Z</published></entry>"
                    for i in range(1, 3)
                )
                + "</feed>"
            )
            return httpx.Response(200, content=body.encode(), request=request)
        if request.url.host == "api.github.com":
            term = query.get("q", [""])[0]
            if term.startswith("navigation target"):
                number = term.rsplit(" ", 1)[-1]
                items = [
                    {
                        "full_name": f"fixture/nav-{number}",
                        "html_url": f"https://github.com/fixture/nav-{number}",
                        "description": "navigation target",
                        "stargazers_count": 1,
                        "language": "Python",
                    }
                ]
            else:
                safe = term.replace(" ", "-")
                items = [
                    {
                        "full_name": f"fixture/{safe}-{i}",
                        "html_url": f"https://github.com/fixture/{safe}-{i}",
                        "description": f"repository {i}",
                        "stargazers_count": i,
                        "language": "Python",
                    }
                    for i in range(20)
                ]
            return httpx.Response(200, json={"total_count": len(items), "items": items}, request=request)
        if request.url.host == "api.openalex.org":
            if rate_limit_openalex and query.get("search", [""])[0] == "collector topic 1":
                return httpx.Response(429, content=b"rate limited", request=request)
            term = query.get("search", [""])[0]
            items = [
                {
                    "id": f"https://openalex.org/W{i}",
                    "doi": f"https://doi.org/10.1234/{i}",
                    "title": f"OpenAlex {term} {i}",
                    "publication_date": "2025-01-01",
                    "relevance_score": 20 - i / 100,
                    "abstract_inverted_index": None,
                }
                for i in range(20)
            ]
            return httpx.Response(200, json={"results": items}, request=request)
        if request.url.host == "en.wikipedia.org":
            if query.get("action") == ["opensearch"]:
                term = query.get("search", [""])[0]
                titles = [f"{term} topic {i}" for i in range(20)]
                return httpx.Response(
                    200, json=[term, titles, ["https://en.wikipedia.org/wiki/X"] * 20, [""] * 20], request=request
                )
            data = {
                "query": {
                    "pages": {
                        str(i): {"pageid": i, "title": f"Page {i}", "extract": f"Evidence {i}"} for i in range(20)
                    }
                }
            }
            return httpx.Response(200, json=data, request=request)
        raise AssertionError("unexpected fixture host")

    return handle


def _clock(pacer, *, elapsed=0.0):
    pacer.elapsed = elapsed
    start = datetime.now(timezone.utc) - timedelta(seconds=elapsed)
    deadline = start + timedelta(hours=8)
    return collector.FrozenCollectionStageClock(
        0.0,
        28_800.0,
        start.isoformat().replace("+00:00", "Z"),
        deadline.isoformat().replace("+00:00", "Z"),
        _sha(b"test-only-clock-origin"),
    )


async def _collect(
    tmp_path,
    *,
    rate_limit_openalex=False,
    elapsed=0.0,
    permit_verifier=None,
    transport_handler=None,
):
    manifest, plan = _manifest()
    stage_bytes = _canonical(manifest)
    pacer = _Pacer()
    calls = []
    transport = httpx.MockTransport(transport_handler or _handler(calls, rate_limit_openalex=rate_limit_openalex))
    inventory_root = tmp_path / "inventory"
    inventory_root.mkdir(mode=0o700)
    root = inventory_root / manifest["stage_uuid"]
    root.mkdir(mode=0o700)
    result = await collector.collect_frozen_pool_stage(
        stage_manifest_bytes=stage_bytes,
        acquisition_plan_bytes=plan,
        stage_clock=_clock(pacer, elapsed=elapsed),
        candidate_identity_sha256="5" * 64,
        candidate_endpoint_sha256="6" * 64,
        permit_receipt_bytes=b"mock only",
        expected_permit_receipt_sha256=_sha(b"mock only"),
        permit_verifier=permit_verifier or _PermitVerifier(),
        one_shot_lease=_Lease(),
        stage_inventory_directory=root,
        test_transport=transport,
        pacer=pacer,
    )
    return result, calls, pacer


@pytest.mark.asyncio
async def test_finalization_reserve_seals_all_uninvoked_slots_without_dispatch(tmp_path):
    result, calls, _pacer = await _collect(tmp_path, elapsed=28_500.0)
    assert result.status == "inconclusive"
    assert calls == []
    summary = json.loads((result.collection_directory / "collection-summary.json").read_bytes())
    assert len(summary["fatal_uninvoked_slots"]) == 13
    assert all(row["status"] == "not_invoked_after_fatal_failure" for row in summary["operation_statuses"])


@pytest.mark.asyncio
async def test_permit_stage_kind_mismatch_fails_before_any_dispatch(tmp_path):
    class WrongStageVerifier(_PermitVerifier):
        def verify_collection(self, *args, **kwargs):
            permit = super().verify_collection(*args, **kwargs)
            return replace(permit, stage_kind="confirmation")

    manifest, _plan = _manifest()
    with pytest.raises(collector.FrozenCollectionError, match="external-permit-binding-mismatch"):
        await _collect(tmp_path, permit_verifier=WrongStageVerifier())
    assert not list((tmp_path / "inventory" / manifest["stage_uuid"]).glob("collection"))


@pytest.mark.asyncio
async def test_oversized_response_is_retained_as_incomplete_and_collection_is_ineligible(tmp_path):
    calls = []
    fallback = _handler([])

    def oversized(request):
        calls.append(request)
        if len(calls) == 1:
            return httpx.Response(
                200,
                content=b"x" * (collector.MAX_RESPONSE_BYTES + 32),
                request=request,
            )
        return fallback(request)

    result, _observed, _pacer = await _collect(tmp_path, transport_handler=oversized)
    assert result.status == "inconclusive"
    # The two engines of R01 can already be dispatched concurrently; the
    # oversized response prevents every later logical operation from starting.
    assert len(calls) == 2
    root = result.collection_directory
    ledger = json.loads((root / "request-ledger-index.json").read_bytes())
    assert len(ledger["entries"]) == 2
    assert {entry["operation_id"] for entry in ledger["entries"]} == {"R01"}
    entry = next(row for row in ledger["entries"] if row["raw_response_overflow_witness"])
    assert entry["raw_response_complete"] is False
    assert entry["raw_response_overflow_witness"] is True
    final = json.loads((root / entry["final_file"]).read_bytes())
    assert final["failure_code"] == "response_body_cap"
    assert final["complete"] is False
    summary = json.loads((root / "collection-summary.json").read_bytes())
    assert summary["status"] == "inconclusive"
    assert len(summary["fatal_uninvoked_slots"]) == 12


@pytest.mark.asyncio
async def test_complete_natural_pools_pass_the_source_pinned_corpus_verifier(tmp_path):
    result, calls, pacer = await _collect(tmp_path)
    assert result.status == "complete"
    assert len(calls) == 33
    assert len(json.loads((result.collection_directory / "request-ledger-index.json").read_bytes())["entries"]) == len(
        calls
    )
    assert len(pacer.sleeps) == 12
    root = result.collection_directory
    assert root.stat().st_mode & 0o777 == 0o700
    for filename in (
        "admission.json",
        "transport-control.json",
        "request-ledger-index.json",
        "collection-summary.json",
        "acquisition-manifest.json",
        "frozen-pool-index.json",
        "engine-availability.json",
        "frozen-pool-evidence.json",
        "collector-receipt.json",
    ):
        assert (root / filename).stat().st_mode & 0o777 == 0o600
    summary = json.loads((root / "collection-summary.json").read_bytes())
    assert len(summary["operation_statuses"]) == 13
    assert {row["status"] for row in summary["operation_statuses"]} == {"complete"}
    assert {row["pool_status"] for row in summary["pool_eligibility"]} == {"complete"}


@pytest.mark.asyncio
async def test_real_429_cooldown_preserves_partial_availability_and_pool_eligibility(tmp_path):
    result, calls, _pacer = await _collect(tmp_path, rate_limit_openalex=True)
    assert result.status == "complete"
    openalex_calls = [request for request in calls if request.url.host == "api.openalex.org"]
    assert len(openalex_calls) == 1
    assert parse_qs(openalex_calls[0].url.query.decode())["search"] == ["collector topic 1"]
    root = result.collection_directory
    summary = json.loads((root / "collection-summary.json").read_bytes())
    r1 = next(row for row in summary["operation_statuses"] if row["operation_id"] == "R01")
    r2 = next(row for row in summary["operation_statuses"] if row["operation_id"] == "R02")
    assert r1["status"] == "inconclusive"
    assert r2["status"] == "inconclusive"
    assert any("openalex" in reason for reason in r2["failure_reasons"])
    index = json.loads((root / "frozen-pool-index.json").read_bytes())
    r2row = next(row for row in index["operations"] if row["operation_id"] == "R02")
    artifact = json.loads((root / r2row["file"]).read_bytes())
    refs = artifact["operation"]["cooldown_source_references"]
    assert refs == [{"engine": "openalex", "request_sequence": 2, "observed_status": 429}]
    assert r2row["pool_status"] == "complete"
    ledger = json.loads((root / "request-ledger-index.json").read_bytes())
    openalex_after = [
        entry for entry in ledger["entries"] if entry["operation_id"] == "R02" and entry["engine"] == "openalex"
    ]
    assert openalex_after == []
