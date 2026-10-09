"""No-network tests for replaying the source-pinned V1 Jev control."""

from __future__ import annotations

import ast
import hashlib
import json
import sys
import types
from dataclasses import asdict
from pathlib import Path
from typing import Any

import pytest

from scripts.coverage_legacy_control import (
    FROZEN_V1_SOURCE_SHA256,
    SERVICE_METHODS_PARITY_AST_SHA256,
    LegacyReplayAdmissionError,
    LegacyReplayIntegrityError,
    _canonical_ast_dump,
    _method_fingerprint,
    _runtime_value,
    replay_legacy_v1_control,
)
from slopsearx.adapter import SearchResult
from slopsearx.rerank import INSTRUCTIONS, LEVELS, MAX_CANDIDATES, MODEL, RerankCandidate
from slopsearx.service import AppContext, SearchService, _rerank_text, _rerank_url


@pytest.fixture(scope="module")
def frozen_v1_source() -> bytes:
    source_path = (
        Path(__file__).resolve().parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
    )
    source_bytes = source_path.read_bytes()
    assert hashlib.sha256(source_bytes).hexdigest() == FROZEN_V1_SOURCE_SHA256
    return source_bytes


@pytest.fixture(scope="module")
def current_service_source() -> bytes:
    source_path = Path(__file__).resolve().parents[1] / "slopsearx/service.py"
    return source_path.read_bytes()


def test_ast_dump_includes_empty_fields_on_versions_that_support_show_empty(monkeypatch: pytest.MonkeyPatch) -> None:
    original_dump = ast.dump
    seen_show_empty: list[bool] = []

    def python_313_dump(
        node: ast.AST,
        annotate_fields: bool = True,
        include_attributes: bool = False,
        indent: int | None = None,
        show_empty: bool = False,
    ) -> str:
        seen_show_empty.append(show_empty)
        return original_dump(
            node,
            annotate_fields=annotate_fields,
            include_attributes=include_attributes,
            indent=indent,
        )

    monkeypatch.setattr(ast, "dump", python_313_dump)
    dumped = _canonical_ast_dump(ast.parse("def stable():\n    pass\n"))

    assert seen_show_empty == [True]
    assert dumped == original_dump(ast.parse("def stable():\n    pass\n"), include_attributes=False)


def test_current_service_ast_fingerprint_matches_reviewed_constant(current_service_source: bytes) -> None:
    assert _method_fingerprint(current_service_source) == SERVICE_METHODS_PARITY_AST_SHA256


def _pool(count: int) -> tuple[SearchResult, ...]:
    return tuple(
        SearchResult(
            url=f"https://example.test/{index}",
            title=f"title {index}",
            content=f"content {index}",
            engine="github",
            engines={"github", f"engine-{index % 3}"},
            score=float(100 - index),
            position=index + 1,
            category="reference",
            published_date="2026-01-01",
            thumbnail=f"https://example.test/thumb/{index}",
            tier=1 if index % 2 == 0 else 2,
            payload={"item": index},
            work_group={"group_id": f"group-{index}", "representative_engine": "github"},
        )
        for index in range(count)
    )


def _expected_request_sha(query: str, pool: tuple[SearchResult, ...]) -> str:
    candidates = tuple(
        RerankCandidate(
            id=f"c{index}",
            title=_rerank_text(result.title, 256),
            url=_rerank_url(result.url),
            snippet=_rerank_text(result.content, 1200),
        )
        for index, result in enumerate(pool[:MAX_CANDIDATES])
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
    return hashlib.sha256(json.dumps(body, ensure_ascii=False).encode("utf-8")).hexdigest()


def _response(scores: dict[str, int], *, usage: bool = True) -> bytes:
    payload: dict[str, Any] = {
        "model": MODEL,
        "answers": {key: {"type": "score", "score": value} for key, value in scores.items()},
    }
    if usage:
        payload["usage"] = {"input_tokens": 123, "output_tokens": 12}
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def _service() -> SearchService:
    return SearchService(AppContext(active_engines={}, rerank_provider=None))


@pytest.mark.parametrize("count", [20, 45])
async def test_v1_control_reorders_real_objects_stable_ties_and_full_tail(
    count: int, frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(count)
    shortlist_count = min(count, MAX_CANDIDATES)
    scores = {f"c{index}": 6 for index in range(shortlist_count)}
    scores[f"c{shortlist_count - 1}"] = 9
    body = _response(scores)
    service = _service()
    before_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}

    results, status, receipt = await replay_legacy_v1_control(
        service,
        query,
        pool,
        frozen_v1_source_bytes=frozen_v1_source,
        current_service_source_bytes=current_service_source,
        expected_request_sha256=_expected_request_sha(query, pool),
        expected_response_sha256=hashlib.sha256(body).hexdigest(),
        archived_response_bytes=body,
    )

    expected_indices = [shortlist_count - 1, *range(shortlist_count - 1), *range(shortlist_count, count)]
    assert status == "applied"
    assert [result for result in results] == [pool[index] for index in expected_indices]
    assert all(actual is pool[index] for actual, index in zip(results, expected_indices))
    assert receipt.ordered_ids == tuple([f"c{shortlist_count - 1}", *[f"c{i}" for i in range(shortlist_count - 1)]])
    assert receipt.response_exposed is True
    assert receipt.current_strict_json_compatible is True
    assert receipt.usage_state == "known"
    assert receipt.input_tokens == 123 and receipt.output_tokens == 12
    assert service._ctx.rerank_provider is None
    after_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}
    assert after_modules == before_modules


async def test_invalid_v1_response_falls_back_and_reports_strict_compatibility(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(4)
    body = json.dumps(
        {
            "model": MODEL,
            "answers": {"c0": {"type": "score", "score": 1}, "c1": {"type": "score", "score": 2}},
        },
        separators=(",", ":"),
    ).encode()
    service = _service()

    results, status, receipt = await replay_legacy_v1_control(
        service,
        query,
        pool,
        frozen_v1_source_bytes=frozen_v1_source,
        current_service_source_bytes=current_service_source,
        expected_request_sha256=_expected_request_sha(query, pool),
        expected_response_sha256=hashlib.sha256(body).hexdigest(),
        archived_response_bytes=body,
    )

    assert status == "fallback"
    assert all(actual is expected for actual, expected in zip(results, pool))
    assert receipt.response_exposed is True
    assert receipt.current_strict_json_compatible is False
    assert receipt.current_strict_json_reason == "candidate_id_set_mismatch"
    assert receipt.usage_state == "unknown"
    assert receipt.input_tokens is None and receipt.output_tokens is None


async def test_v1_accepts_duplicate_score_key_but_current_strict_parser_is_separately_false(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = (
        b'{"model":"jev-1.13.0","answers":{"c0":{"type":"score","score":9,"score":1},"c1":{"type":"score","score":4}}}'
    )
    service = _service()

    results, status, receipt = await replay_legacy_v1_control(
        service,
        query,
        pool,
        frozen_v1_source_bytes=frozen_v1_source,
        current_service_source_bytes=current_service_source,
        expected_request_sha256=_expected_request_sha(query, pool),
        expected_response_sha256=hashlib.sha256(body).hexdigest(),
        archived_response_bytes=body,
    )

    assert status == "applied"
    assert [item for item in results] == [pool[1], pool[0]]
    assert receipt.current_strict_json_compatible is False
    assert receipt.current_strict_json_reason == "strict_json_rejected:ValueError"


async def test_request_mismatch_is_integrity_failure_after_v1_fail_open(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(3)
    body = _response({"c0": 1, "c1": 2, "c2": 3})
    service = _service()

    with pytest.raises(LegacyReplayIntegrityError) as exc:
        await replay_legacy_v1_control(
            service,
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=current_service_source,
            expected_request_sha256="0" * 64,
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )

    assert exc.value.artifact == "request"
    assert exc.value.observed_sha256 != exc.value.expected_sha256
    assert exc.value.response_exposed is False
    assert service._ctx.rerank_provider is None


async def test_source_and_response_digest_failures_happen_before_execution(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = _response({"c0": 1, "c1": 2})
    service = _service()
    before_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}

    with pytest.raises(LegacyReplayIntegrityError) as source_error:
        await replay_legacy_v1_control(
            service,
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source + b"#tampered",
            current_service_source_bytes=current_service_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )
    assert source_error.value.artifact == "source"

    with pytest.raises(LegacyReplayIntegrityError) as response_error:
        await replay_legacy_v1_control(
            service,
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=current_service_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256="0" * 64,
            archived_response_bytes=body,
        )
    assert response_error.value.artifact == "response"
    after_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}
    assert after_modules == before_modules


async def test_response_byte_limit_is_enforced_before_v1_parser(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = b" " * 2_000_001
    service = _service()

    with pytest.raises(LegacyReplayAdmissionError, match="2,000,000-byte"):
        await replay_legacy_v1_control(
            service,
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=current_service_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )


async def test_supplied_service_source_drift_is_rejected_before_loading_v1(
    frozen_v1_source: bytes, current_service_source: bytes
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = _response({"c0": 1, "c1": 2})
    before_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}

    changed_source = current_service_source.replace(b'return ranked, "skipped"', b'return ranked, "changed"', 1)
    assert changed_source != current_service_source
    with pytest.raises(LegacyReplayAdmissionError, match="projection code differs"):
        await replay_legacy_v1_control(
            _service(),
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=changed_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )
    after_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}
    assert after_modules == before_modules


async def test_loaded_method_drift_is_rejected_even_with_unchanged_source(
    frozen_v1_source: bytes, current_service_source: bytes, monkeypatch: pytest.MonkeyPatch
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = _response({"c0": 1, "c1": 2})
    before_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}

    async def changed_method(self: SearchService, *args: object, **kwargs: object) -> tuple[list[SearchResult], str]:
        return list(pool), "applied"

    monkeypatch.setattr(SearchService, "_rerank_results", changed_method)
    with pytest.raises(LegacyReplayAdmissionError, match="projection code differs"):
        await replay_legacy_v1_control(
            _service(),
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=current_service_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )
    after_modules = {name for name in sys.modules if name.startswith("_coverage_frozen_jev_v1_")}
    assert after_modules == before_modules


@pytest.mark.parametrize("mutation", ["exceptiontable", "bytecode"])
async def test_loaded_code_object_changes_are_rejected(
    mutation: str,
    frozen_v1_source: bytes,
    current_service_source: bytes,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    query = "offline frozen task"
    pool = _pool(2)
    body = _response({"c0": 1, "c1": 2})
    original = SearchService._rerank_results
    original_code = original.__code__
    if mutation == "exceptiontable":
        assert original_code.co_exceptiontable
        changed_code = original_code.replace(co_exceptiontable=b"")
    else:
        changed_bytecode = bytearray(original_code.co_code)
        changed_bytecode[-1] ^= 1
        changed_code = original_code.replace(co_code=bytes(changed_bytecode))
    changed_function = types.FunctionType(
        changed_code,
        original.__globals__,
        original.__name__,
        original.__defaults__,
        original.__closure__,
    )
    monkeypatch.setattr(SearchService, "_rerank_results", changed_function)

    with pytest.raises(LegacyReplayAdmissionError, match="projection code differs"):
        await replay_legacy_v1_control(
            _service(),
            query,
            pool,
            frozen_v1_source_bytes=frozen_v1_source,
            current_service_source_bytes=current_service_source,
            expected_request_sha256=_expected_request_sha(query, pool),
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
        )


def test_code_constant_fingerprint_distinguishes_integer_one_from_true() -> None:
    original_code = SearchService._rerank_results.__code__
    integer_constants = tuple(1 if type(item) is int and item == 1000 else item for item in original_code.co_consts)
    boolean_constants = tuple(True if type(item) is int and item == 1000 else item for item in original_code.co_consts)
    integer_code = original_code.replace(co_consts=integer_constants)
    boolean_code = original_code.replace(co_consts=boolean_constants)

    assert _runtime_value(integer_code) != _runtime_value(boolean_code)
