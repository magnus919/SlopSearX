"""Offline tests for the bounded source-capture seam."""

from __future__ import annotations

import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Mapping

import httpx
import pytest

from scripts import intent_ranking_receipts
from scripts.coverage_source_capture import (
    MAX_RESPONSE_BYTES,
    SourceCaptureError,
    VerifiedSourceCapturePermit,
    capture_sources_once,
)


def _canonical(value: object) -> bytes:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    return encoded.encode()


def _sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _protocol_bytes() -> bytes:
    path = Path(__file__).resolve().parents[1] / "docs/experiments/evidence/coverage-first-study/protocol.json"
    return path.read_bytes()


def _candidate_identity() -> bytes:
    return _canonical({"runtime": {"revision": "c" * 40, "model": "free"}})


def _manifest(
    protocol_bytes: bytes,
    sources: list[dict[str, object]],
    candidate_base_url: str = "https://capture.example",
    candidate_identity_bytes: bytes | None = None,
    cohorts_sha256: str | None = None,
    candidate_identity_sha256: str | None = None,
) -> bytes:
    protocol = json.loads(protocol_bytes)
    candidate_identity_bytes = candidate_identity_bytes or _candidate_identity()
    return _canonical(
        {
            "schema": "coverage-source-capture-manifest/1",
            "stage": "development",
            "stage_uuid": "f611a79a-9eef-46f4-b211-3d4d394e9e21",
            "source_revision": "8f3577d022e2d98fcd405d915b5c3b9b16e899bf",
            "protocol_sha256": _sha(protocol_bytes),
            "cohorts_sha256": cohorts_sha256 or protocol["cohorts_sha256"],
            "candidate_identity_sha256": candidate_identity_sha256 or _sha(candidate_identity_bytes),
            "candidate_endpoint_sha256": _sha(candidate_base_url.rstrip("/ ").encode()),
            "sources": sources,
        }
    )


def _source(task: str, source_id: str, index: int, url: str | None) -> dict[str, object]:
    return {
        "task_id": task,
        "source_id": source_id,
        "result_index": index,
        "url": url,
        "title": f"title {source_id}",
        "engine": "wikipedia",
    }


class _Verifier:
    def verify(self, manifest_bytes: bytes, receipt_bytes: bytes, expected_receipt_sha256: str):
        assert receipt_bytes == b"external-permit-receipt"
        assert expected_receipt_sha256 == _sha(receipt_bytes)
        manifest = json.loads(manifest_bytes)
        return VerifiedSourceCapturePermit(
            status="verified-admitted",
            scope="source-capture",
            stage_uuid=manifest["stage_uuid"],
            manifest_sha256=_sha(manifest_bytes),
            protocol_sha256=manifest["protocol_sha256"],
            source_revision=manifest["source_revision"],
            cohorts_sha256=manifest["cohorts_sha256"],
            candidate_identity_sha256=manifest["candidate_identity_sha256"],
            candidate_endpoint_sha256=manifest["candidate_endpoint_sha256"],
            source_count=len(manifest["sources"]),
            max_owned_calls=641,
            max_health_calls=1,
            max_scrape_calls=640,
            timeout_seconds=30.0,
            response_bytes=2_000_000,
            source_context_characters=8_000,
            receipt_sha256=expected_receipt_sha256,
        )


class _Lease:
    def __init__(self) -> None:
        self.calls = 0

    def consume_once(self, permit: VerifiedSourceCapturePermit) -> Mapping[str, object]:
        self.calls += 1
        return {"status": "consumed", "stage_uuid": permit.stage_uuid, "receipt_sha256": "b" * 64}


def _root(tmp_path: Path) -> Path:
    path = tmp_path / "private-receipts"
    path.mkdir(mode=0o700)
    os.chmod(path, 0o700)
    assert stat.S_IMODE(path.stat().st_mode) == 0o700
    return path


async def _capture(
    *,
    tmp_path: Path,
    transport: httpx.AsyncBaseTransport,
    sources: list[dict[str, object]],
    candidate_base_url: str = "https://capture.example",
    token: str | None = "test-secret-token",
    clock=lambda: 100.0,
    start: float = 100.0,
    deadline: float = 200.0,
    verifier: _Verifier | None = None,
    lease: _Lease | None = None,
    manifest_endpoint_sha256: str | None = None,
    candidate_identity_bytes: bytes | None = None,
    manifest_cohorts_sha256: str | None = None,
    manifest_candidate_identity_sha256: str | None = None,
):
    protocol_bytes = _protocol_bytes()
    candidate_identity_bytes = candidate_identity_bytes or _candidate_identity()
    manifest_bytes = _manifest(
        protocol_bytes,
        sources,
        candidate_base_url,
        candidate_identity_bytes,
        manifest_cohorts_sha256,
        manifest_candidate_identity_sha256,
    )
    if manifest_endpoint_sha256 is not None:
        manifest_obj = json.loads(manifest_bytes)
        manifest_obj["candidate_endpoint_sha256"] = manifest_endpoint_sha256
        manifest_bytes = _canonical(manifest_obj)
    permit_bytes = b"external-permit-receipt"
    return await capture_sources_once(
        protocol_bytes=protocol_bytes,
        source_manifest_bytes=manifest_bytes,
        candidate_identity_bytes=candidate_identity_bytes,
        candidate_base_url=candidate_base_url,
        operator_token=token,
        permit_receipt_bytes=permit_bytes,
        expected_permit_receipt_sha256=_sha(permit_bytes),
        permit_verifier=verifier or _Verifier(),
        one_shot_lease=lease or _Lease(),
        receipt_root=_root(tmp_path),
        transport=transport,
        stage_started_monotonic=start,
        stage_deadline_monotonic=deadline,
        clock=clock,
    )


def _response(markdown: str) -> bytes:
    return json.dumps(
        {"success": True, "data": {"markdown": markdown}}, ensure_ascii=False, separators=(",", ":")
    ).encode()


def _healthy_response() -> httpx.Response:
    return httpx.Response(200, json={"status": "ok", "runtime": {"revision": "c" * 40, "model": "free"}})


@pytest.mark.asyncio
async def test_reflected_operator_key_is_not_persisted_in_error_body(tmp_path: Path) -> None:
    token = "synthetic-operator-secret-never-persist"
    calls = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(403, content=f"Rejected key: {token}".encode())

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        token=token,
        sources=[
            _source("task-a", "source-a", 1, "https://docs.example/a"),
            _source("task-a", "source-b", 2, "https://docs.example/b"),
        ],
    )
    assert result.status == "terminal-incomplete"
    assert len(calls) == 2
    assert result.private_inventory[0]["failure_code"] == "credential-reflection-suppressed"
    assert result.private_inventory[1]["status"] == "not_attempted_terminal_failure"
    for path in result.receipt_directory.parent.rglob("*"):
        if path.is_file():
            assert token.encode() not in path.read_bytes()


@pytest.mark.asyncio
async def test_credential_prefix_at_response_cap_is_suppressed(tmp_path: Path) -> None:
    token = "synthetic-boundary-operator-secret"
    prefix = token[:12].encode()
    body = b"x" * (MAX_RESPONSE_BYTES - len(prefix)) + token.encode()

    def handler(request: httpx.Request) -> httpx.Response:
        if request.method == "GET":
            return _healthy_response()
        return httpx.Response(403, content=body)

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        token=token,
        sources=[_source("task-a", "source-a", 1, "https://docs.example/a")],
    )
    assert result.status == "terminal-incomplete"
    assert result.private_inventory[0]["failure_code"] == "credential-reflection-suppressed"
    for path in result.receipt_directory.parent.rglob("*"):
        if path.is_file():
            assert prefix not in path.read_bytes()


async def test_captures_once_with_private_provenance_and_bounded_shared_context(tmp_path: Path) -> None:
    calls: list[httpx.Request] = []
    markdown = "é" * 8_005

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        assert request.headers["authorization"] == "Bearer test-secret-token"
        if request.url.path == "/health":
            return httpx.Response(
                200,
                json={"status": "ok", "runtime": {"revision": "c" * 40, "model": "free"}},
            )
        assert request.method == "POST"
        assert json.loads(request.content)["url"] == "https://docs.example/a"
        return httpx.Response(200, content=_response(markdown))

    sources = [
        _source("task-a", "card-a", 1, "https://docs.example/a"),
        _source("task-b", "card-a", 1, "https://docs.example/a"),
        _source("task-c", "card-file", 1, "file:///etc/passwd"),
    ]
    lease = _Lease()
    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=sources,
        lease=lease,
    )

    assert result.status == "complete"
    assert result.source_count == 3
    assert result.captured_count == 1
    assert result.failed_count == 0
    assert result.owned_http_calls == 2
    assert lease.calls == 1
    assert [request.url.path for request in calls] == ["/health", "/v2/scrape"]
    rows = result.private_inventory
    assert rows[0]["status"] == "captured"
    assert rows[0]["task_id"] == "task-a" and rows[0]["source_id"] == "card-a"
    assert rows[0]["context_characters"] == 8_000
    assert rows[0]["context_truncated"] is True
    assert rows[1]["status"] == "duplicate_url_not_attempted"
    assert rows[1]["source_id"] == rows[0]["source_id"]
    assert rows[2]["status"] == "unsupported_url_not_attempted"

    context_path = result.receipt_directory / str(rows[0]["context_artifact"])
    context = json.loads(context_path.read_text())
    assert context["context"] == "é" * 8_000
    assert context["context_sha256"] == _sha(("é" * 8_000).encode())
    assert context["task_id"] == "task-a" and context["source_id"] == "card-a"
    assert context_path.stat().st_mode & 0o777 == 0o600
    all_private = b"".join(path.read_bytes() for path in result.receipt_directory.rglob("*") if path.is_file())
    assert b"test-secret-token" not in all_private
    assert b"capture.example" not in all_private
    assert result.internal_scraper_fanout.startswith("unknown")
    assert result.quality_credit is False

    response_body = _response(markdown)
    body_sha = _sha(_canonical({"url": "https://docs.example/a", "formats": ["markdown"], "onlyMainContent": True}))
    binding = {
        "stage_uuid": result.stage_uuid,
        "operation_id": "source-0001",
        "request_body_sha256": body_sha,
        "source_revision": "8f3577d022e2d98fcd405d915b5c3b9b16e899bf",
    }
    receipt_path = result.receipt_directory / "source-0001" / "receipt.json"
    archived, raw = intent_ranking_receipts.replay_response(
        _root_from_existing(result.receipt_directory.parent),
        expected_bindings=binding,
        expected_receipt_sha256=_sha(receipt_path.read_bytes()),
    )
    assert archived["response_body_sha256"] == _sha(response_body)
    assert raw == response_body


def _root_from_existing(path: Path) -> Path:
    assert stat.S_IMODE(path.stat().st_mode) == 0o700
    return path


async def test_transport_failure_is_retained_and_later_sources_are_not_invoked(tmp_path: Path) -> None:
    seen: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        if request.url.path == "/health":
            return _healthy_response()
        if request.url.path == "/v2/scrape":
            raise httpx.ConnectError("contains-private-host-and-secret", request=request)
        raise AssertionError("unexpected request")

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[
            _source("task-a", "card-a", 1, "https://docs.example/a"),
            _source("task-a", "card-b", 2, "https://docs.example/b"),
        ],
    )
    assert seen == ["/health", "/v2/scrape"]
    assert result.status == "terminal-incomplete"
    assert result.private_inventory[0]["status"] == "terminal_capture_failure"
    assert result.private_inventory[0]["failure_code"] == "ConnectError"
    assert result.private_inventory[1]["status"] == "not_attempted_terminal_failure"
    archive = result.receipt_directory / "source-0001" / "receipt.json"
    stored = archive.read_text()
    assert "contains-private" not in stored
    assert json.loads(stored)["complete"] is False


@pytest.mark.asyncio
async def test_real_transport_and_nip_dns_alias_are_rejected_before_lease_or_request(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class CountingVerifier(_Verifier):
        calls = 0

        def verify(self, *args):
            self.calls += 1
            return super().verify(*args)

    lease = _Lease()
    verifier = CountingVerifier()
    actual_requests = []

    async def count_without_network(self, request):
        actual_requests.append(request)
        raise AssertionError("real transport must never be dispatched by this test")

    monkeypatch.setattr(httpx.AsyncHTTPTransport, "handle_async_request", count_without_network)
    transport = httpx.AsyncHTTPTransport(retries=0, trust_env=False)
    try:
        with pytest.raises(SourceCaptureError, match="live-fetch-boundary-not-qualified"):
            await _capture(
                tmp_path=tmp_path,
                transport=transport,
                sources=[
                    {
                        "task_id": "R01",
                        "source_id": "c1",
                        "result_index": 1,
                        "url": "https://127.0.0.1.nip.io/private",
                        "title": "synthetic DNS alias to a private address",
                        "engine": "test",
                    }
                ],
                verifier=verifier,
                lease=lease,
            )
    finally:
        await transport.aclose()

    assert verifier.calls == 0
    assert lease.calls == 0
    assert actual_requests == []
    assert list((tmp_path / "private-receipts").iterdir()) == []


async def test_response_over_cap_is_archived_partial_and_stops_remaining_slots(tmp_path: Path) -> None:
    seen: list[str] = []
    large_body = b"x" * (MAX_RESPONSE_BYTES + 1)

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        if request.url.path == "/health":
            return _healthy_response()
        return httpx.Response(200, content=large_body)

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[
            _source("task-a", "card-a", 1, "https://docs.example/a"),
            _source("task-a", "card-b", 2, "https://docs.example/b"),
        ],
    )
    assert seen == ["/health", "/v2/scrape"]
    assert result.status == "terminal-incomplete"
    archive = result.receipt_directory / "source-0001"
    receipt = json.loads((archive / "receipt.json").read_text())
    body = (archive / "response.bin").read_bytes()
    assert receipt["complete"] is False
    assert receipt["status"] == "response-byte-cap"
    assert len(body) == MAX_RESPONSE_BYTES
    assert result.private_inventory[1]["status"] == "not_attempted_terminal_failure"


async def test_health_failure_stops_capture_and_retains_every_uninvoked_source(tmp_path: Path) -> None:
    seen: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        return httpx.Response(503, content=b"health unavailable")

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
    )
    assert seen == ["/health"]
    assert result.status == "terminal-incomplete"
    assert result.private_inventory[0]["status"] == "not_attempted_terminal_failure"
    assert result.owned_http_calls == 1


@pytest.mark.parametrize(
    "payload,status",
    [
        ({"status": "degraded", "runtime": {"revision": "c" * 40, "model": "free"}}, "health_payload_failure"),
        ({"status": "ok"}, "health_runtime_mismatch"),
        ({"status": "ok", "runtime": {"revision": "d" * 40, "model": "free"}}, "health_runtime_mismatch"),
        ({"status": "ok", "runtime": {"revision": "c" * 40, "model": "other"}}, "health_runtime_mismatch"),
    ],
)
async def test_health_degraded_unknown_or_mismatched_runtime_stops_before_scrape(
    tmp_path: Path, payload: dict[str, object], status: str
) -> None:
    seen: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        return httpx.Response(200, json=payload)

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
    )
    assert seen == ["/health"]
    assert result.status == "terminal-incomplete"
    assert result.private_inventory[0]["status"] == "not_attempted_terminal_failure"
    inventory = json.loads((result.receipt_directory / "inventory.json").read_bytes())
    assert inventory["health_status"] == status


async def test_permit_failure_happens_before_lease_or_transport(tmp_path: Path) -> None:
    lease = _Lease()
    called = False

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal called
        called = True
        return httpx.Response(200)

    class WrongVerifier:
        def verify(self, manifest_bytes: bytes, receipt_bytes: bytes, expected_receipt_sha256: str):
            return VerifiedSourceCapturePermit(
                status="verified-admitted",
                scope="other-scope",
                stage_uuid="f611a79a-9eef-46f4-b211-3d4d394e9e21",
                manifest_sha256="0" * 64,
                protocol_sha256="0" * 64,
                source_revision="8f3577d022e2d98fcd405d915b5c3b9b16e899bf",
                cohorts_sha256="0" * 64,
                candidate_identity_sha256="0" * 64,
                candidate_endpoint_sha256="0" * 64,
                source_count=1,
                max_owned_calls=641,
                max_health_calls=1,
                max_scrape_calls=640,
                timeout_seconds=30.0,
                response_bytes=2_000_000,
                source_context_characters=8_000,
                receipt_sha256="0" * 64,
            )

    with pytest.raises(SourceCaptureError, match="permit-binding-mismatch"):
        await _capture(
            tmp_path=tmp_path,
            transport=httpx.MockTransport(handler),
            sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
            verifier=WrongVerifier(),
            lease=lease,
        )
    assert lease.calls == 0
    assert called is False


async def test_endpoint_binding_mismatch_happens_before_permit_or_dispatch(tmp_path: Path) -> None:
    lease = _Lease()
    called = False

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal called
        called = True
        return httpx.Response(200)

    with pytest.raises(SourceCaptureError, match="candidate-endpoint-binding-mismatch"):
        await _capture(
            tmp_path=tmp_path,
            transport=httpx.MockTransport(handler),
            sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
            candidate_base_url="https://other.example",
            manifest_endpoint_sha256="f" * 64,
            lease=lease,
        )
    assert lease.calls == 0
    assert called is False


async def test_protocol_cohort_binding_mismatch_precedes_lease_and_dispatch(tmp_path: Path) -> None:
    lease = _Lease()
    called = False

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal called
        called = True
        return httpx.Response(200)

    with pytest.raises(SourceCaptureError, match="capture-cohorts-binding-mismatch"):
        await _capture(
            tmp_path=tmp_path,
            transport=httpx.MockTransport(handler),
            sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
            manifest_cohorts_sha256="f" * 64,
            lease=lease,
        )
    assert lease.calls == 0
    assert called is False


async def test_candidate_identity_digest_mismatch_precedes_lease_and_dispatch(tmp_path: Path) -> None:
    lease = _Lease()
    called = False

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal called
        called = True
        return httpx.Response(200)

    with pytest.raises(SourceCaptureError, match="candidate-identity-binding-mismatch"):
        await _capture(
            tmp_path=tmp_path,
            transport=httpx.MockTransport(handler),
            sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
            candidate_identity_bytes=_canonical({"runtime": {"revision": "d" * 40, "model": "free"}}),
            manifest_candidate_identity_sha256="a" * 64,
            lease=lease,
        )
    assert lease.calls == 0
    assert called is False


async def test_source_id_cannot_bind_different_public_urls_before_lease_or_dispatch(tmp_path: Path) -> None:
    lease = _Lease()
    called = False

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal called
        called = True
        return httpx.Response(200)

    with pytest.raises(SourceCaptureError, match="capture-source-id-url-conflict"):
        await _capture(
            tmp_path=tmp_path,
            transport=httpx.MockTransport(handler),
            sources=[
                _source("task-a", "shared-source", 1, "https://docs.example/a"),
                _source("task-b", "shared-source", 1, "https://docs.example/b"),
            ],
            lease=lease,
        )
    assert lease.calls == 0
    assert called is False


async def test_stage_deadline_retains_uninvoked_sources(tmp_path: Path) -> None:
    seen: list[str] = []
    clock_values = iter([100.0, 100.0, 200.0])

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        return _healthy_response()

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[_source("task-a", "card-a", 1, "https://docs.example/a")],
        clock=lambda: next(clock_values),
    )
    assert seen == ["/health"]
    assert result.status == "terminal-incomplete"
    assert result.private_inventory[0]["status"] == "not_attempted_stage_deadline"


async def test_safe_public_http_source_is_eligible_without_rewriting_url(tmp_path: Path) -> None:
    seen: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.url.path)
        if request.url.path == "/health":
            return _healthy_response()
        return httpx.Response(200, content=_response("public source"))

    result = await _capture(
        tmp_path=tmp_path,
        transport=httpx.MockTransport(handler),
        sources=[_source("task-a", "card-a", 1, "http://docs.example/a")],
    )
    assert seen == ["/health", "/v2/scrape"]
    assert result.private_inventory[0]["status"] == "captured"
    assert result.private_inventory[0]["url"] == "http://docs.example/a"
