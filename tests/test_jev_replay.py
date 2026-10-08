"""Offline production-control replay tests; no provider or network calls."""

from __future__ import annotations

import hashlib
import json
from typing import Any

import pytest

from slopsearx.adapter import SearchResult
from slopsearx.rerank import MODEL, JevReranker
from slopsearx.service import AppContext, SearchService
from tests.jev_replay import ReplayIntegrityError, replay_archived_jev_response


def _pool(count: int) -> tuple[SearchResult, ...]:
    return tuple(
        SearchResult(
            url=f"https://example.test/{i}",
            title=f"title {i}",
            content=f"content {i}",
            engine="brave",
            engines={"brave", f"source-{i % 3}"},
            score=float(100 - i),
            position=i + 1,
            category="science",
            published_date="2025-01-01",
            thumbnail=f"https://example.test/thumb/{i}",
            tier=1 if i % 2 == 0 else 2,
            payload={"kind": "record", "n": i},
            work_group={"group_id": f"w{i}", "representative_engine": "brave"},
        )
        for i in range(count)
    )


def _response(scores: dict[str, int], *, answer_ids: list[str] | None = None) -> bytes:
    answers = {candidate_id: {"type": "score", "score": score} for candidate_id, score in scores.items()}
    if answer_ids is not None:
        answers = {key: answers[key] for key in answer_ids if key in answers}
    return json.dumps({"model": MODEL, "answers": answers}, separators=(",", ":")).encode()


def _request_sha_for_contract(query: str, pool: tuple[SearchResult, ...]) -> str:
    """Independent receipt fixture for the known Jev request shape in this test."""
    from slopsearx.rerank import INSTRUCTIONS, LEVELS, MAX_CANDIDATES, RerankCandidate
    from slopsearx.service import _rerank_text, _rerank_url

    candidates = tuple(
        RerankCandidate(
            id=f"c{i}",
            title=_rerank_text(item.title, 256),
            url=_rerank_url(item.url),
            snippet=_rerank_text(item.content, 1200),
        )
        for i, item in enumerate(pool[:MAX_CANDIDATES])
    )
    body = {
        "model": MODEL,
        "state": {"query": query, "candidates": [candidate.__dict__ for candidate in candidates]},
        "questions": {
            candidate.id: {
                "type": "score",
                "instructions": INSTRUCTIONS.format(id=candidate.id),
                "criteria": LEVELS,
            }
            for candidate in candidates
        },
    }
    encoded = json.dumps(body, ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


@pytest.mark.parametrize("count", [2, 45])
async def test_replays_archived_bytes_through_original_service_and_parser(count: int, monkeypatch: Any) -> None:
    query = "frozen query"
    pool = _pool(count)
    # Equal top scores verify the parser's stable cN tie order; the highest
    # score moves the final shortlisted card to the front.
    shortlist_count = min(count, 40)
    scores = {f"c{i}": (9 if i == shortlist_count - 1 else 7 if i < 2 else 2) for i in range(shortlist_count)}
    body = _response(scores)
    service = SearchService(AppContext(active_engines={}, rerank_provider=JevReranker("offline-only")))

    results, status, receipt = await replay_archived_jev_response(
        service,
        query,
        pool,
        expected_request_sha256=_request_sha_for_contract(query, pool),
        expected_response_sha256=hashlib.sha256(body).hexdigest(),
        archived_response_bytes=body,
        monkeypatch=monkeypatch,
    )

    shortlist_ids = sorted(range(shortlist_count), key=lambda i: (-scores[f"c{i}"], i))
    expected_ids = shortlist_ids + list(range(shortlist_count, count))
    assert status == "applied"
    assert [item for item in results] == [pool[i] for i in expected_ids]
    assert all(actual is pool[i] for actual, i in zip(results, expected_ids))
    assert receipt.exchange_accepted
    assert receipt.request_sha256 == _request_sha_for_contract(query, pool)
    assert receipt.response_sha256 == hashlib.sha256(body).hexdigest()
    # Metadata and positions are original objects and remain untouched here;
    # SearchService.search() owns reassignment after an applied rerank.
    assert [item.position for item in results] == [pool[i].position for i in expected_ids]
    assert [item.payload for item in results] == [pool[i].payload for i in expected_ids]


async def test_request_digest_mismatch_is_integrity_failure_not_fallback(monkeypatch: Any) -> None:
    query = "frozen query"
    pool = _pool(3)
    body = _response({"c0": 1, "c1": 2, "c2": 3})
    service = SearchService(AppContext(active_engines={}, rerank_provider=JevReranker("offline-only")))
    with pytest.raises(ReplayIntegrityError) as error:
        await replay_archived_jev_response(
            service,
            query,
            pool,
            expected_request_sha256="0" * 64,
            expected_response_sha256=hashlib.sha256(body).hexdigest(),
            archived_response_bytes=body,
            monkeypatch=monkeypatch,
        )
    assert error.value.artifact == "request"
    assert error.value.expected_sha256 == "0" * 64
    assert error.value.observed_sha256 != "0" * 64
    # A mismatch is not surfaced as ordinary provider fallback; the production
    # parser swallows transport errors, so the adapter must fail closed after it returns.
    assert "archived request digest mismatch" in str(error.value)


async def test_malformed_candidate_membership_uses_unchanged_service_fallback(monkeypatch: Any) -> None:
    query = "frozen query"
    pool = _pool(4)
    body = _response({f"c{i}": i for i in range(4)}, answer_ids=["c0", "c1", "c2"])
    service = SearchService(AppContext(active_engines={}, rerank_provider=JevReranker("offline-only")))
    results, status, receipt = await replay_archived_jev_response(
        service,
        query,
        pool,
        expected_request_sha256=_request_sha_for_contract(query, pool),
        expected_response_sha256=hashlib.sha256(body).hexdigest(),
        archived_response_bytes=body,
        monkeypatch=monkeypatch,
    )
    assert status == "fallback"
    assert all(actual is expected for actual, expected in zip(results, pool))
    assert receipt.exchange_accepted


async def test_response_digest_mismatch_rejected_before_parser(monkeypatch: Any) -> None:
    query = "frozen query"
    pool = _pool(3)
    body = _response({"c0": 1, "c1": 2, "c2": 3})
    service = SearchService(AppContext(active_engines={}, rerank_provider=JevReranker("offline-only")))
    parser_calls = 0
    original_rerank = service._rerank_results

    async def count_parser_calls(
        query_arg: str, ranked: list[SearchResult], timeout_s: float
    ) -> tuple[list[SearchResult], str]:
        nonlocal parser_calls
        parser_calls += 1
        return await original_rerank(query_arg, ranked, timeout_s)

    monkeypatch.setattr(service, "_rerank_results", count_parser_calls)
    with pytest.raises(ReplayIntegrityError) as error:
        await replay_archived_jev_response(
            service,
            query,
            pool,
            expected_request_sha256=_request_sha_for_contract(query, pool),
            expected_response_sha256="0" * 64,
            archived_response_bytes=body,
            monkeypatch=monkeypatch,
        )
    assert error.value.artifact == "response"
    assert error.value.expected_sha256 == "0" * 64
    assert error.value.observed_sha256 == hashlib.sha256(body).hexdigest()
    assert parser_calls == 0
