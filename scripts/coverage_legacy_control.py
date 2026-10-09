"""Offline replay adapter for the source-pinned Jev V1 W0 control.

The caller supplies the exact source bytes obtained from the frozen Git blob.
This module checks the immutable SHA-256 before executing those bytes and has
no Git, filesystem, network, environment, or credential lookup path. It then
replays one externally hash-pinned response through the unchanged V1 parser
and the current SearchService projection only when the reviewed service-method
fingerprint matches the W0 AST parity receipt.

This proves only the successful-response V1 control path for an already
canonical pool. It does not qualify source acquisition, full image/runtime
state, timing, all fail-open branches, or any scientific result. Missing or
invalid usage stays explicitly unknown with null token fields; it is never
reported as zero.
"""

from __future__ import annotations

import copy
import hashlib
import json
import math
import re
import sys
import types
import uuid
from dataclasses import dataclass
from unittest.mock import patch

import httpx

from slopsearx import rerank as current_rerank
from slopsearx.adapter import SearchResult
from slopsearx.rerank import RerankCandidate as CurrentRerankCandidate
from slopsearx.rerank import RerankDecision as CurrentRerankDecision
from slopsearx.service import SearchService, _rerank_text, _rerank_url

FROZEN_V1_COMMIT = "8f3577d022e2d98fcd405d915b5c3b9b16e899bf"
FROZEN_V1_BLOB_SHA1 = "aab38edc92b2ca7e8e796e8a1af07edc7c55ad81"
FROZEN_V1_SOURCE_SHA256 = "93ea486848ccdc7778a0ce49973773402d84fbb2ca1b9a64eaabf5f37da6e078"
MAX_REPLAY_RESPONSE_BYTES = 2_000_000

# These fingerprints were derived from the read-only AST parity check against
# FROZEN_V1_COMMIT. The AST digest records the reviewed source equivalence;
# the code fingerprint makes the loaded service fail closed if it later drifts.
SERVICE_METHODS_PARITY_AST_SHA256 = "b3b14635045a511b98b21e74181b03f50becd92691ba1ae2e4e13cffe0ee347a"
SERVICE_METHODS_RUNTIME_SHA256 = "70a86be3a128014eea9b3096b37cb755765c305154f55128bf701b37d446f534"


class LegacyReplayIntegrityError(RuntimeError):
    """A source, request, or response receipt did not match its expected hash."""

    def __init__(
        self,
        artifact: str,
        expected_sha256: str,
        observed_sha256: str | None,
        *,
        response_exposed: bool | None = None,
    ) -> None:
        self.artifact = artifact
        self.expected_sha256 = expected_sha256
        self.observed_sha256 = observed_sha256
        self.response_exposed = response_exposed
        super().__init__(f"legacy replay {artifact} digest mismatch; replay rejected")


class LegacyReplayAdmissionError(RuntimeError):
    """The offline response or production-method gate is outside the contract."""


@dataclass(frozen=True)
class LegacyControlReceipt:
    """Receipt of this V1 parser replay, not full source/runtime qualification.

    ``usage_state`` is ``unknown`` with null token counts when the archived
    response lacks valid usage numbers; it never fabricates zero usage.
    """

    source_sha256: str
    service_parity_ast_sha256: str
    request_sha256: str | None
    response_sha256: str
    response_bytes: int
    response_exposed: bool
    rerank_status: str
    ordered_ids: tuple[str, ...] | None
    current_strict_json_compatible: bool
    current_strict_json_reason: str
    usage_state: str
    input_tokens: int | None
    output_tokens: int | None


def _runtime_value(value: object) -> object:
    if isinstance(value, types.CodeType):
        return {
            "bytecode": value.co_code.hex(),
            "constants": [_runtime_value(item) for item in value.co_consts],
            "names": value.co_names,
            "varnames": value.co_varnames,
            "argcount": value.co_argcount,
            "kwonly": value.co_kwonlyargcount,
            "flags": value.co_flags,
        }
    if isinstance(value, (tuple, list)):
        return [_runtime_value(item) for item in value]
    if isinstance(value, (set, frozenset)):
        return sorted((_runtime_value(item) for item in value), key=repr)
    if isinstance(value, bytes):
        return {"bytes": value.hex()}
    if isinstance(value, (str, int, float, bool, type(None))):
        return value
    return repr(value)


def _service_runtime_fingerprint() -> str:
    import json as json_module

    functions = (
        ("SearchService._rerank_results", SearchService._rerank_results),
        ("_rerank_text", _rerank_text),
        ("_rerank_url", _rerank_url),
    )
    material = [(name, _runtime_value(function.__code__)) for name, function in functions]
    encoded = json_module.dumps(material, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _validate_digest(label: str, value: str) -> None:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ValueError(f"{label} must be a lowercase SHA-256 hex digest")


def _load_frozen_v1(source_bytes: bytes) -> types.ModuleType:
    if not isinstance(source_bytes, bytes):
        raise TypeError("frozen_v1_source_bytes must be exact bytes")
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    if source_sha256 != FROZEN_V1_SOURCE_SHA256:
        raise LegacyReplayIntegrityError("source", FROZEN_V1_SOURCE_SHA256, source_sha256)

    module_name = f"_coverage_frozen_jev_v1_{uuid.uuid4().hex}"
    module = types.ModuleType(module_name)
    module.__file__ = f"<git-blob:{FROZEN_V1_BLOB_SHA1}>"
    code = compile(source_bytes, module.__file__, "exec", dont_inherit=True)
    # dataclasses consult sys.modules while classes are being created. Restore
    # the process table immediately after execution; the returned module stays
    # private to this call and is never installed as an importable package.
    with patch.dict(sys.modules, {module_name: module}):
        exec(code, module.__dict__)  # noqa: S102 - exact SHA-pinned public source
    if module.MODEL != current_rerank.MODEL or tuple(module.LEVELS) != tuple(current_rerank.LEVELS):
        raise LegacyReplayAdmissionError("V1 Score model or levels differ from reviewed compatibility contract")
    if module.INSTRUCTIONS != current_rerank.INSTRUCTIONS:
        raise LegacyReplayAdmissionError("V1 Score instructions differ from reviewed compatibility contract")
    return module


def _strict_current_compatibility(body: bytes, candidate_ids: tuple[str, ...]) -> tuple[bool, str]:
    """Report V2 strict-parser compatibility separately from the V1 outcome."""
    try:
        parsed = json.loads(
            body,
            object_pairs_hook=current_rerank._unique_json_object,
            parse_constant=current_rerank._reject_json_constant,
            parse_float=current_rerank._finite_json_float,
        )
    except Exception as exc:
        return False, f"strict_json_rejected:{type(exc).__name__}"
    if not isinstance(parsed, dict):
        return False, "root_not_object"
    answers = parsed.get("answers")
    if parsed.get("model") != current_rerank.MODEL or not isinstance(answers, dict):
        return False, "model_or_answers_invalid"
    if set(answers) != set(candidate_ids):
        return False, "candidate_id_set_mismatch"
    for candidate_id in candidate_ids:
        answer = answers[candidate_id]
        if not isinstance(answer, dict):
            return False, "answer_not_object"
        score = answer.get("score")
        if answer.get("type") != "score" or type(score) not in (int, float):
            return False, "score_type_invalid"
        assert isinstance(score, (int, float))
        if not math.isfinite(score) or not 0 <= score <= len(current_rerank.LEVELS) - 1:
            return False, "score_range_invalid"
    return True, "strict_current_parser_accepts"


def _usage_observation(body: bytes) -> tuple[str, int | None, int | None]:
    try:
        parsed = json.loads(body)
    except Exception:
        return "unknown", None, None
    usage = parsed.get("usage") if isinstance(parsed, dict) else None
    if not isinstance(usage, dict):
        return "unknown", None, None
    input_tokens, output_tokens = usage.get("input_tokens"), usage.get("output_tokens")
    if type(input_tokens) is int and input_tokens >= 0 and type(output_tokens) is int and output_tokens >= 0:
        return "known", input_tokens, output_tokens
    return "unknown", None, None


async def replay_legacy_v1_control(
    service: SearchService,
    query: str,
    canonical_pool: tuple[SearchResult, ...],
    *,
    frozen_v1_source_bytes: bytes,
    expected_request_sha256: str,
    expected_response_sha256: str,
    archived_response_bytes: bytes,
) -> tuple[list[SearchResult], str, LegacyControlReceipt]:
    """Replay one hash-pinned response through original V1 and shared service.

    Source bytes are supplied by the caller after retrieval from the pinned
    public Git blob. This function never reads files, calls Git, opens a real
    transport, reads environment variables, or contacts a provider.
    """
    _validate_digest("expected_request_sha256", expected_request_sha256)
    _validate_digest("expected_response_sha256", expected_response_sha256)
    if not isinstance(archived_response_bytes, bytes):
        raise TypeError("archived_response_bytes must be exact response bytes")
    observed_response_sha256 = hashlib.sha256(archived_response_bytes).hexdigest()
    if observed_response_sha256 != expected_response_sha256:
        raise LegacyReplayIntegrityError("response", expected_response_sha256, observed_response_sha256)
    if len(archived_response_bytes) > MAX_REPLAY_RESPONSE_BYTES:
        raise LegacyReplayAdmissionError("archived response exceeds the 2,000,000-byte stage guard")
    if _service_runtime_fingerprint() != SERVICE_METHODS_RUNTIME_SHA256:
        raise LegacyReplayAdmissionError("current service projection code differs from AST-reviewed W0 methods")
    if not isinstance(canonical_pool, tuple) or not all(isinstance(item, SearchResult) for item in canonical_pool):
        raise TypeError("canonical_pool must be a tuple of full SearchResult objects")
    if len(canonical_pool) < 2:
        raise LegacyReplayAdmissionError("V1 successful-response replay requires at least two candidates")
    if not isinstance(query, str):
        raise TypeError("query must be a string")
    if not query.strip():
        raise LegacyReplayAdmissionError("query must be non-empty")
    try:
        if len(query.encode("utf-8")) > 4096:
            raise LegacyReplayAdmissionError("query exceeds the V1 4,096-byte request guard")
    except UnicodeEncodeError as exc:
        raise LegacyReplayAdmissionError("query is not valid UTF-8 for the V1 request") from exc

    v1 = _load_frozen_v1(frozen_v1_source_bytes)
    if not isinstance(service, SearchService):
        raise TypeError("service must be the current SearchService instance")
    original_provider = service._ctx.rerank_provider
    if original_provider is not None:
        raise LegacyReplayAdmissionError("offline replay requires an otherwise unconfigured service provider")
    original_pool = canonical_pool
    before = copy.deepcopy(original_pool)
    request_sha256: str | None = None
    response_exposed = False
    v1_ids: tuple[str, ...] | None = None
    strict_ok, strict_reason = _strict_current_compatibility(
        archived_response_bytes, tuple(f"c{i}" for i in range(min(len(canonical_pool), v1.MAX_CANDIDATES)))
    )
    usage_state, input_tokens, output_tokens = _usage_observation(archived_response_bytes)

    class _PinnedResponse:
        def raise_for_status(self) -> None:
            return None

        def json(self) -> object:
            # Exercise the original implementation's response.json() call
            # against the actual HTTPX response parser and exact pinned bytes.
            return httpx.Response(
                200,
                content=archived_response_bytes,
                request=httpx.Request("POST", "https://replay.invalid/"),
            ).json()

    class _OfflineClient:
        def __init__(self, **kwargs: object) -> None:
            if kwargs.get("follow_redirects") is not False or kwargs.get("trust_env") is not False:
                raise LegacyReplayAdmissionError("unexpected V1 HTTP client configuration")

        async def __aenter__(self) -> _OfflineClient:
            return self

        async def __aexit__(self, *args: object) -> None:
            return None

        async def post(self, url: str, **kwargs: object) -> _PinnedResponse:
            nonlocal request_sha256, response_exposed
            if url != "https://api.typesafe.ai/v1/systemone":
                raise LegacyReplayAdmissionError("unexpected V1 provider endpoint")
            body = kwargs.get("content")
            if not isinstance(body, bytes):
                raise LegacyReplayAdmissionError("V1 request body was not bytes")
            request_sha256 = hashlib.sha256(body).hexdigest()
            if request_sha256 != expected_request_sha256:
                # V1 catches ordinary exceptions and returns None. The outer
                # integrity check below converts this swallowed mismatch back
                # into a hard replay rejection.
                raise ValueError("archived request digest mismatch")
            response_exposed = True
            return _PinnedResponse()

    class _V1Bridge:
        async def rerank(
            self, bridge_query: str, current_candidates: tuple[CurrentRerankCandidate, ...]
        ) -> CurrentRerankDecision | None:
            nonlocal v1_ids
            old_candidates = tuple(
                v1.RerankCandidate(
                    id=candidate.id,
                    title=candidate.title,
                    url=candidate.url,
                    snippet=candidate.snippet,
                )
                for candidate in current_candidates
            )
            old_decision = await old_provider.rerank(bridge_query, old_candidates)
            if old_decision is None:
                return None
            v1_ids = tuple(old_decision.ordered_ids)
            # Preserve V1's exact ID order. Current service validates and maps
            # the permutation onto the original SearchResult objects.
            return CurrentRerankDecision(tuple(old_decision.ordered_ids))

        def cache_identity(self) -> str:
            return "legacy-v1-offline-replay"

    old_provider = v1.JevReranker("offline-replay-no-credential")
    offline_httpx_namespace = {**vars(httpx), "AsyncClient": _OfflineClient}
    setattr(v1, "httpx", types.SimpleNamespace(**offline_httpx_namespace))
    service._ctx.rerank_provider = _V1Bridge()
    try:
        results, status = await service._rerank_results(query, list(canonical_pool), current_rerank.RERANK_TIMEOUT_S)
    finally:
        service._ctx.rerank_provider = original_provider

    if request_sha256 != expected_request_sha256 or not response_exposed:
        raise LegacyReplayIntegrityError(
            "request",
            expected_request_sha256,
            request_sha256,
            response_exposed=response_exposed,
        )
    after = copy.deepcopy(original_pool)
    if before != after:
        raise AssertionError("V1 replay mutated a supplied SearchResult or its metadata")
    if len(results) != len(original_pool) or {id(item) for item in results} != {id(item) for item in original_pool}:
        raise AssertionError("V1 replay did not preserve a permutation of original result objects")

    receipt = LegacyControlReceipt(
        source_sha256=FROZEN_V1_SOURCE_SHA256,
        service_parity_ast_sha256=SERVICE_METHODS_PARITY_AST_SHA256,
        request_sha256=request_sha256,
        response_sha256=observed_response_sha256,
        response_bytes=len(archived_response_bytes),
        response_exposed=response_exposed,
        rerank_status=status,
        ordered_ids=v1_ids,
        current_strict_json_compatible=strict_ok,
        current_strict_json_reason=strict_reason,
        usage_state=usage_state,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
    )
    return results, status, receipt
