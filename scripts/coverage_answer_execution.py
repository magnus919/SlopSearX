"""Bounded paired answerer dispatch for the proposed coverage study.

This is an injected transport seam, not a registration or admission authority.
It accepts a separately verified stage-bound permit and one-shot lease, makes
no environment or credential lookup, and remains unqualified until the whole
source/dependency/runtime path is independently reviewed.
"""

from __future__ import annotations

import asyncio
import hashlib
import ipaddress
import json
import os
import re
import stat
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Mapping, Protocol, Sequence
from urllib.parse import urlsplit

import httpx

from scripts import coverage_consumer_inputs as consumer
from scripts import intent_ranking_receipts as receipts

PROMPT_PATH = Path(__file__).resolve().parents[1] / "docs/experiments/evidence/EXP-100/answerer-prompt.txt"
PROMPT_SHA256 = "6b32ce442150ee69531ed5f440619b616ed08187d89dac7d756acadedbace425"
PERMIT_SCHEMA = "coverage-answer-stage-permit/1"
RESULT_SCHEMA = "coverage-answer-stage-result/1"
MAX_CALLS = 18
MAX_REQUEST_BYTES = 384_000
MAX_RESPONSE_BYTES = 2_000_000
MAX_OUTPUT_TOKENS = 8192
REQUEST_TIMEOUT_SECONDS = 30.0
MAX_STAGE_WALL_SECONDS = 28_800.0
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")
_OP_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")


class AnswerExecutionError(RuntimeError):
    """A bounded answer dispatch stopped; messages never include private data."""


@dataclass(frozen=True)
class AnswerTask:
    """One task with equal-budget prepared arm inputs and a pinned citation catalog."""

    task_id: str
    kind: str
    query: str
    purpose: str
    critical_checks: tuple[Mapping[str, str], ...]
    answers: tuple[consumer.AnswerInput, consumer.AnswerInput]
    catalog_bytes: bytes
    catalog_sha256: str


@dataclass(frozen=True)
class VerifiedAnswerPermit:
    """Typed result expected from an external source-bound admission verifier."""

    status: str
    stage_uuid: str
    source_revision: str
    protocol_sha256: str
    cohorts_sha256: str
    operation_manifest_sha256: str
    endpoint_sha256: str
    endpoint_security_mode: str
    operation_ids: tuple[str, ...]
    max_calls: int
    request_bytes_maximum: int
    response_bytes_maximum: int
    receipt_sha256: str


class PermitVerifier(Protocol):
    def verify(
        self, receipt_bytes: bytes, expected_receipt_sha256: str, bindings: Mapping[str, object]
    ) -> VerifiedAnswerPermit: ...


class OneShotLease(Protocol):
    def consume_once(
        self, permit: VerifiedAnswerPermit, *, operation_id: str, request_sha256: str
    ) -> Mapping[str, object]: ...


@dataclass(frozen=True)
class AnswerStageResult:
    status: str
    stage_uuid: str
    owned_http_calls: int
    operations: tuple[Mapping[str, object], ...]
    terminal_receipt_sha256: str
    usage_status: str = "unavailable"
    semantic_grade: bool = False


@dataclass(frozen=True)
class _Request:
    operation_id: str
    kind: str
    task_id: str
    arm: str | None
    body: bytes
    answer_input: consumer.AnswerInput | None
    task: AnswerTask | None


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise AnswerExecutionError("answer-json-invalid") from exc


def _strict_json(raw: bytes) -> object:
    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result = {}
        for key, value in items:
            if key in result:
                raise AnswerExecutionError("answer-json-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda _value: (_ for _ in ()).throw(AnswerExecutionError("answer-json-nonfinite")),
        )
    except AnswerExecutionError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as exc:
        raise AnswerExecutionError("answer-json-invalid") from exc


def _answer_payload(task: AnswerTask, answer: consumer.AnswerInput) -> dict[str, object]:
    if answer.arm not in {"w0", "candidate"}:
        raise AnswerExecutionError("answer-arm-invalid")
    if type(answer.body) is not bytes or _sha(answer.body) != answer.body_sha256:
        raise AnswerExecutionError("answer-input-pin-mismatch")
    payload = _strict_json(answer.body)
    if type(payload) is not dict or set(payload) != {
        "task",
        "cards",
        "opening_outcomes",
        "delivered_sources",
        "output_schema",
    }:
        raise AnswerExecutionError("answer-input-shape")
    task_payload = payload["task"]
    if type(task_payload) is not dict or set(task_payload) != {
        "task_id",
        "kind",
        "query",
        "purpose",
        "critical_checks",
    }:
        raise AnswerExecutionError("answer-task-shape")
    if (
        task_payload["task_id"] != task.task_id
        or task_payload["kind"] != task.kind
        or task_payload["query"] != task.query
        or task_payload["purpose"] != task.purpose
        or task_payload["critical_checks"] != list(task.critical_checks)
    ):
        raise AnswerExecutionError("answer-task-input-mismatch")
    return {
        "task_id": task_payload["task_id"],
        "kind": task_payload["kind"],
        "query": task_payload["query"],
        "purpose": task_payload["purpose"],
        "critical_checks": task_payload["critical_checks"],
        "cards": payload["cards"],
        "outcomes": payload["opening_outcomes"],
        "sources": payload["delivered_sources"],
        "schema": payload["output_schema"],
    }


def _chat_url(endpoint: str, *, allow_trusted_private_http: bool = False) -> tuple[str, str]:
    if type(endpoint) is not str or len(endpoint.encode("utf-8")) > 2048:
        raise AnswerExecutionError("answer-endpoint-invalid")
    parsed = urlsplit(endpoint)
    if (
        parsed.scheme not in {"https", "http"}
        or not parsed.hostname
        or parsed.username
        or parsed.password
        or parsed.query
        or parsed.fragment
    ):
        raise AnswerExecutionError("answer-endpoint-invalid")
    security_mode = "https-required"
    if parsed.scheme == "http":
        if not allow_trusted_private_http or not _is_trusted_private_http_host(parsed.hostname):
            raise AnswerExecutionError("answer-plaintext-endpoint-not-permitted")
        security_mode = "trusted-private-http"
    base = endpoint.rstrip("/")
    if base.endswith("/chat/completions"):
        return base, security_mode
    if base.endswith("/v1"):
        return base + "/chat/completions", security_mode
    return base + "/v1/chat/completions", security_mode


def _is_trusted_private_http_host(host: str) -> bool:
    normalized = host.rstrip(".").lower()
    if normalized == "localhost" or "." not in normalized:
        return bool(normalized) and normalized not in {".", ".."}
    try:
        address = ipaddress.ip_address(normalized)
    except ValueError:
        return False
    return address.is_loopback or address.is_private


def _prompt_bytes() -> bytes:
    try:
        raw = PROMPT_PATH.read_bytes()
    except OSError as exc:
        raise AnswerExecutionError("frozen-answer-prompt-unavailable") from exc
    if _sha(raw) != PROMPT_SHA256:
        raise AnswerExecutionError("frozen-answer-prompt-pin-mismatch")
    return raw


def _make_requests(tasks: Sequence[AnswerTask]) -> tuple[_Request, ...]:
    if type(tasks) not in {list, tuple} or len(tasks) != 8:
        raise AnswerExecutionError("answer-task-count")
    readiness = []
    for index in (1, 2):
        body = _canonical(
            {
                "model": "free",
                "messages": [
                    {"role": "system", "content": 'Return exactly the JSON object {"ready":true}.'},
                    {"role": "user", "content": "This is a synthetic endpoint readiness check, not study evidence."},
                ],
                "max_tokens": MAX_OUTPUT_TOKENS,
                "temperature": 0,
                "response_format": {"type": "json_object"},
            }
        )
        readiness.append(_Request(f"readiness-{index}", "readiness", f"neutral-{index}", None, body, None, None))
    prompt = _prompt_bytes().decode("utf-8")
    requests = list(readiness)
    seen = set()
    for task in tasks:
        if type(task) is not AnswerTask or task.task_id in seen:
            raise AnswerExecutionError("answer-task-duplicate-or-type")
        seen.add(task.task_id)
        if len(task.answers) != 2 or {row.arm for row in task.answers} != {"w0", "candidate"}:
            raise AnswerExecutionError("paired-answer-inputs-required")
        if _sha(task.catalog_bytes) != task.catalog_sha256:
            raise AnswerExecutionError("answer-catalog-pin-mismatch")
        for arm in ("w0", "candidate"):
            answer = next(row for row in task.answers if row.arm == arm)
            user = _canonical(_answer_payload(task, answer)).decode("utf-8")
            body = _canonical(
                {
                    "model": "free",
                    "messages": [{"role": "system", "content": prompt}, {"role": "user", "content": user}],
                    "max_tokens": MAX_OUTPUT_TOKENS,
                    "temperature": 0,
                    "response_format": {"type": "json_object"},
                }
            )
            requests.append(_Request(f"answer-{task.task_id}-{arm}", "answer", task.task_id, arm, body, answer, task))
    if len(requests) != MAX_CALLS:
        raise AnswerExecutionError("answer-operation-count")
    if any(not _OP_ID.fullmatch(row.operation_id) for row in requests):
        raise AnswerExecutionError("answer-operation-id-invalid")
    return tuple(requests)


def _request_manifest(
    requests: Sequence[_Request], *, endpoint_security_mode: str = "https-required"
) -> tuple[bytes, str]:
    if endpoint_security_mode not in {"https-required", "trusted-private-http"}:
        raise AnswerExecutionError("answer-endpoint-security-mode-invalid")
    operations = [
        {
            "slot": index,
            "operation_id": request.operation_id,
            "kind": request.kind,
            "task_id": request.task_id,
            "arm": request.arm,
            "request_body_sha256": _sha(request.body),
            "request_bytes": len(request.body),
        }
        for index, request in enumerate(requests, 1)
    ]
    if any(len(request.body) > MAX_REQUEST_BYTES for request in requests):
        raise AnswerExecutionError("answer-request-byte-cap")
    raw = _canonical({"endpoint_security_mode": endpoint_security_mode, "operations": operations})
    return raw, _sha(raw)


def _reported_usage(payload: Mapping[str, object]) -> tuple[str, dict[str, int]]:
    usage = payload.get("usage")
    if usage is None:
        return "unavailable", {}
    if type(usage) is not dict:
        return "invalid", {}
    observed: dict[str, int] = {}
    for name in ("prompt_tokens", "completion_tokens", "total_tokens"):
        if name not in usage:
            continue
        value = usage[name]
        if type(value) is not int or value < 0:
            return "invalid", {}
        observed[name] = value
    return ("reported", observed) if observed else ("unavailable", {})


def _verify_permit(
    *,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    verifier: PermitVerifier | None,
    bindings: Mapping[str, object],
) -> VerifiedAnswerPermit:
    if type(permit_bytes) is not bytes or not permit_bytes or len(permit_bytes) > 16_384:
        raise AnswerExecutionError("external-answer-permit-required")
    if type(expected_permit_sha256) is not str or not _SHA256.fullmatch(expected_permit_sha256):
        raise AnswerExecutionError("answer-permit-pin-invalid")
    if _sha(permit_bytes) != expected_permit_sha256:
        raise AnswerExecutionError("answer-permit-pin-mismatch")
    if verifier is None or not callable(getattr(verifier, "verify", None)):
        raise AnswerExecutionError("qualified-answer-permit-verifier-required")
    permit = verifier.verify(permit_bytes, expected_permit_sha256, bindings)
    if type(permit) is not VerifiedAnswerPermit or permit.status != "verified-admitted":
        raise AnswerExecutionError("answer-permit-not-admitted")
    expected = {
        "stage_uuid": bindings["stage_uuid"],
        "source_revision": bindings["source_revision"],
        "protocol_sha256": bindings["protocol_sha256"],
        "cohorts_sha256": bindings["cohorts_sha256"],
        "operation_manifest_sha256": bindings["operation_manifest_sha256"],
        "endpoint_sha256": bindings["endpoint_sha256"],
        "endpoint_security_mode": bindings["endpoint_security_mode"],
        "operation_ids": bindings["operation_ids"],
        "max_calls": MAX_CALLS,
        "request_bytes_maximum": MAX_REQUEST_BYTES,
        "response_bytes_maximum": MAX_RESPONSE_BYTES,
        "receipt_sha256": expected_permit_sha256,
    }
    if any(getattr(permit, key) != value for key, value in expected.items()):
        raise AnswerExecutionError("answer-permit-binding-mismatch")
    return permit


def _safe_root(value: str | os.PathLike[str]) -> Path:
    path = Path(value)
    try:
        info = path.lstat()
    except OSError as exc:
        raise AnswerExecutionError("answer-private-root-unavailable") from exc
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o700
        or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
    ):
        raise AnswerExecutionError("answer-private-root-unsafe")
    return path


def _write_once(root: Path, name: str, body: bytes) -> str:
    if "/" in name or name in {".", ".."}:
        raise AnswerExecutionError("answer-artifact-name-invalid")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(root / name, flags, 0o600)
    except FileExistsError as exc:
        raise AnswerExecutionError("answer-stage-slot-already-consumed") from exc
    except OSError as exc:
        raise AnswerExecutionError("answer-private-artifact-open-failed") from exc
    try:
        view = memoryview(body)
        while view:
            size = os.write(fd, view)
            if size <= 0:
                raise AnswerExecutionError("answer-private-artifact-short-write")
            view = view[size:]
        os.fsync(fd)
    finally:
        os.close(fd)
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    directory = os.open(root, flags)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)
    return _sha(body)


def _stage_times(started_utc: str, deadline_utc: str) -> tuple[float, float]:
    try:
        start = datetime.fromisoformat(started_utc.replace("Z", "+00:00"))
        end = datetime.fromisoformat(deadline_utc.replace("Z", "+00:00"))
    except (AttributeError, TypeError, ValueError) as exc:
        raise AnswerExecutionError("answer-stage-clock-invalid") from exc
    if start.tzinfo is None or end.tzinfo is None or (end - start).total_seconds() != MAX_STAGE_WALL_SECONDS:
        raise AnswerExecutionError("answer-stage-clock-bound")
    now = datetime.now(timezone.utc)
    if not start <= now < end:
        raise AnswerExecutionError("answer-stage-clock-inactive")
    return time.monotonic(), time.monotonic() + (end - now).total_seconds()


def _contains_secret(raw: bytes, secret: bytes) -> bool:
    if secret in raw:
        return True
    max_prefix = min(len(secret) - 1, len(raw))
    return any(raw.endswith(secret[:size]) for size in range(1, max_prefix + 1))


def _mark_uninvoked(rows: list[dict[str, object]], requests: Sequence[_Request], start: int, reason: str) -> None:
    for request in requests[start:]:
        rows.append(
            {
                "operation_id": request.operation_id,
                "task_id": request.task_id,
                "arm": request.arm,
                "status": "not-invoked-after-terminal-stop",
                "terminal_reason": reason,
                "usage_status": "unavailable",
            }
        )


async def execute_answer_stage(
    *,
    stage_uuid: str,
    source_revision: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    answer_manifest_sha256: str,
    stage_started_utc: str,
    stage_deadline_utc: str,
    tasks: Sequence[AnswerTask],
    endpoint: str,
    allow_trusted_private_http: bool = False,
    api_key: str,
    permit_bytes: bytes,
    expected_permit_sha256: str,
    permit_verifier: PermitVerifier | None,
    one_shot_lease: OneShotLease | None,
    lease_root: str | os.PathLike[str],
    archive_root: str | os.PathLike[str],
    result_root: str | os.PathLike[str],
    transport: httpx.AsyncBaseTransport | None = None,
) -> AnswerStageResult:
    """Execute 2 neutral checks and 16 paired answers once, in fixed order.

    The permit verifier and lease must be independently qualified. The injected
    HTTPX transport is intended for offline tests; production qualification
    must separately bind the actual endpoint, runtime and dependency closure.
    """
    if not _GIT_SHA.fullmatch(source_revision):
        raise AnswerExecutionError("answer-source-revision-invalid")
    for value in (protocol_sha256, cohorts_sha256, answer_manifest_sha256):
        if type(value) is not str or not _SHA256.fullmatch(value):
            raise AnswerExecutionError("answer-stage-pin-invalid")
    if type(stage_uuid) is not str:
        raise AnswerExecutionError("answer-stage-uuid-invalid")
    try:
        if str(uuid.UUID(stage_uuid)) != stage_uuid:
            raise ValueError("not canonical")
    except ValueError as exc:
        raise AnswerExecutionError("answer-stage-uuid-invalid") from exc
    if type(api_key) is not str or not api_key or api_key.strip() != api_key or "\r" in api_key or "\n" in api_key:
        raise AnswerExecutionError("explicit-answer-key-invalid")
    try:
        key_bytes = api_key.encode("utf-8")
    except UnicodeEncodeError as exc:
        raise AnswerExecutionError("explicit-answer-key-invalid") from exc

    requests = _make_requests(tasks)
    endpoint_url, endpoint_security_mode = _chat_url(endpoint, allow_trusted_private_http=allow_trusted_private_http)
    operation_manifest, operation_manifest_sha = _request_manifest(
        requests, endpoint_security_mode=endpoint_security_mode
    )
    if operation_manifest_sha != answer_manifest_sha256:
        raise AnswerExecutionError("answer-operation-manifest-pin-mismatch")
    endpoint_sha = _sha(_canonical({"endpoint": endpoint_url, "security_mode": endpoint_security_mode}))
    started, deadline = _stage_times(stage_started_utc, stage_deadline_utc)
    bindings = {
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "protocol_sha256": protocol_sha256,
        "cohorts_sha256": cohorts_sha256,
        "operation_manifest_sha256": operation_manifest_sha,
        "endpoint_sha256": endpoint_sha,
        "endpoint_security_mode": endpoint_security_mode,
        "operation_ids": tuple(row.operation_id for row in requests),
    }
    permit = _verify_permit(
        permit_bytes=permit_bytes,
        expected_permit_sha256=expected_permit_sha256,
        verifier=permit_verifier,
        bindings=bindings,
    )
    if one_shot_lease is None or not callable(getattr(one_shot_lease, "consume_once", None)):
        raise AnswerExecutionError("answer-one-shot-lease-required")
    if key_bytes in operation_manifest or any(key_bytes in request.body for request in requests):
        raise AnswerExecutionError("answer-key-in-request-material")

    leases = _safe_root(lease_root)
    archives = _safe_root(archive_root)
    results = _safe_root(result_root)
    # A stage claim prevents any second process from replaying this schedule.
    lease_sha = _write_once(
        leases,
        f"{stage_uuid}.answer-stage.claim.json",
        _canonical(
            {
                "schema": "coverage-answer-one-shot-claim/1",
                "stage_uuid": stage_uuid,
                "permit_sha256": expected_permit_sha256,
                "operation_manifest_sha256": operation_manifest_sha,
            }
        ),
    )
    rows: list[dict[str, object]] = []
    lease_receipts: list[str] = []
    owned_http_calls = 0
    client = httpx.AsyncClient(
        transport=transport,
        timeout=httpx.Timeout(REQUEST_TIMEOUT_SECONDS),
        follow_redirects=False,
        trust_env=False,
    )
    terminal_reason = None
    try:
        for index, request in enumerate(requests):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                terminal_reason = "answer-stage-deadline"
                _mark_uninvoked(rows, requests, index, terminal_reason)
                break
            if time.monotonic() - started >= MAX_STAGE_WALL_SECONDS:
                terminal_reason = "answer-stage-wall-limit"
                _mark_uninvoked(rows, requests, index, terminal_reason)
                break
            request_sha = _sha(request.body)
            try:
                lease_result = one_shot_lease.consume_once(
                    permit, operation_id=request.operation_id, request_sha256=request_sha
                )
            except Exception as exc:
                terminal_reason = "answer-operation-lease-failed"
                rows.append(
                    {
                        "operation_id": request.operation_id,
                        "status": terminal_reason,
                        "error_class": type(exc).__name__,
                        "usage_status": "unavailable",
                    }
                )
                _mark_uninvoked(rows, requests, index + 1, terminal_reason)
                break
            lease_receipt_sha = lease_result.get("receipt_sha256") if isinstance(lease_result, Mapping) else None
            if (
                not isinstance(lease_result, Mapping)
                or lease_result.get("status") != "consumed"
                or lease_result.get("stage_uuid") != stage_uuid
                or lease_result.get("operation_id") != request.operation_id
                or lease_result.get("request_sha256") != request_sha
                or type(lease_receipt_sha) is not str
                or not _SHA256.fullmatch(lease_receipt_sha)
            ):
                terminal_reason = "answer-operation-lease-receipt-invalid"
                rows.append(
                    {"operation_id": request.operation_id, "status": terminal_reason, "usage_status": "unavailable"}
                )
                _mark_uninvoked(rows, requests, index + 1, terminal_reason)
                break
            assert isinstance(lease_receipt_sha, str)
            lease_receipts.append(lease_receipt_sha)
            bindings_for_receipt = {
                "stage_uuid": stage_uuid,
                "operation_id": request.operation_id,
                "request_body_sha256": request_sha,
                "source_revision": source_revision,
            }
            body = bytearray()
            credential_reflection = False
            status_code = None
            call_status = "transport-error"
            try:
                owned_http_calls += 1
                async with asyncio.timeout(min(REQUEST_TIMEOUT_SECONDS, remaining)):
                    async with client.stream(
                        "POST",
                        endpoint_url,
                        content=request.body,
                        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
                    ) as response:
                        status_code = response.status_code
                        async for chunk in response.aiter_bytes(65_536):
                            space = MAX_RESPONSE_BYTES - len(body)
                            if len(chunk) > space:
                                body.extend(chunk[: max(space, 0)])
                                call_status = "response-too-large"
                                raise AnswerExecutionError("answer-response-byte-cap")
                            body.extend(chunk)
                            if _contains_secret(bytes(body), key_bytes):
                                body.clear()
                                credential_reflection = True
                                call_status = "credential-reflection-suppressed"
                                raise AnswerExecutionError("answer-credential-echo-suppressed")
                if status_code is None or not 200 <= status_code < 300:
                    call_status = "http-error"
                    raise AnswerExecutionError("answer-http-status")
                call_status = "complete"
            except Exception as exc:
                if credential_reflection or _contains_secret(bytes(body), key_bytes):
                    body.clear()
                    call_status = "credential-reflection-suppressed"
                    terminal_reason = "credential-reflection-suppressed"
                elif call_status == "response-too-large":
                    terminal_reason = "answer-response-byte-cap"
                elif call_status == "complete":
                    terminal_reason = "answer-response-transport-interrupted"
                    call_status = "partial-response"
                elif call_status == "http-error":
                    terminal_reason = "answer-http-error"
                else:
                    terminal_reason = "answer-transport-error"
                archived = receipts.archive_response(
                    archives,
                    bindings=bindings_for_receipt,
                    complete=False,
                    status=call_status,
                    http_status=status_code,
                    response_body=bytes(body),
                )
                rows.append(
                    {
                        "operation_id": request.operation_id,
                        "task_id": request.task_id,
                        "arm": request.arm,
                        "status": call_status,
                        "terminal_reason": terminal_reason,
                        "http_status": status_code,
                        "response_bytes": len(body),
                        "archive_receipt_sha256": archived["receipt_sha256"],
                        "usage_status": "unavailable",
                        "error_class": type(exc).__name__,
                    }
                )
                _mark_uninvoked(rows, requests, index + 1, terminal_reason)
                break

            # Exact successful bytes are durable before strict parsing or validation.
            archived = receipts.archive_response(
                archives,
                bindings=bindings_for_receipt,
                complete=True,
                status="complete",
                http_status=status_code,
                response_body=bytes(body),
            )
            row: dict[str, object] = {
                "operation_id": request.operation_id,
                "task_id": request.task_id,
                "arm": request.arm,
                "status": "response-archived",
                "http_status": status_code,
                "response_bytes": len(body),
                "response_sha256": _sha(bytes(body)),
                "archive_receipt_sha256": archived["receipt_sha256"],
                "usage_status": "unavailable",
            }
            try:
                payload = _strict_json(bytes(body))
                if type(payload) is not dict:
                    raise AnswerExecutionError("answer-envelope-shape")
                usage_status, provider_usage = _reported_usage(payload)
                row["usage_status"] = usage_status
                if provider_usage:
                    row["provider_usage"] = provider_usage
                choices = payload.get("choices")
                if type(choices) is not list or len(choices) != 1 or type(choices[0]) is not dict:
                    raise AnswerExecutionError("answer-choice-shape")
                message = choices[0].get("message")
                if type(message) is not dict or type(message.get("content")) is not str:
                    raise AnswerExecutionError("answer-content-shape")
                content = message["content"].encode("utf-8")
                if request.kind == "readiness":
                    if _strict_json(content) != {"ready": True}:
                        raise AnswerExecutionError("answer-readiness-rejected")
                    row["status"] = "readiness-complete"
                else:
                    assert request.answer_input is not None and request.task is not None
                    parsed = _strict_json(content)
                    if type(parsed) is not dict:
                        raise AnswerExecutionError("answer-output-shape")
                    critical_ids = [check["id"] for check in request.task.critical_checks]
                    structurally_valid = consumer.validate_answer(
                        parsed, request.answer_input, critical_check_ids=critical_ids
                    )
                    restored = consumer.restore_answer_citations(
                        parsed,
                        request.answer_input,
                        critical_check_ids=critical_ids,
                        catalog_bytes=request.task.catalog_bytes,
                        expected_catalog_sha256=request.task.catalog_sha256,
                    )
                    answer_bytes = _canonical(restored)
                    answer_sha = _write_once(results, f"{stage_uuid}.{request.operation_id}.answer.json", answer_bytes)
                    row.update(
                        status="answer-structurally-valid",
                        answer_sha256=answer_sha,
                        structural_receipt_sha256=structurally_valid["answer_sha256"],
                    )
                if time.monotonic() >= deadline:
                    raise AnswerExecutionError("answer-completed-after-stage-deadline")
            except Exception as exc:
                terminal_reason = "answer-output-invalid-or-stage-expired"
                row.update(
                    status="invalid-answer-output",
                    terminal_reason=terminal_reason,
                    error_class=type(exc).__name__,
                )
                rows.append(row)
                _mark_uninvoked(rows, requests, index + 1, terminal_reason)
                break
            rows.append(row)
    finally:
        await client.aclose()

    successful = len(rows) == MAX_CALLS and all(
        row.get("status") in {"readiness-complete", "answer-structurally-valid"} for row in rows
    )
    usage_status = (
        "reported"
        if any(row.get("usage_status") == "reported" for row in rows)
        else "invalid"
        if any(row.get("usage_status") == "invalid" for row in rows)
        else "unavailable"
    )
    result_doc = {
        "schema": RESULT_SCHEMA,
        "status": "complete-structurally-valid-not-semantically-graded" if successful else "terminal-incomplete",
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "protocol_sha256": protocol_sha256,
        "cohorts_sha256": cohorts_sha256,
        "answer_manifest_sha256": answer_manifest_sha256,
        "endpoint_sha256": endpoint_sha,
        "endpoint_security_mode": endpoint_security_mode,
        "permit_sha256": expected_permit_sha256,
        "stage_claim_sha256": lease_sha,
        "operation_lease_receipt_sha256": lease_receipts,
        "owned_http_calls": owned_http_calls,
        "usage_status": usage_status,
        "semantic_grade": False,
        "terminal_reason": terminal_reason,
        "operations": rows,
    }
    if time.monotonic() >= deadline:
        result_doc["status"] = "terminal-incomplete"
        result_doc["terminal_reason"] = "answer-stage-deadline-before-final-receipt"
    final = _canonical(result_doc)
    final_sha = _write_once(results, f"{stage_uuid}.answer-terminal-inventory.json", final)
    if time.monotonic() >= deadline and successful:
        correction = _canonical(
            {
                "schema": "coverage-answer-stage-correction/1",
                "stage_uuid": stage_uuid,
                "terminal_receipt_sha256": final_sha,
                "status": "terminal-incomplete",
                "terminal_reason": "answer-stage-deadline-during-final-receipt-fsync",
                "acceptance_authority": "terminal-inventory-and-correction",
            }
        )
        _write_once(results, f"{stage_uuid}.answer-terminal-correction.json", correction)
        terminal_reason = "answer-stage-deadline-during-final-receipt-fsync"
        result_doc["status"] = "terminal-incomplete"
    return AnswerStageResult(
        status=str(result_doc["status"]),
        stage_uuid=stage_uuid,
        owned_http_calls=owned_http_calls,
        operations=tuple(rows),
        terminal_receipt_sha256=final_sha,
        usage_status=usage_status,
    )
