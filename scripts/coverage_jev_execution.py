"""One-shot, injected Jev execution and exact-response replay seam.

This is a composable prototype, not an admission authority. Callers must obtain
the exact permit bytes and their independent digest through a separately
qualified path. The module never reads credentials or other environment
variables, logs request/response material, retries, or creates a permit.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import math
import os
import re
import stat
import time
import weakref
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, Mapping, cast

import httpx

from scripts import coverage_study_core as core
from scripts import intent_ranking_coverage as coverage
from scripts import intent_ranking_receipts as receipts

if TYPE_CHECKING:
    from scripts.coverage_legacy_control import LegacyControlReceipt
    from slopsearx.adapter import SearchResult
    from slopsearx.service import SearchService

JEV_SYSTEMONE_URL = "https://api.typesafe.ai/v1/systemone"
PERMIT_SCHEMA = "coverage-jev-operation-permit/1"
FINAL_RECEIPT_SCHEMA = "coverage-jev-operation-result/1"
INVENTORY_SCHEMA = "coverage-jev-terminal-inventory/1"
OBSERVATION_SCHEMA = "coverage-jev-operation-observation/1"
SELECTOR_PHASE_SECONDS = 1.0
MAX_REQUEST_BYTES = 384_000
MAX_RESPONSE_BYTES = 2_000_000
MAX_OPERATION_INPUT_BYTES = 384_000
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_SHA1 = re.compile(r"[0-9a-f]{40}\Z")
_OP_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
_LEDGER_LOCKS: weakref.WeakKeyDictionary[core.StudyRun, asyncio.Lock] = weakref.WeakKeyDictionary()


@dataclass
class _LedgerActivity:
    active: int = 0
    maximum_active: int = 0


_LEDGER_ACTIVITY: weakref.WeakKeyDictionary[core.StudyRun, _LedgerActivity] = weakref.WeakKeyDictionary()
_LEDGER_RESOURCE_OBSERVATIONS: weakref.WeakKeyDictionary[core.StudyRun, dict[str, dict[str, object]]] = (
    weakref.WeakKeyDictionary()
)


class JevExecutionError(RuntimeError):
    """Execution refused or a durable artifact could not be verified."""


@dataclass(frozen=True)
class CallResult:
    operation_id: str
    status: str
    request_sha256: str
    response_sha256: str | None
    response_bytes: int
    archive_receipt_sha256: str | None
    result_receipt_sha256: str | None
    ranking: coverage.RankingResult | None
    terminal_reason: str | None
    native_status: str | None = None
    native_ordered_ids: tuple[str, ...] | None = None
    native_strict_json_compatible: bool | None = None
    native_strict_json_reason: str | None = None
    observed_elapsed_ms: int | None = None
    dispatch_count: int | None = None
    request_bytes: int | None = None
    input_tokens_observed: int | None = None
    output_tokens_observed: int | None = None
    serialized_operation_sequence: int | None = None
    max_concurrent_operations_observed: int | None = None


class _DispatchCountingTransport(httpx.AsyncBaseTransport):
    """Count requests at the actual HTTPX transport boundary."""

    def __init__(self, inner: httpx.AsyncBaseTransport) -> None:
        self.inner = inner
        self.dispatch_count = 0

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        self.dispatch_count += 1
        return await self.inner.handle_async_request(request)

    async def aclose(self) -> None:
        await self.inner.aclose()


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _contains_credential(raw: bytes, credential: bytes) -> bool:
    if not credential:
        return False
    if credential in raw:
        return True
    # When an over-limit response is truncated, do not persist a body whose
    # suffix could be the beginning of the credential continued past the cap.
    max_prefix = min(len(credential) - 1, len(raw))
    return any(raw.endswith(credential[:size]) for size in range(1, max_prefix + 1))


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
                "utf-8"
            )
            + b"\n"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise JevExecutionError("artifact-json-invalid") from exc


def _write_exclusive(root: Path, name: str, body: bytes) -> str:
    """Durably create one private file; partial files intentionally consume slot."""
    if "/" in name or name in {".", ".."}:
        raise JevExecutionError("artifact-name-invalid")
    root_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        dir_fd = os.open(root, root_flags)
        root_info = os.fstat(dir_fd)
        if not stat.S_ISDIR(root_info.st_mode) or stat.S_IMODE(root_info.st_mode) != 0o700:
            os.close(dir_fd)
            raise JevExecutionError("private-root-changed")
    except JevExecutionError:
        raise
    except OSError as exc:
        raise JevExecutionError("private-artifact-directory-open-failed") from exc
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(name, flags, 0o600, dir_fd=dir_fd)
    except FileExistsError as exc:
        os.close(dir_fd)
        raise JevExecutionError("one-shot-artifact-already-exists") from exc
    except OSError as exc:
        os.close(dir_fd)
        raise JevExecutionError("private-artifact-create-failed") from exc
    try:
        view = memoryview(body)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise JevExecutionError("private-artifact-short-write")
            view = view[written:]
        os.fsync(fd)
    except BaseException:
        os.close(dir_fd)
        raise
    finally:
        os.close(fd)
    try:
        os.fsync(dir_fd)
    except OSError as exc:
        raise JevExecutionError("private-artifact-directory-fsync-failed") from exc
    finally:
        os.close(dir_fd)
    return _sha(body)


def _private_root(value: str | os.PathLike[str]) -> Path:
    path = Path(value)
    try:
        info = path.lstat()
    except OSError as exc:
        raise JevExecutionError("private-root-unavailable") from exc
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o700
        or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
    ):
        raise JevExecutionError("private-root-unsafe")
    return path


def _permit(
    raw: bytes,
    expected_sha256: str,
    *,
    prepared: core.PreparedStage,
    operation_id: str,
    operation_input_sha256: str,
    request_sha256: str,
    parser_mode: str,
) -> dict[str, str]:
    if type(raw) is not bytes or not raw or len(raw) > 16_384:
        raise JevExecutionError("external-permit-bytes-invalid")
    if type(expected_sha256) is not str or not _SHA256.fullmatch(expected_sha256) or _sha(raw) != expected_sha256:
        raise JevExecutionError("external-permit-pin-mismatch")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError, RecursionError) as exc:
        raise JevExecutionError("external-permit-invalid") from exc
    fields = {
        "schema",
        "status",
        "stage_uuid",
        "operation_id",
        "parser_mode",
        "source_revision",
        "protocol_sha256",
        "source_closure_sha256",
        "operation_input_sha256",
        "request_body_sha256",
    }
    if type(value) is not dict or set(value) != fields or _canonical(value) != raw:
        raise JevExecutionError("external-permit-schema")
    expected = {
        "schema": PERMIT_SCHEMA,
        "status": "verified-admitted",
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "parser_mode": parser_mode,
        "source_revision": prepared.source_revision,
        "protocol_sha256": prepared.pins.get("protocol"),
        "source_closure_sha256": prepared.pins.get("qualified_source_closure"),
        "operation_input_sha256": operation_input_sha256,
        "request_body_sha256": request_sha256,
    }
    if value != expected:
        raise JevExecutionError("external-permit-binding-mismatch")
    return value


def _unique_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result


def _observed_usage(body: bytes) -> tuple[int, int] | None:
    try:
        document = coverage._strict_json(body)
    except (coverage.CoverageError, RecursionError):
        return None
    usage = document.get("usage") if type(document) is dict else None
    if (
        type(usage) is dict
        and set(usage) == {"input_tokens", "output_tokens"}
        and type(usage["input_tokens"]) is int
        and type(usage["output_tokens"]) is int
        and usage["input_tokens"] >= 0
        and usage["output_tokens"] >= 0
    ):
        return usage["input_tokens"], usage["output_tokens"]
    return None


def _operation_input(raw: bytes) -> dict[str, object]:
    if type(raw) is not bytes or not raw or len(raw) > MAX_OPERATION_INPUT_BYTES:
        raise JevExecutionError("operation-input-size-or-type-invalid")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError, RecursionError) as exc:
        raise JevExecutionError("operation-input-invalid") from exc
    if type(value) is not dict or _canonical(value) != raw:
        raise JevExecutionError("operation-input-not-canonical")
    return value


def build_selector_request(
    *, operation_id: str, operation_input_bytes: bytes, legacy_control: Mapping[str, object] | None = None
) -> tuple[bytes, coverage.CompiledRequest | None]:
    """Pure request-construction preview; execution rebuilds and rechecks it inside the phase."""
    value = _operation_input(operation_input_bytes)
    if "candidate" in operation_id:
        if (
            legacy_control is not None
            or set(value) != {"schema", "query", "purpose", "facets", "candidates", "incumbent_order"}
            or value["schema"] != "coverage-jev-operation-input/1"
        ):
            raise JevExecutionError("coverage-operation-input-shape")
        typed_value = cast(dict[str, Any], value)
        try:
            compiled = coverage.compile_request(
                query=typed_value["query"],
                purpose=typed_value["purpose"],
                facets=typed_value["facets"],
                candidates=typed_value["candidates"],
                incumbent_order=typed_value["incumbent_order"],
            )
        except (coverage.CoverageError, TypeError) as exc:
            raise JevExecutionError("coverage-operation-input-invalid") from exc
        if len(compiled.body) > MAX_REQUEST_BYTES:
            raise JevExecutionError("request-size-or-type-invalid")
        return compiled.body, compiled

    if legacy_control is None or set(legacy_control) != {
        "service",
        "query",
        "canonical_pool",
        "frozen_v1_source_bytes",
        "current_service_source_bytes",
    }:
        raise JevExecutionError("original-v1-parser-input-shape")
    from dataclasses import asdict

    from scripts import coverage_legacy_control as legacy
    from slopsearx.service import SearchService, _rerank_text, _rerank_url

    query = cast(str, legacy_control["query"])
    pool = cast(tuple[Any, ...], legacy_control["canonical_pool"])
    frozen_source = cast(bytes, legacy_control["frozen_v1_source_bytes"])
    current_service_source = cast(bytes, legacy_control["current_service_source_bytes"])
    if type(query) is not str or type(pool) is not tuple or len(pool) < 2:
        raise JevExecutionError("original-v1-input-invalid")
    if _sha(frozen_source) != legacy.FROZEN_V1_SOURCE_SHA256:
        raise JevExecutionError("frozen-v1-source-pin-mismatch")
    if legacy._method_fingerprint(
        current_service_source
    ) != legacy.SERVICE_METHODS_PARITY_AST_SHA256 or not legacy._loaded_methods_match_reviewed_source(
        current_service_source
    ):
        raise JevExecutionError("original-v1-current-service-parity-mismatch")
    if not isinstance(legacy_control["service"], SearchService):
        raise JevExecutionError("original-v1-service-type-mismatch")
    if legacy_control["service"]._ctx.rerank_provider is not None:
        raise JevExecutionError("original-v1-service-provider-must-be-unconfigured")
    expected: dict[str, object] = {
        "schema": "coverage-jev-operation-input/1",
        "query": query,
        "candidates": [],
    }
    # Load and execute only the hard-SHA-pinned public V1 source before any
    # claim or dispatch. The request's model, rubric, and limits all come from
    # that source, rather than the current generator or mutable module constants.
    try:
        v1 = legacy._load_frozen_v1(frozen_source)
    except Exception as exc:
        raise JevExecutionError("frozen-v1-compiler-unavailable") from exc
    candidates = tuple(
        v1.RerankCandidate(
            id=f"c{index}",
            title=_rerank_text(result.title, 256),
            url=_rerank_url(result.url),
            snippet=_rerank_text(result.content, 1200),
        )
        for index, result in enumerate(pool[: v1.MAX_CANDIDATES])
    )
    expected["candidates"] = [asdict(candidate) for candidate in candidates]
    if value != expected:
        raise JevExecutionError("original-v1-input-binding-mismatch")
    body = {
        "model": v1.MODEL,
        "state": {"query": query, "candidates": [asdict(candidate) for candidate in candidates]},
        "questions": {
            candidate.id: {
                "type": "score",
                "instructions": v1.INSTRUCTIONS.format(id=candidate.id),
                "criteria": v1.LEVELS,
            }
            for candidate in candidates
        },
    }
    try:
        request = json.dumps(body, ensure_ascii=False).encode("utf-8")
    except UnicodeEncodeError as exc:
        raise JevExecutionError("original-v1-request-utf8") from exc
    if not 2 <= len(candidates) <= v1.MAX_CANDIDATES or len(query.encode("utf-8")) > v1.MAX_QUERY_BYTES:
        raise JevExecutionError("original-v1-input-outside-frozen-limits")
    if len(request) > v1.MAX_REQUEST_BYTES:
        raise JevExecutionError("original-v1-request-too-large")
    return request, None


async def _capture_frozen_v1_generated_request(
    *, frozen_source: bytes, query: str, operation_input_bytes: bytes
) -> bytes:
    """Run the pinned V1 request generator with only its HTTP client replaced.

    The frozen provider constructs its own request and calls the local fake's
    ``post`` method. That method captures exact content bytes and raises before
    any transport can exist. This verifies the manual projection byte-for-byte
    against the actual source generator before a paid dispatch.
    """
    from types import SimpleNamespace

    from scripts import coverage_legacy_control as legacy

    try:
        v1 = legacy._load_frozen_v1(frozen_source)
        operation = _operation_input(operation_input_bytes)
        if set(operation) != {"schema", "query", "candidates"} or type(operation["candidates"]) is not list:
            raise JevExecutionError("original-v1-dry-compile-input-shape")
        candidates = tuple(v1.RerankCandidate(**row) for row in operation["candidates"])
    except JevExecutionError:
        raise
    except Exception as exc:
        raise JevExecutionError("original-v1-dry-compile-input-invalid") from exc

    captured: list[bytes] = []

    class RequestCapturedError(Exception):
        pass

    class CaptureOnlyClient:
        def __init__(self, **_kwargs: object) -> None:
            pass

        async def __aenter__(self) -> CaptureOnlyClient:
            return self

        async def __aexit__(self, *_args: object) -> None:
            return None

        async def post(self, url: str, **kwargs: object) -> object:
            body = kwargs.get("content")
            if url != JEV_SYSTEMONE_URL or type(body) is not bytes:
                raise JevExecutionError("original-v1-dry-compile-request-shape")
            captured.append(body)
            raise RequestCapturedError()

    # Namespace-local replacement: the loaded frozen source sees this fake,
    # while the process-wide HTTPX module and every other caller are untouched.
    setattr(v1, "httpx", SimpleNamespace(AsyncClient=CaptureOnlyClient))
    try:
        await v1.JevReranker("offline-dry-compile-no-credential").rerank(query, candidates)
    except JevExecutionError:
        raise
    except Exception as exc:
        raise JevExecutionError("original-v1-dry-compile-failed") from exc
    if len(captured) != 1:
        raise JevExecutionError("original-v1-dry-compile-no-single-request")
    return captured[0]


async def execute_selector_call(
    *,
    prepared: core.PreparedStage,
    ledger: core.StudyRun,
    operation_id: str,
    operation_input_bytes: bytes,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    api_key: str,
    lease_root: str | os.PathLike[str],
    archive_root: str | os.PathLike[str],
    result_root: str | os.PathLike[str],
    transport: httpx.AsyncBaseTransport | None = None,
    legacy_control: Mapping[str, object] | None = None,
) -> CallResult:
    """Serialize use of a stage ledger and execute exactly one slot."""
    if type(ledger) is not core.StudyRun:
        raise JevExecutionError("study-ledger-required")
    lock = _LEDGER_LOCKS.get(ledger)
    if lock is None:
        lock = asyncio.Lock()
        _LEDGER_LOCKS[ledger] = lock
    async with lock:
        activity = _LEDGER_ACTIVITY.get(ledger)
        if activity is None:
            activity = _LedgerActivity()
            _LEDGER_ACTIVITY[ledger] = activity
        activity.active += 1
        activity.maximum_active = max(activity.maximum_active, activity.active)
        sequence = (
            prepared.operation_ids.index(operation_id) + 1
            if type(prepared) is core.PreparedStage and operation_id in prepared.operation_ids
            else 0
        )
        try:
            return await _execute_selector_call_once(
                prepared=prepared,
                ledger=ledger,
                operation_id=operation_id,
                operation_input_bytes=operation_input_bytes,
                permit_bytes=permit_bytes,
                expected_permit_sha256=expected_permit_sha256,
                api_key=api_key,
                lease_root=lease_root,
                archive_root=archive_root,
                result_root=result_root,
                transport=transport,
                legacy_control=legacy_control,
                serialized_operation_sequence=sequence,
                max_concurrent_operations_observed=activity.maximum_active,
            )
        finally:
            activity.active -= 1


async def _execute_selector_call_once(
    *,
    prepared: core.PreparedStage,
    ledger: core.StudyRun,
    operation_id: str,
    operation_input_bytes: bytes,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    api_key: str,
    lease_root: str | os.PathLike[str],
    archive_root: str | os.PathLike[str],
    result_root: str | os.PathLike[str],
    transport: httpx.AsyncBaseTransport | None = None,
    legacy_control: Mapping[str, object] | None = None,
    serialized_operation_sequence: int = 1,
    max_concurrent_operations_observed: int = 1,
) -> CallResult:
    """Dispatch one registered call exactly once; archive before parsing.

    ``api_key`` is an explicit, transient input. The caller owns its secure
    retrieval and must not place it in permit bytes, environment-backed
    fixtures, logs, or returned values. A supplied transport is a test seam;
    production use should pass ``None`` to get HTTPX's trust-env-disabled,
    no-redirect client.
    """
    started = time.monotonic()
    if type(prepared) is not core.PreparedStage or prepared.fresh_execution_authorized is not False:
        raise JevExecutionError("prepared-stage-required")
    if type(ledger) is not core.StudyRun:
        raise JevExecutionError("study-ledger-required")
    if type(operation_id) is not str or not _OP_ID.fullmatch(operation_id):
        raise JevExecutionError("operation-id-invalid")
    if operation_id not in prepared.operation_ids:
        raise JevExecutionError("operation-not-registered")
    input_sha = _sha(operation_input_bytes)
    parser_mode = "coverage" if "candidate" in operation_id else "original-v1"
    request_body, compiled_request = build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=operation_input_bytes,
        legacy_control=legacy_control,
    )
    if compiled_request is None:
        if legacy_control is None:
            raise JevExecutionError("original-v1-parser-input-shape")
        generated_body = await _capture_frozen_v1_generated_request(
            frozen_source=cast(bytes, legacy_control["frozen_v1_source_bytes"]),
            query=cast(str, legacy_control["query"]),
            operation_input_bytes=operation_input_bytes,
        )
        if generated_body != request_body:
            raise JevExecutionError("original-v1-generator-body-mismatch")
    if type(request_body) is not bytes or not request_body or len(request_body) > MAX_REQUEST_BYTES:
        raise JevExecutionError("request-size-or-type-invalid")
    request_sha = _sha(request_body)
    _permit(
        permit_bytes,
        expected_permit_sha256,
        prepared=prepared,
        operation_id=operation_id,
        operation_input_sha256=input_sha,
        request_sha256=request_sha,
        parser_mode=parser_mode,
    )
    if (
        type(api_key) is not str
        or not api_key.strip()
        or api_key != api_key.strip()
        or "\r" in api_key
        or "\n" in api_key
    ):
        raise JevExecutionError("explicit-api-key-invalid")
    try:
        api_key_bytes = api_key.encode("utf-8")
    except UnicodeEncodeError as exc:
        raise JevExecutionError("explicit-api-key-invalid") from exc
    if api_key_bytes and (api_key_bytes in operation_input_bytes or api_key_bytes in request_body):
        raise JevExecutionError("api-key-in-request-material")

    leases = _private_root(lease_root)
    archives = _private_root(archive_root)
    results = _private_root(result_root)
    claim = {
        "schema": "coverage-jev-one-shot-claim/1",
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "permit_sha256": expected_permit_sha256,
        "operation_input_sha256": input_sha,
        "request_body_sha256": request_sha,
    }
    _write_exclusive(leases, f"{prepared.stage_uuid}.{operation_id}.claim.json", _canonical(claim))
    try:
        ledger.begin(operation_id)
    except core.StudyError as exc:
        return CallResult(operation_id, "preflight-stop", request_sha, None, 0, None, None, None, str(exc))

    inner_transport = transport if transport is not None else httpx.AsyncHTTPTransport(trust_env=False, retries=0)
    dispatch_transport = _DispatchCountingTransport(inner_transport)

    bindings = {
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "request_body_sha256": request_sha,
        "source_revision": prepared.source_revision,
    }
    raw_body = bytearray()
    http_status: int | None = None
    archived: dict[str, object] | None = None
    state = "transport-or-http-failure"
    status = "transport-error"
    ranking: coverage.RankingResult | None = None
    native_status: str | None = None
    native_ordered_ids: tuple[str, ...] | None = None
    native_strict_json_compatible: bool | None = None
    native_strict_json_reason: str | None = None
    usage: tuple[int, int] | None = None
    terminal_reason: str | None = None
    timed_out = False

    async def exchange() -> None:
        nonlocal http_status, raw_body, archived, state, status, ranking, terminal_reason
        nonlocal timed_out, usage
        nonlocal native_status, native_ordered_ids, native_strict_json_compatible, native_strict_json_reason
        async with httpx.AsyncClient(
            transport=dispatch_transport,
            timeout=httpx.Timeout(SELECTOR_PHASE_SECONDS),
            follow_redirects=False,
            trust_env=False,
        ) as client:
            if time.monotonic() - started >= SELECTOR_PHASE_SECONDS:
                timed_out = True
                terminal_reason = "selector-phase-deadline-exceeded"
                status = "phase-deadline-exceeded"
            else:
                async with client.stream(
                    "POST",
                    JEV_SYSTEMONE_URL,
                    headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
                    content=request_body,
                ) as response:
                    http_status = response.status_code
                    async for chunk in response.aiter_bytes():
                        remaining = MAX_RESPONSE_BYTES - len(raw_body)
                        if len(chunk) > remaining:
                            raw_body.extend(chunk[: max(0, remaining)])
                            status = "response-too-large"
                            terminal_reason = "response-byte-limit"
                            if _contains_credential(bytes(raw_body), api_key_bytes):
                                raw_body.clear()
                                status = "credential-reflection-suppressed"
                                terminal_reason = "credential-reflection-suppressed"
                            break
                        raw_body.extend(chunk)
                        if _contains_credential(bytes(raw_body), api_key_bytes):
                            raw_body.clear()
                            status = "credential-reflection-suppressed"
                            terminal_reason = "credential-reflection-suppressed"
                            break
                    if terminal_reason is None and 200 <= http_status < 300:
                        state = "complete-success"
                        status = "complete"
                    else:
                        status = "http-failure" if terminal_reason is None else status
                        if terminal_reason is None:
                            terminal_reason = "http-status-not-success"

        if time.monotonic() - started > SELECTOR_PHASE_SECONDS:
            timed_out = True
            terminal_reason = "selector-phase-deadline-exceeded"
            status = "phase-deadline-exceeded"
        complete = state == "complete-success" and terminal_reason is None
        archived = receipts.archive_response(
            Path(archives),
            bindings=bindings,
            complete=complete,
            status="complete" if complete else status,
            http_status=http_status,
            response_body=bytes(raw_body),
        )
        if not complete:
            return
        if compiled_request is not None:
            try:
                parsed = coverage.parse_response(bytes(raw_body), compiled_request)
                ranking = coverage.select(compiled_request, parsed)
                usage = (parsed.usage["input_tokens"], parsed.usage["output_tokens"])
                status = ranking.status
            except coverage.CoverageError:
                state = "complete-invalid-response"
                status = "complete-invalid-response"
                terminal_reason = "invalid-complete-response"
                usage = _observed_usage(bytes(raw_body))
        else:
            # Re-enter the original V1 parser through its offline exact-byte
            # bridge. It validates its internally generated request digest and
            # does not consult credentials or open a provider transport.
            try:
                if legacy_control is None:
                    raise JevExecutionError("original-v1-parser-input-shape")
                replayed, legacy_status, legacy_receipt = await replay_original_v1_response(
                    archive_root=archives,
                    prepared=prepared,
                    operation_input_bytes=operation_input_bytes,
                    permit_bytes=permit_bytes,
                    expected_permit_sha256=expected_permit_sha256,
                    operation_id=operation_id,
                    bindings=bindings,
                    receipt_sha256=cast(str, archived["receipt_sha256"]),
                    service=cast(Any, legacy_control["service"]),
                    query=cast(str, legacy_control["query"]),
                    canonical_pool=cast(tuple[Any, ...], legacy_control["canonical_pool"]),
                    frozen_v1_source_bytes=cast(bytes, legacy_control["frozen_v1_source_bytes"]),
                    current_service_source_bytes=cast(bytes, legacy_control["current_service_source_bytes"]),
                    expected_v1_request_sha256=request_sha,
                )
                native_status = legacy_status
                native_strict_json_compatible = legacy_receipt.current_strict_json_compatible
                native_strict_json_reason = legacy_receipt.current_strict_json_reason
                original_pool = cast(tuple[Any, ...], legacy_control["canonical_pool"])
                global_id_by_object = {id(item): f"c{index}" for index, item in enumerate(original_pool)}
                if len(global_id_by_object) != len(original_pool) or any(
                    id(item) not in global_id_by_object for item in replayed
                ):
                    raise JevExecutionError("original-v1-output-object-binding")
                replayed_ids = tuple(global_id_by_object[id(item)] for item in replayed)
                if legacy_receipt.ordered_ids is not None and replayed_ids != legacy_receipt.ordered_ids:
                    raise JevExecutionError("original-v1-output-order-binding")
                native_ordered_ids = replayed_ids
                if (
                    legacy_receipt.usage_state == "known"
                    and type(legacy_receipt.input_tokens) is int
                    and type(legacy_receipt.output_tokens) is int
                ):
                    usage = (legacy_receipt.input_tokens, legacy_receipt.output_tokens)
                    if legacy_status == "applied":
                        status = "complete-original-v1"
                    else:
                        state = "complete-invalid-response"
                        terminal_reason = "original-v1-fallback-terminal"
                        status = "complete-invalid-response"
                else:
                    terminal_reason = "unknown-usage-terminal"
                    status = "unknown-usage-terminal"
            except Exception:
                # Do not disclose exception text, which could contain request
                # details. Exact raw response bytes remain durably archived.
                terminal_reason = "original-v1-parse-failed"
                status = "complete-invalid-response"

    try:
        try:
            async with asyncio.timeout(SELECTOR_PHASE_SECONDS):
                await exchange()
        except asyncio.CancelledError:
            terminal_reason = "external-cancellation"
            status = "partial-or-interrupted"
            try:
                if archived is None:
                    archived = receipts.archive_response(
                        Path(archives),
                        bindings=bindings,
                        complete=False,
                        status=status,
                        http_status=http_status,
                        response_body=bytes(raw_body),
                    )
                if ledger.in_flight == operation_id:
                    try:
                        ledger.external_failure(operation_id, state="partial-or-interrupted")
                    except core.StudyError:
                        pass
            finally:
                raise
        except (receipts.ReceiptError, JevExecutionError, core.StudyError) as exc:
            terminal_reason = type(exc).__name__
            status = "archive-or-ledger-failure"
        except Exception:
            timed_out = time.monotonic() - started >= SELECTOR_PHASE_SECONDS
            terminal_reason = "selector-phase-deadline-exceeded" if timed_out else "transport-or-http-failure"
            status = "phase-deadline-exceeded" if timed_out else "transport-error"
            if archived is None:
                archived = receipts.archive_response(
                    Path(archives),
                    bindings=bindings,
                    complete=False,
                    status=status,
                    http_status=http_status,
                    response_body=bytes(raw_body),
                )
        if terminal_reason is not None and ledger.in_flight == operation_id:
            try:
                failure_state = (
                    "transport-or-http-failure"
                    if status in {"transport-error", "http-failure"}
                    else "partial-or-interrupted"
                )
                ledger.external_failure(operation_id, state=failure_state)
            except core.StudyError:
                # The ledger raises after it records a terminal row and appends
                # every subsequent registered slot as uninvoked.
                pass
    except core.StudyError as exc:
        terminal_reason = str(exc)
        status = "terminal-ledger-stop"

    elapsed_before_receipt = time.monotonic() - started
    if elapsed_before_receipt > SELECTOR_PHASE_SECONDS:
        timed_out = True
        terminal_reason = "selector-phase-deadline-exceeded"
        status = "phase-deadline-exceeded"
    result_doc = {
        "schema": FINAL_RECEIPT_SCHEMA,
        "authority": "provisional-only-terminal-inventory-controls-acceptance",
        "provisional": True,
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "status": status,
        "terminal_reason": terminal_reason,
        "source_revision": prepared.source_revision,
        "source_closure_sha256": prepared.pins.get("qualified_source_closure"),
        "protocol_sha256": prepared.pins.get("protocol"),
        "permit_sha256": expected_permit_sha256,
        "operation_input_sha256": input_sha,
        "request_body_sha256": request_sha,
        "response_sha256": _sha(bytes(raw_body)) if archived is not None else None,
        "response_bytes": len(raw_body),
        "request_bytes": len(request_body),
        "provider_dispatch_count": dispatch_transport.dispatch_count,
        "input_tokens_observed": usage[0] if usage is not None else None,
        "output_tokens_observed": usage[1] if usage is not None else None,
        "serialized_operation_sequence": serialized_operation_sequence,
        "max_concurrent_operations_observed": max_concurrent_operations_observed,
        "archive_receipt_sha256": archived["receipt_sha256"] if archived is not None else None,
        "phase_elapsed_ms_before_receipt": int(elapsed_before_receipt * 1000),
        "phase_deadline_s": SELECTOR_PHASE_SECONDS,
        "timed_out": timed_out,
        "provider_dispatch": dispatch_transport.dispatch_count > 0,
        "parser_mode": parser_mode,
        "ranking_status": ranking.status if ranking is not None else None,
        "ordered_ids": (
            list(ranking.ordered_ids)
            if ranking is not None
            else list(native_ordered_ids)
            if native_ordered_ids is not None
            else None
        ),
        "native_status": native_status,
        "native_ordered_ids": list(native_ordered_ids) if native_ordered_ids is not None else None,
        "native_strict_json_compatible": native_strict_json_compatible,
        "native_strict_json_reason": native_strict_json_reason,
        "usage_state": "known" if usage is not None else "unknown-or-not-parsed",
    }
    result_sha = _write_exclusive(results, f"{prepared.stage_uuid}.{operation_id}.result.json", _canonical(result_doc))
    if time.monotonic() - started > SELECTOR_PHASE_SECONDS:
        terminal_reason = "selector-phase-deadline-exceeded"
        status = "phase-deadline-exceeded"
        timed_out = True
        correction = {
            "schema": "coverage-jev-operation-correction/1",
            "stage_uuid": prepared.stage_uuid,
            "operation_id": operation_id,
            "result_receipt_sha256": result_sha,
            "status": status,
            "terminal_reason": terminal_reason,
            "timed_out": True,
            "acceptance_authority": "terminal-inventory-only",
        }
        _write_exclusive(
            results,
            f"{prepared.stage_uuid}.{operation_id}.correction.json",
            _canonical(correction),
        )
    # A receipt cannot truthfully contain its own final-fsync duration. Measure
    # immediately after the result and any deadline-correction receipt are
    # durable; close_stage later binds this observation into terminal inventory.
    observed_elapsed_ms = max(0, math.ceil((time.monotonic() - started) * 1000))
    observation = {
        "schema": OBSERVATION_SCHEMA,
        "stage_uuid": prepared.stage_uuid,
        "operation_id": operation_id,
        "result_receipt_sha256": result_sha,
        "archive_receipt_sha256": archived["receipt_sha256"] if archived is not None else None,
        "elapsed_ms_through_final_receipt_fsync": observed_elapsed_ms,
        "provider_dispatch_count": dispatch_transport.dispatch_count,
        "request_bytes": len(request_body),
        "response_bytes": len(raw_body),
        "usage_state": "known" if usage is not None else "unknown-or-not-parsed",
        "input_tokens_observed": usage[0] if usage is not None else None,
        "output_tokens_observed": usage[1] if usage is not None else None,
        "serialized_operation_sequence": serialized_operation_sequence,
        "max_concurrent_operations_observed": max_concurrent_operations_observed,
        "serialization_scope": "same-study-ledger-lock",
    }
    _LEDGER_RESOURCE_OBSERVATIONS.setdefault(ledger, {})[operation_id] = observation
    if ledger.in_flight == operation_id:
        if state == "complete-invalid-response" and usage is not None and not timed_out:
            try:
                ledger.complete(
                    operation_id,
                    input_tokens=usage[0],
                    output_tokens=usage[1],
                    response_state="complete-invalid-response",
                )
            except core.StudyError:
                pass
        elif terminal_reason is not None or usage is None:
            try:
                ledger.external_failure(operation_id, state="partial-or-interrupted")
            except core.StudyError:
                pass
        else:
            try:
                ledger.complete(operation_id, input_tokens=usage[0], output_tokens=usage[1])
            except core.StudyError as exc:
                terminal_reason = str(exc)
                status = "terminal-ledger-stop"
    return CallResult(
        operation_id,
        status,
        request_sha,
        _sha(bytes(raw_body)) if archived is not None else None,
        len(raw_body),
        cast(str, archived["receipt_sha256"]) if archived else None,
        result_sha,
        ranking,
        terminal_reason,
        native_status,
        native_ordered_ids,
        native_strict_json_compatible,
        native_strict_json_reason,
        observed_elapsed_ms,
        dispatch_transport.dispatch_count,
        len(request_body),
        usage[0] if usage is not None else None,
        usage[1] if usage is not None else None,
        serialized_operation_sequence,
        max_concurrent_operations_observed,
    )


def close_stage(
    *,
    prepared: core.PreparedStage,
    ledger: core.StudyRun,
    result_root: str | os.PathLike[str],
) -> dict[str, object]:
    """Durably write exact registered operation accounting, including uninvoked slots."""
    root = _private_root(result_root)
    closure = ledger.close()
    if tuple(row.operation_id for row in closure.rows) != prepared.operation_ids:
        raise JevExecutionError("terminal-inventory-binding-mismatch")
    resource_observations = _terminal_resource_observations(root, prepared, ledger)
    body = _canonical(
        {
            "schema": INVENTORY_SCHEMA,
            "stage_uuid": prepared.stage_uuid,
            "source_revision": prepared.source_revision,
            "registration_sha256": prepared.registration_sha256,
            "protocol_sha256": prepared.pins.get("protocol"),
            "source_closure_sha256": prepared.pins.get("qualified_source_closure"),
            "status": closure.status,
            "all_calls_complete": closure.all_calls_complete,
            "input_tokens": closure.input_tokens,
            "output_tokens": closure.output_tokens,
            "operations": [{"operation_id": row.operation_id, "state": row.state} for row in closure.rows],
            "resource_observations": resource_observations,
        }
    )
    digest = _write_exclusive(root, f"{prepared.stage_uuid}.terminal-inventory.json", body)
    return {
        "sha256": digest,
        "bytes": len(body),
        "status": closure.status,
        "all_calls_complete": closure.all_calls_complete,
    }


def _terminal_resource_observations(
    root: Path, prepared: core.PreparedStage, ledger: core.StudyRun
) -> list[dict[str, object]]:
    """Persist in-memory post-fsync measurements, bound to durable result receipts."""
    observations: list[dict[str, object]] = []
    measured = _LEDGER_RESOURCE_OBSERVATIONS.get(ledger, {})
    if set(measured) - set(prepared.operation_ids):
        raise JevExecutionError("terminal-observation-unregistered-operation")
    for index, operation_id in enumerate(prepared.operation_ids, start=1):
        observation = measured.get(operation_id)
        if observation is None:
            observations.append({"operation_id": operation_id, "status": "not-observed"})
            continue
        result_path = root / f"{prepared.stage_uuid}.{operation_id}.result.json"
        try:
            raw_result = receipts._read_private(result_path, max_bytes=65_536)
            result = json.loads(raw_result.decode("utf-8"), object_pairs_hook=_unique_pairs)
        except Exception as exc:
            raise JevExecutionError("terminal-observation-result-missing-or-invalid") from exc
        if type(result) is not dict or _canonical(result) != raw_result:
            raise JevExecutionError("terminal-observation-result-not-canonical")
        elapsed = observation.get("elapsed_ms_through_final_receipt_fsync")
        dispatch_count = observation.get("provider_dispatch_count")
        request_bytes = observation.get("request_bytes")
        response_bytes = observation.get("response_bytes")
        input_tokens = observation.get("input_tokens_observed")
        output_tokens = observation.get("output_tokens_observed")
        sequence = observation.get("serialized_operation_sequence")
        max_concurrent = observation.get("max_concurrent_operations_observed")
        expected_observation = {
            "schema": OBSERVATION_SCHEMA,
            "stage_uuid": prepared.stage_uuid,
            "operation_id": operation_id,
            "result_receipt_sha256": _sha(raw_result),
            "archive_receipt_sha256": result.get("archive_receipt_sha256"),
            "elapsed_ms_through_final_receipt_fsync": elapsed,
            "provider_dispatch_count": result.get("provider_dispatch_count"),
            "request_bytes": result.get("request_bytes"),
            "response_bytes": result.get("response_bytes"),
            "usage_state": result.get("usage_state"),
            "input_tokens_observed": result.get("input_tokens_observed"),
            "output_tokens_observed": result.get("output_tokens_observed"),
            "serialized_operation_sequence": result.get("serialized_operation_sequence"),
            "max_concurrent_operations_observed": result.get("max_concurrent_operations_observed"),
            "serialization_scope": "same-study-ledger-lock",
        }
        if (
            observation != expected_observation
            or type(elapsed) is not int
            or elapsed < 0
            or type(dispatch_count) is not int
            or dispatch_count not in {0, 1}
            or type(request_bytes) is not int
            or request_bytes < 0
            or type(response_bytes) is not int
            or response_bytes < 0
            or (input_tokens is not None and (type(input_tokens) is not int or input_tokens < 0))
            or (output_tokens is not None and (type(output_tokens) is not int or output_tokens < 0))
            or (input_tokens is None) != (output_tokens is None)
            or observation.get("usage_state") != ("known" if input_tokens is not None else "unknown-or-not-parsed")
            or type(sequence) is not int
            or sequence != index
            or type(max_concurrent) is not int
            or max_concurrent != 1
            or result.get("stage_uuid") != prepared.stage_uuid
            or result.get("operation_id") != operation_id
            or result.get("source_revision") != prepared.source_revision
            or result.get("protocol_sha256") != prepared.pins.get("protocol")
            or result.get("source_closure_sha256") != prepared.pins.get("qualified_source_closure")
            or result.get("provider_dispatch") is not (dispatch_count > 0)
            or result.get("response_bytes") != response_bytes
            or result.get("request_bytes") != request_bytes
            or result.get("input_tokens_observed") != input_tokens
            or result.get("output_tokens_observed") != output_tokens
            or result.get("serialized_operation_sequence") != sequence
            or result.get("max_concurrent_operations_observed") != max_concurrent
        ):
            raise JevExecutionError("terminal-observation-binding-mismatch")
        observations.append(
            {
                "operation_id": operation_id,
                "status": "observed",
                "result_receipt_sha256": _sha(raw_result),
                **{
                    key: value
                    for key, value in observation.items()
                    if key not in {"schema", "stage_uuid", "result_receipt_sha256"}
                },
            }
        )
    return observations


def replay_candidate_response(
    *,
    archive_root: str | os.PathLike[str],
    prepared: core.PreparedStage,
    operation_input_bytes: bytes,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    operation_id: str,
    bindings: Mapping[str, str],
    receipt_sha256: str,
    compiled_request: coverage.CompiledRequest,
) -> coverage.RankingResult:
    """Verify exact archive bytes and run the unchanged coverage parser/selector."""
    if type(prepared) is not core.PreparedStage or operation_id not in prepared.operation_ids:
        raise JevExecutionError("candidate-replay-stage-binding")
    request_body, rebuilt = build_selector_request(
        operation_id=operation_id, operation_input_bytes=operation_input_bytes
    )
    if rebuilt is None or rebuilt.body != compiled_request.body or _sha(request_body) != _sha(compiled_request.body):
        raise JevExecutionError("candidate-replay-input-binding-mismatch")
    _permit(
        permit_bytes,
        expected_permit_sha256,
        prepared=prepared,
        operation_id=operation_id,
        operation_input_sha256=_sha(operation_input_bytes),
        request_sha256=_sha(request_body),
        parser_mode="coverage",
    )
    if (
        bindings.get("stage_uuid") != prepared.stage_uuid
        or bindings.get("operation_id") != operation_id
        or bindings.get("source_revision") != prepared.source_revision
    ):
        raise JevExecutionError("candidate-replay-stage-binding")
    try:
        receipt, body = receipts.replay_response(
            Path(archive_root), expected_bindings=dict(bindings), expected_receipt_sha256=receipt_sha256
        )
    except receipts.ReceiptError as exc:
        raise JevExecutionError("candidate-response-archive-invalid") from exc
    if _sha(compiled_request.body) != receipt["request_body_sha256"]:
        raise JevExecutionError("candidate-replay-request-binding-mismatch")
    try:
        parsed = coverage.parse_response(body, compiled_request)
        return coverage.select(compiled_request, parsed)
    except coverage.CoverageError:
        return coverage.RankingResult(compiled_request.incumbent_order, "fallback", "invalid_response", {})


async def replay_original_v1_response(
    *,
    archive_root: str | os.PathLike[str],
    prepared: core.PreparedStage,
    operation_input_bytes: bytes,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    operation_id: str,
    bindings: Mapping[str, str],
    receipt_sha256: str,
    service: SearchService,
    query: str,
    canonical_pool: tuple[SearchResult, ...],
    frozen_v1_source_bytes: bytes,
    current_service_source_bytes: bytes,
    expected_v1_request_sha256: str,
) -> tuple[list[SearchResult], str, LegacyControlReceipt]:
    """Verify archive bytes then delegate only to the frozen original V1 helper."""
    if type(prepared) is not core.PreparedStage or operation_id not in prepared.operation_ids:
        raise JevExecutionError("v1-replay-stage-binding")
    legacy_control = {
        "service": service,
        "query": query,
        "canonical_pool": canonical_pool,
        "frozen_v1_source_bytes": frozen_v1_source_bytes,
        "current_service_source_bytes": current_service_source_bytes,
    }
    request_body, _compiled = build_selector_request(
        operation_id=operation_id,
        operation_input_bytes=operation_input_bytes,
        legacy_control=legacy_control,
    )
    if _sha(request_body) != expected_v1_request_sha256:
        raise JevExecutionError("v1-replay-input-binding-mismatch")
    _permit(
        permit_bytes,
        expected_permit_sha256,
        prepared=prepared,
        operation_id=operation_id,
        operation_input_sha256=_sha(operation_input_bytes),
        request_sha256=_sha(request_body),
        parser_mode="original-v1",
    )
    if (
        bindings.get("stage_uuid") != prepared.stage_uuid
        or bindings.get("operation_id") != operation_id
        or bindings.get("source_revision") != prepared.source_revision
    ):
        raise JevExecutionError("v1-replay-stage-binding")
    try:
        receipt, body = receipts.replay_response(
            Path(archive_root), expected_bindings=dict(bindings), expected_receipt_sha256=receipt_sha256
        )
    except receipts.ReceiptError as exc:
        raise JevExecutionError("v1-response-archive-invalid") from exc
    if receipt["request_body_sha256"] != expected_v1_request_sha256:
        raise JevExecutionError("v1-replay-request-binding-mismatch")
    from scripts import coverage_legacy_control as legacy

    return await legacy.replay_legacy_v1_control(
        service,
        query,
        canonical_pool,
        frozen_v1_source_bytes=frozen_v1_source_bytes,
        current_service_source_bytes=current_service_source_bytes,
        expected_request_sha256=expected_v1_request_sha256,
        expected_response_sha256=receipt["response_body_sha256"],
        archived_response_bytes=body,
    )
