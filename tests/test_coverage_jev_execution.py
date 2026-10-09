"""Synthetic, no-provider tests for the guarded Jev execution seam."""

from __future__ import annotations

import asyncio
import hashlib
import json
import stat
import tempfile
import time
from dataclasses import asdict
from pathlib import Path

import httpx
import pytest

from scripts import coverage_jev_execution as execution
from scripts import coverage_study_core as core
from scripts import intent_ranking_coverage as coverage
from scripts import intent_ranking_receipts as receipts
from tests.test_coverage_study_core import make_fixture


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def prepared_stage() -> core.PreparedStage:
    args, *_ = make_fixture()
    return core.preflight_stage(**args)


def candidate_input() -> bytes:
    return canonical(
        {
            "schema": "coverage-jev-operation-input/1",
            "query": "synthetic query",
            "purpose": "synthetic purpose",
            "facets": [
                {"id": "facet-a", "description": "synthetic facet"},
                {"id": "facet-b", "description": "another synthetic facet"},
            ],
            "candidates": [
                {"id": "c0", "title": "One", "url": "https://example.test/1", "snippet": "first"},
                {"id": "c1", "title": "Two", "url": "https://example.test/2", "snippet": "second"},
            ],
            "incumbent_order": ["c0", "c1"],
        }
    )


def compile_small() -> coverage.CompiledRequest:
    _body, compiled = execution.build_selector_request(
        operation_id="synthetic-candidate", operation_input_bytes=candidate_input()
    )
    assert compiled is not None
    return compiled


def valid_response(compiled: coverage.CompiledRequest, *, input_tokens: int = 10, output_tokens: int = 5) -> bytes:
    answers = {}
    legend = {str(index): value for index, value in enumerate(coverage.LEVELS)}
    for question_id in compiled.question_ids:
        if question_id.startswith("score:"):
            probabilities = {str(index): float(index == 0) for index in range(10)}
            answers[question_id] = {
                "type": "score",
                "score": 0,
                "legend": legend,
                "probabilities": probabilities,
                "confidence": 0.5,
            }
        else:
            options = compiled.profile_options[question_id]
            probabilities = {option: float(option == "none") for option in options}
            answers[question_id] = {
                "type": "choice",
                "choice": "none",
                "probabilities": probabilities,
                "confidence": 0.5,
            }
    return json.dumps(
        {
            "model": coverage.MODEL,
            "answers": answers,
            "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
        },
        separators=(",", ":"),
    ).encode()


class PrivateRoots:
    def __init__(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="coverage-jev-test-")
        self.root = Path(self.temp.name)
        self.lease = self.make("lease")
        self.archive = self.make("archive")
        self.result = self.make("result")

    def make(self, name: str) -> Path:
        path = self.root / name
        path.mkdir(mode=0o700)
        path.chmod(0o700)
        return path

    def close(self) -> None:
        self.temp.cleanup()


class PartialStream(httpx.AsyncByteStream):
    async def __aiter__(self):
        yield b'{"partial":'
        raise httpx.ReadError("synthetic interrupted response")

    async def aclose(self) -> None:
        return None


def permit_for(
    prepared: core.PreparedStage,
    operation_id: str,
    operation_input: bytes,
    request: bytes,
    *,
    parser_mode: str = "coverage",
) -> tuple[bytes, str]:
    body = canonical(
        {
            "schema": execution.PERMIT_SCHEMA,
            "status": "verified-admitted",
            "stage_uuid": prepared.stage_uuid,
            "operation_id": operation_id,
            "parser_mode": parser_mode,
            "source_revision": prepared.source_revision,
            "protocol_sha256": prepared.pins["protocol"],
            "source_closure_sha256": prepared.pins["qualified_source_closure"],
            "operation_input_sha256": sha(operation_input),
            "request_body_sha256": sha(request),
        }
    )
    return body, sha(body)


def invoke_kwargs(prepared, ledger, roots, compiled, operation_id, response, transport):
    operation_input = candidate_input()
    request, _compiled = execution.build_selector_request(
        operation_id=operation_id, operation_input_bytes=operation_input
    )
    permit, permit_sha = permit_for(prepared, operation_id, operation_input, request)
    return {
        "prepared": prepared,
        "ledger": ledger,
        "operation_id": operation_id,
        "operation_input_bytes": operation_input,
        "permit_bytes": permit,
        "expected_permit_sha256": permit_sha,
        "api_key": "Synthetic-Key-Do-Not-Persist-9283",
        "lease_root": roots.lease,
        "archive_root": roots.archive,
        "result_root": roots.result,
        "transport": transport,
    }


def v1_request(query, pool) -> bytes:
    from slopsearx.rerank import INSTRUCTIONS, LEVELS, MODEL, RerankCandidate
    from slopsearx.service import _rerank_text, _rerank_url

    candidates = tuple(
        RerankCandidate(
            id=f"c{index}",
            title=_rerank_text(result.title, 256),
            url=_rerank_url(result.url),
            snippet=_rerank_text(result.content, 1200),
        )
        for index, result in enumerate(pool[:40])
    )
    body = {
        "model": MODEL,
        "state": {"query": query, "candidates": [asdict(candidate) for candidate in candidates]},
        "questions": {
            candidate.id: {
                "type": "score",
                "instructions": INSTRUCTIONS.format(id=candidate.id),
                "criteria": LEVELS,
            }
            for candidate in candidates
        },
    }
    return json.dumps(body, ensure_ascii=False).encode()


def w0_input(query, pool) -> bytes:
    from slopsearx.service import _rerank_text, _rerank_url

    return canonical(
        {
            "schema": "coverage-jev-operation-input/1",
            "query": query,
            "candidates": [
                {
                    "id": f"c{index}",
                    "title": _rerank_text(result.title, 256),
                    "url": _rerank_url(result.url),
                    "snippet": _rerank_text(result.content, 1200),
                }
                for index, result in enumerate(pool[:40])
            ],
        }
    )


def invoke_w0_kwargs(prepared, ledger, roots, operation_id, transport):
    from tests.test_coverage_legacy_control import _pool, _service

    query = "synthetic offline W0 test"
    pool = _pool(4)
    operation_input = w0_input(query, pool)
    request, _compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=operation_input,
        legacy_control={
            "service": _service(),
            "query": query,
            "canonical_pool": pool,
            "frozen_v1_source_bytes": (
                Path(__file__).resolve().parents[1]
                / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
            ).read_bytes(),
            "current_service_source_bytes": (Path(__file__).resolve().parents[1] / "slopsearx/service.py").read_bytes(),
        },
    )
    permit, permit_sha = permit_for(prepared, operation_id, operation_input, request, parser_mode="original-v1")
    return {
        "prepared": prepared,
        "ledger": ledger,
        "operation_id": operation_id,
        "operation_input_bytes": operation_input,
        "permit_bytes": permit,
        "expected_permit_sha256": permit_sha,
        "api_key": "Synthetic-Key-Do-Not-Persist-9283",
        "lease_root": roots.lease,
        "archive_root": roots.archive,
        "result_root": roots.result,
        "transport": transport,
        "legacy_control": {
            "service": _service(),
            "query": query,
            "canonical_pool": pool,
            "frozen_v1_source_bytes": (
                Path(__file__).resolve().parents[1]
                / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
            ).read_bytes(),
            "current_service_source_bytes": (Path(__file__).resolve().parents[1] / "slopsearx/service.py").read_bytes(),
        },
    }


@pytest.mark.asyncio
async def test_success_archives_before_parse_and_failure_closes_full_inventory() -> None:
    from tests.test_coverage_legacy_control import _pool, _response, _service

    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    compiled = compile_small()
    seen = []
    legacy_query = "synthetic offline control"
    legacy_pool = _pool(4)
    legacy_body = v1_request(legacy_query, legacy_pool)
    response_body = _response({"c0": 7, "c1": 8, "c2": 6, "c3": 9})
    legacy_input = w0_input(legacy_query, legacy_pool)
    permit, permit_sha = permit_for(
        prepared, prepared.operation_ids[0], legacy_input, legacy_body, parser_mode="original-v1"
    )

    async def okay(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        assert str(request.url) == execution.JEV_SYSTEMONE_URL
        assert request.method == "POST"
        assert request.headers["authorization"] == "Bearer Synthetic-Key-Do-Not-Persist-9283"
        assert request.content == legacy_body
        assert request.extensions.get("timeout", {}).get("connect") == execution.SELECTOR_PHASE_SECONDS
        return httpx.Response(200, content=response_body, request=request)

    result = await execution.execute_selector_call(
        prepared=prepared,
        ledger=ledger,
        operation_id=prepared.operation_ids[0],
        operation_input_bytes=legacy_input,
        permit_bytes=permit,
        expected_permit_sha256=permit_sha,
        api_key="Synthetic-Key-Do-Not-Persist-9283",
        lease_root=roots.lease,
        archive_root=roots.archive,
        result_root=roots.result,
        transport=httpx.MockTransport(okay),
        legacy_control={
            "service": _service(),
            "query": legacy_query,
            "canonical_pool": legacy_pool,
            "frozen_v1_source_bytes": (
                Path(__file__).resolve().parents[1]
                / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
            ).read_bytes(),
            "current_service_source_bytes": (Path(__file__).resolve().parents[1] / "slopsearx/service.py").read_bytes(),
        },
    )
    assert result.status == "complete-original-v1"
    assert result.ranking is None
    assert result.native_status == "applied"
    assert result.native_ordered_ids == ("c3", "c1", "c0", "c2")
    assert result.native_strict_json_compatible is True
    assert result.native_strict_json_reason == "strict_current_parser_accepts"
    assert len(seen) == 1
    assert stat.S_IMODE(roots.archive.stat().st_mode) == 0o700
    claim_path = roots.lease / f"{prepared.stage_uuid}.{prepared.operation_ids[0]}.claim.json"
    assert stat.S_IMODE(claim_path.stat().st_mode) == 0o600
    files = b"".join(path.read_bytes() for path in roots.root.rglob("*") if path.is_file())
    assert b"Synthetic-Key-Do-Not-Persist-9283" not in files
    candidate_replay_roots = PrivateRoots()
    candidate_operation = prepared.operation_ids[1]
    candidate_inputs = candidate_input()
    candidate_request, _compiled = execution.build_selector_request(
        operation_id=candidate_operation, operation_input_bytes=candidate_inputs
    )
    candidate_permit, candidate_permit_sha = permit_for(
        prepared, candidate_operation, candidate_inputs, candidate_request
    )
    replay_bindings = {
        "stage_uuid": prepared.stage_uuid,
        "operation_id": candidate_operation,
        "request_body_sha256": sha(candidate_request),
        "source_revision": prepared.source_revision,
    }
    replay_receipt = receipts.archive_response(
        candidate_replay_roots.archive,
        bindings=replay_bindings,
        complete=True,
        status="complete",
        http_status=200,
        response_body=valid_response(compiled),
    )
    replayed = execution.replay_candidate_response(
        archive_root=candidate_replay_roots.archive,
        prepared=prepared,
        operation_input_bytes=candidate_inputs,
        permit_bytes=candidate_permit,
        expected_permit_sha256=candidate_permit_sha,
        operation_id=candidate_operation,
        bindings=replay_bindings,
        receipt_sha256=replay_receipt["receipt_sha256"],
        compiled_request=compiled,
    )
    assert replayed.reason == "missing_facet_lead"
    candidate_replay_roots.close()

    async def fail(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, stream=PartialStream(), request=request)

    second = prepared.operation_ids[1]
    second_result = await execution.execute_selector_call(
        **invoke_kwargs(prepared, ledger, roots, compiled, second, b"", httpx.MockTransport(fail))
    )
    assert second_result.status == "transport-error"
    assert second_result.archive_receipt_sha256 is not None
    partial_path = roots.archive / prepared.stage_uuid / second / "response.bin"
    assert partial_path.read_bytes() == b'{"partial":'
    with pytest.raises(receipts.ReceiptError, match="response-incomplete"):
        receipts.replay_response(
            roots.archive,
            expected_bindings={
                "stage_uuid": prepared.stage_uuid,
                "operation_id": second,
                "request_body_sha256": sha(compiled.body),
                "source_revision": prepared.source_revision,
            },
            expected_receipt_sha256=second_result.archive_receipt_sha256,
        )
    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    assert terminal["status"] == "terminal-failure"
    doc = json.loads((roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes())
    assert len(doc["operations"]) == len(prepared.operation_ids)
    assert doc["operations"][0]["state"] == "complete-success"
    assert doc["operations"][1]["state"] == "transport-or-http-failure"
    assert all(row["state"] == "not-invoked-after-terminal-stop" for row in doc["operations"][2:])
    roots.close()


@pytest.mark.asyncio
async def test_external_permit_mismatch_precedes_claim_and_dispatch() -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    operation_id = prepared.operation_ids[0]
    kwargs = invoke_w0_kwargs(
        prepared,
        ledger,
        roots,
        operation_id,
        httpx.MockTransport(lambda request: httpx.Response(200, content=b"{}", request=request)),
    )
    kwargs["expected_permit_sha256"] = "0" * 64
    with pytest.raises(execution.JevExecutionError, match="external-permit-pin-mismatch"):
        await execution.execute_selector_call(**kwargs)
    assert list(roots.lease.iterdir()) == []
    assert list(roots.archive.iterdir()) == []
    roots.close()


@pytest.mark.asyncio
async def test_frozen_v1_generator_drift_refuses_before_claim_or_dispatch(monkeypatch) -> None:
    from slopsearx import rerank

    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    calls = []
    kwargs = invoke_w0_kwargs(
        prepared,
        ledger,
        roots,
        prepared.operation_ids[0],
        httpx.MockTransport(
            lambda request: calls.append(request) or httpx.Response(200, content=b"{}", request=request)
        ),
    )
    monkeypatch.setattr(rerank, "LEVELS", tuple(rerank.LEVELS) + ("drift",))
    with pytest.raises(execution.JevExecutionError, match="frozen-v1-compiler-unavailable"):
        await execution.execute_selector_call(**kwargs)
    assert calls == []
    assert list(roots.lease.iterdir()) == []
    roots.close()


@pytest.mark.asyncio
async def test_actual_frozen_v1_generator_body_mismatch_refuses_before_dispatch(monkeypatch) -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    calls = []
    kwargs = invoke_w0_kwargs(
        prepared,
        ledger,
        roots,
        prepared.operation_ids[0],
        httpx.MockTransport(
            lambda request: calls.append(request) or httpx.Response(200, content=b"{}", request=request)
        ),
    )

    async def drift(**_kwargs):
        return b"deliberately-drifted-frozen-generator-body"

    monkeypatch.setattr(execution, "_capture_frozen_v1_generated_request", drift)
    with pytest.raises(execution.JevExecutionError, match="original-v1-generator-body-mismatch"):
        await execution.execute_selector_call(**kwargs)
    assert calls == []
    assert list(roots.lease.iterdir()) == []
    roots.close()


@pytest.mark.asyncio
async def test_original_v1_native_fallback_retains_full_order_and_diagnostics() -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    operation_id = prepared.operation_ids[0]
    kwargs = invoke_w0_kwargs(
        prepared,
        ledger,
        roots,
        operation_id,
        httpx.MockTransport(
            lambda request: httpx.Response(
                200,
                content=json.dumps(
                    {
                        "model": "jev-1.13.0",
                        "answers": {"c0": {"type": "score", "score": 1}},
                        "usage": {"input_tokens": 123, "output_tokens": 12},
                    },
                    separators=(",", ":"),
                ).encode(),
                request=request,
            )
        ),
    )
    result = await execution.execute_selector_call(**kwargs)
    assert result.status == "complete-invalid-response"
    assert result.native_status == "fallback"
    assert result.native_ordered_ids == ("c0", "c1", "c2", "c3")
    assert result.native_strict_json_compatible is False
    assert result.native_strict_json_reason == "candidate_id_set_mismatch"
    receipt_doc = json.loads((roots.result / f"{prepared.stage_uuid}.{operation_id}.result.json").read_bytes())
    assert receipt_doc["ordered_ids"] == ["c0", "c1", "c2", "c3"]
    assert receipt_doc["native_status"] == "fallback"
    assert receipt_doc["native_strict_json_reason"] == "candidate_id_set_mismatch"
    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    assert terminal["status"] == "terminal-failure"
    roots.close()


@pytest.mark.asyncio
async def test_reflected_explicit_api_key_is_never_archived() -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    operation_id = prepared.operation_ids[0]
    key = "Synthetic-Key-Do-Not-Persist-9283"
    kwargs = invoke_w0_kwargs(
        prepared,
        ledger,
        roots,
        operation_id,
        httpx.MockTransport(
            lambda request: httpx.Response(
                502,
                content=f"upstream error reflected {key}".encode(),
                request=request,
            )
        ),
    )
    result = await execution.execute_selector_call(**kwargs)
    assert result.status == "credential-reflection-suppressed"
    assert result.terminal_reason == "credential-reflection-suppressed"
    assert result.response_bytes == 0
    assert result.archive_receipt_sha256 is not None
    archived = (roots.archive / prepared.stage_uuid / operation_id / "response.bin").read_bytes()
    receipt = json.loads((roots.archive / prepared.stage_uuid / operation_id / "receipt.json").read_bytes())
    assert archived == b""
    assert receipt["complete"] is False
    assert receipt["status"] == "credential-reflection-suppressed"
    all_private = b"".join(path.read_bytes() for path in roots.root.rglob("*") if path.is_file())
    assert key.encode() not in all_private
    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    assert terminal["status"] == "terminal-failure"
    inventory = json.loads((roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes())
    assert inventory["operations"][0]["state"] == "partial-or-interrupted"
    roots.close()


@pytest.mark.asyncio
async def test_reused_operation_claim_blocks_second_transport_attempt() -> None:
    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    calls = 0

    async def transport(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        from tests.test_coverage_legacy_control import _response

        return httpx.Response(200, content=_response({f"c{i}": 4 for i in range(4)}), request=request)

    kwargs = invoke_w0_kwargs(prepared, ledger, roots, prepared.operation_ids[0], httpx.MockTransport(transport))
    first = await execution.execute_selector_call(**kwargs)
    assert first.archive_receipt_sha256
    with pytest.raises(execution.JevExecutionError, match="one-shot-artifact-already-exists"):
        await execution.execute_selector_call(**kwargs)
    assert calls == 1
    roots.close()


@pytest.mark.asyncio
async def test_phase_deadline_archives_partial_and_stops_without_retry(monkeypatch) -> None:
    from tests.test_coverage_legacy_control import _response

    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    monkeypatch.setattr(execution, "SELECTOR_PHASE_SECONDS", 0.02)
    calls = 0

    async def slow(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        await asyncio.sleep(0.1)
        return httpx.Response(200, content=_response({f"c{i}": 4 for i in range(4)}), request=request)

    result = await execution.execute_selector_call(
        **invoke_w0_kwargs(prepared, ledger, roots, prepared.operation_ids[0], httpx.MockTransport(slow))
    )
    assert result.terminal_reason == "selector-phase-deadline-exceeded"
    assert calls == 1
    assert result.archive_receipt_sha256
    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    assert terminal["status"] == "terminal-failure"
    roots.close()


@pytest.mark.asyncio
async def test_result_receipt_fsync_deadline_writes_terminal_correction(monkeypatch) -> None:
    from tests.test_coverage_legacy_control import _response

    prepared = prepared_stage()
    ledger = core.StudyRun(prepared)
    roots = PrivateRoots()
    monkeypatch.setattr(execution, "SELECTOR_PHASE_SECONDS", 0.02)
    original_write = execution._write_exclusive

    def delayed_result_write(root: Path, name: str, body: bytes) -> str:
        digest = original_write(root, name, body)
        if name.endswith(".result.json"):
            time.sleep(0.04)
        return digest

    monkeypatch.setattr(execution, "_write_exclusive", delayed_result_write)
    result = await execution.execute_selector_call(
        **invoke_w0_kwargs(
            prepared,
            ledger,
            roots,
            prepared.operation_ids[0],
            httpx.MockTransport(
                lambda request: httpx.Response(
                    200,
                    content=_response({f"c{i}": 4 for i in range(4)}),
                    request=request,
                )
            ),
        )
    )
    assert result.status == "phase-deadline-exceeded"
    assert result.terminal_reason == "selector-phase-deadline-exceeded"
    provisional = json.loads(
        (roots.result / f"{prepared.stage_uuid}.{prepared.operation_ids[0]}.result.json").read_bytes()
    )
    assert provisional["provisional"] is True
    assert provisional["authority"] == "provisional-only-terminal-inventory-controls-acceptance"
    correction = json.loads(
        (roots.result / f"{prepared.stage_uuid}.{prepared.operation_ids[0]}.correction.json").read_bytes()
    )
    assert correction["result_receipt_sha256"] == result.result_receipt_sha256
    assert correction["status"] == "phase-deadline-exceeded"
    assert correction["acceptance_authority"] == "terminal-inventory-only"
    terminal = execution.close_stage(prepared=prepared, ledger=ledger, result_root=roots.result)
    assert terminal["status"] == "terminal-failure"
    inventory = json.loads((roots.result / f"{prepared.stage_uuid}.terminal-inventory.json").read_bytes())
    assert inventory["operations"][0]["state"] == "partial-or-interrupted"
    roots.close()


@pytest.mark.asyncio
async def test_exact_original_v1_replay_path_uses_frozen_helper() -> None:
    from scripts.coverage_legacy_control import FROZEN_V1_SOURCE_SHA256
    from tests.test_coverage_legacy_control import _expected_request_sha, _pool, _response, _service

    root = Path(__file__).resolve().parents[1]
    frozen = (root / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt").read_bytes()
    assert sha(frozen) == FROZEN_V1_SOURCE_SHA256
    query = "synthetic offline W0 replay"
    pool = _pool(4)
    expected_request = _expected_request_sha(query, pool)
    prepared = prepared_stage()
    operation_id = prepared.operation_ids[0]
    operation_input = w0_input(query, pool)
    legacy_control = {
        "service": _service(),
        "query": query,
        "canonical_pool": pool,
        "frozen_v1_source_bytes": frozen,
        "current_service_source_bytes": (root / "slopsearx/service.py").read_bytes(),
    }
    built_request, _compiled = execution.build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=operation_input,
        legacy_control=legacy_control,
    )
    assert sha(built_request) == expected_request
    permit, permit_sha = permit_for(prepared, operation_id, operation_input, built_request, parser_mode="original-v1")
    response = _response({f"c{i}": 4 for i in range(4)})
    bindings = {
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "request_body_sha256": expected_request,
        "source_revision": prepared.source_revision,
    }
    with tempfile.TemporaryDirectory(prefix="coverage-v1-replay-") as temp:
        archive_root = Path(temp)
        archive_root.chmod(0o700)
        archived = receipts.archive_response(
            archive_root,
            bindings=bindings,
            complete=True,
            status="complete",
            http_status=200,
            response_body=response,
        )
        result, status, receipt = await execution.replay_original_v1_response(
            archive_root=archive_root,
            prepared=prepared,
            operation_input_bytes=operation_input,
            permit_bytes=permit,
            expected_permit_sha256=permit_sha,
            operation_id=operation_id,
            bindings=bindings,
            receipt_sha256=archived["receipt_sha256"],
            service=_service(),
            query=query,
            canonical_pool=pool,
            frozen_v1_source_bytes=frozen,
            current_service_source_bytes=(root / "slopsearx/service.py").read_bytes(),
            expected_v1_request_sha256=expected_request,
        )
    assert status == "applied"
    assert len(result) == len(pool)
    assert receipt.usage_state == "known"
