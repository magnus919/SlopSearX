"""Offline replay seam for archived Jev responses through production code.

This is a test utility, not a runtime/provider adapter. It proves only the
shared-service shortlist projection and Jev parser behavior for supplied bytes.
"""

from __future__ import annotations

import copy
import hashlib
import re
from collections.abc import AsyncIterator
from dataclasses import dataclass
from types import SimpleNamespace
from typing import Any

from slopsearx import rerank as rerank_module
from slopsearx.adapter import SearchResult
from slopsearx.rerank import JevReranker
from slopsearx.service import SearchService


@dataclass(frozen=True)
class ReplayReceipt:
    request_sha256: str | None
    response_sha256: str
    exchange_accepted: bool


class ReplayIntegrityError(RuntimeError):
    """A local replay receipt did not match an archived digest."""

    def __init__(self, artifact: str, expected_sha256: str, observed_sha256: str | None) -> None:
        self.artifact = artifact
        self.expected_sha256 = expected_sha256
        self.observed_sha256 = observed_sha256
        super().__init__(f"archived {artifact} digest mismatch; replay rejected")


class _ArchivedResponse:
    def __init__(self, body: bytes) -> None:
        self._body = body
        self.headers: dict[str, str] = {}

    async def __aenter__(self) -> _ArchivedResponse:
        return self

    async def __aexit__(self, *args: object) -> None:
        return None

    def raise_for_status(self) -> None:
        return None

    async def aiter_bytes(self) -> AsyncIterator[bytes]:
        # One exact chunk mirrors the archived response representation.
        yield self._body


async def replay_archived_jev_response(
    service: SearchService,
    query: str,
    canonical_pool: tuple[SearchResult, ...],
    *,
    expected_request_sha256: str,
    expected_response_sha256: str,
    archived_response_bytes: bytes,
    monkeypatch: Any,
) -> tuple[list[SearchResult], str, ReplayReceipt]:
    """Run the real service projection and Jev parser without network access.

    `canonical_pool` must be the complete, already-grouped/deduplicated ranked
    production pool (before presentation slicing). Externally supplied request
    and response hashes are checked; response bytes are rejected before parser
    invocation when their receipt does not match.
    """
    provider = service._ctx.rerank_provider
    if not isinstance(provider, JevReranker):
        raise TypeError("offline replay requires the production JevReranker")
    if not isinstance(canonical_pool, tuple) or not all(isinstance(item, SearchResult) for item in canonical_pool):
        raise TypeError("canonical_pool must be a tuple of full SearchResult objects")
    if not isinstance(expected_request_sha256, str) or re.fullmatch(r"[0-9a-f]{64}", expected_request_sha256) is None:
        raise ValueError("expected_request_sha256 must be a lowercase SHA-256 hex digest")
    if not isinstance(expected_response_sha256, str) or re.fullmatch(r"[0-9a-f]{64}", expected_response_sha256) is None:
        raise ValueError("expected_response_sha256 must be a lowercase SHA-256 hex digest")
    if not isinstance(archived_response_bytes, bytes):
        raise TypeError("archived_response_bytes must be exact response bytes")
    observed_response_sha256 = hashlib.sha256(archived_response_bytes).hexdigest()
    if observed_response_sha256 != expected_response_sha256:
        raise ReplayIntegrityError("response", expected_response_sha256, observed_response_sha256)

    original_objects = canonical_pool
    before = copy.deepcopy(canonical_pool)
    request_sha256: str | None = None
    exchange_accepted = False

    class OfflineClient:
        def __init__(self, **kwargs: object) -> None:
            if kwargs.get("follow_redirects") is not False or kwargs.get("trust_env") is not False:
                raise AssertionError("unexpected production transport settings")

        async def __aenter__(self) -> OfflineClient:
            return self

        async def __aexit__(self, *args: object) -> None:
            return None

        def stream(self, method: str, url: str, **kwargs: object) -> _ArchivedResponse:
            nonlocal request_sha256, exchange_accepted
            if method != "POST" or url != "https://api.typesafe.ai/v1/systemone":
                raise AssertionError("unexpected Jev request target")
            body = kwargs.get("content")
            if not isinstance(body, bytes):
                raise AssertionError("Jev request body must be bytes")
            request_sha256 = hashlib.sha256(body).hexdigest()
            if request_sha256 != expected_request_sha256:
                # No archived response is exposed on request-receipt mismatch.
                raise ValueError("archived request digest mismatch")
            exchange_accepted = True
            return _ArchivedResponse(archived_response_bytes)

    # Replace only rerank.py's imported httpx namespace; no socket-capable
    # AsyncClient remains reachable from this production call path.
    with monkeypatch.context() as patch:
        patch.setattr(rerank_module, "httpx", SimpleNamespace(AsyncClient=OfflineClient))
        results, status = await service._rerank_results(query, list(original_objects), 1.0)

    if canonical_pool != tuple(before):
        raise AssertionError("production replay mutated the supplied canonical pool")
    same_objects = {id(item) for item in results} == {id(item) for item in original_objects}
    if len(results) != len(original_objects) or not same_objects:
        raise AssertionError("production replay did not preserve the exact SearchResult object permutation")
    if request_sha256 != expected_request_sha256 or not exchange_accepted:
        raise ReplayIntegrityError("request", expected_request_sha256, request_sha256)
    receipt = ReplayReceipt(
        request_sha256=request_sha256,
        response_sha256=observed_response_sha256,
        exchange_accepted=exchange_accepted,
    )
    return results, status, receipt
