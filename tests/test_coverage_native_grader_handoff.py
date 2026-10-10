import asyncio
import base64
import hashlib
import json
import os
import stat
import time
import uuid
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import coverage_native_grader_handoff as handoff


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def packet(packet_id, task_id="task-1"):
    payload = {
        "schema": "coverage-assessment-model-packet/1",
        "packet_id": packet_id,
        "task_id": task_id,
        "assessor_id": "R1",
        "role": "source",
        "model_input": {"model_call_required": True},
    }
    raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return {
        "packet_id": packet_id,
        "task_id": task_id,
        "assessor_id": "R1",
        "role": "source",
        "bytes": raw,
        "sha256": sha(raw),
        "byte_count": len(raw),
    }


def inputs(*packets):
    return SimpleNamespace(preassessment_packets=tuple(packets))


async def wait_for_requests(scope, count):
    end = time.monotonic() + 2
    while time.monotonic() < end:
        paths = list((scope / "requests").glob("*.request.json")) if (scope / "requests").exists() else []
        if len(paths) == count:
            return paths
        await asyncio.sleep(0.01)
    raise AssertionError("handoff request files were not created")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def _write_private(path: Path, raw: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.parent.chmod(0o700)
    path.write_bytes(raw)
    path.chmod(0o600)


def publish_synthetic_host_evidence(scope: Path, stage_uuid: str, request_path: Path, *, tamper=False) -> None:
    from scripts import coverage_native_host_dispatch as host

    request_raw = request_path.read_bytes()
    request = json.loads(request_raw)
    packet_raw = base64.b64decode(request["packet_bytes_base64"])
    packet_id = request["packet_id"]
    phase = request["phase"]
    thread_id = str(uuid.uuid4())
    turn_id = str(uuid.uuid4())
    answer = b'{"synthetic":"grade"}'
    catalog = {"id": 2, "result": {"data": [], "nextCursor": None}}
    catalog_raw = canonical(catalog)
    text = packet_raw.decode()
    events = [
        {
            "direction": "client",
            "message": {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "clientInfo": {"name": "coverage-native-grader", "version": "1"},
                    "capabilities": {"experimentalApi": True},
                },
            },
        },
        {"direction": "server", "message": {"id": 1, "result": {}}},
        {"direction": "client", "message": {"jsonrpc": "2.0", "method": "initialized", "params": {}}},
        {
            "direction": "client",
            "message": {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "thread/start",
                "params": {
                    "model": "gpt-6.1-sol",
                    "allowProviderModelFallback": False,
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "ephemeral": True,
                    "environments": [],
                    "dynamicTools": [],
                    "cwd": "/synthetic",
                },
            },
        },
        {
            "direction": "server",
            "message": {
                "id": 2,
                "result": {
                    "model": "gpt-6.1-sol",
                    "modelProvider": "synthetic-host",
                    "thread": {"id": thread_id},
                },
            },
        },
        {
            "direction": "client",
            "message": {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "turn/start",
                "params": {"threadId": thread_id, "model": "gpt-6.1-sol", "input": [{"type": "text", "text": text}]},
            },
        },
        {"direction": "server", "message": {"id": 3, "result": {"turn": {"id": turn_id}}}},
        {
            "direction": "server",
            "message": {"method": "turn/started", "params": {"threadId": thread_id, "turn": {"id": turn_id}}},
        },
        {
            "direction": "server",
            "message": {
                "method": "turn/completed",
                "params": {
                    "threadId": thread_id,
                    "turn": {
                        "id": turn_id,
                        "status": "completed",
                        "items": [
                            {"type": "userMessage", "content": [{"type": "text", "text": text}]},
                            {
                                "type": "agentMessage",
                                "text": (b'{"synthetic":"forged"}' if tamper else answer).decode(),
                            },
                        ],
                    },
                },
            },
        },
    ]
    transcript = {
        "schema": host.TRANSCRIPT_SCHEMA,
        "stage_uuid": stage_uuid,
        "phase": phase,
        "packet_id": packet_id,
        "packet_sha256": request["packet_sha256"],
        "packet_byte_count": len(packet_raw),
        "model": "gpt-6.1-sol",
        "model_catalog_row_sha256": None,
        "model_catalog_response_sha256": hashlib.sha256(catalog_raw).hexdigest(),
        "cli_executable_sha256": "d" * 64,
        "cli_version": "codex-cli synthetic",
        "events": events,
    }
    audit = scope / "native-host"
    for directory in (audit, audit / "claims", audit / "transcripts", audit / "failures"):
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        directory.chmod(0o700)
    _write_private(audit / "model-list.response.json", catalog_raw)
    _write_private(
        audit / "claims" / f"{packet_id}.claim.json",
        canonical(
            {
                "schema": "coverage-native-host-claim/1",
                "stage_uuid": stage_uuid,
                "phase": phase,
                "packet_id": packet_id,
                "packet_sha256": request["packet_sha256"],
                "request_sha256": hashlib.sha256(request_raw).hexdigest(),
                "cli_executable_sha256": "d" * 64,
                "model": "gpt-6.1-sol",
                "reserved_before_dispatch": True,
            }
        ),
    )
    _write_private(audit / "transcripts" / f"{packet_id}.transcript.json", canonical(transcript))
    _write_private(
        audit / "dispatch-summary.json",
        canonical(
            {
                "schema": "coverage-native-host-dispatch-summary/1",
                "stage_uuid": stage_uuid,
                "phase": phase,
                "model": "gpt-6.1-sol",
                "model_list_response_sha256": hashlib.sha256(catalog_raw).hexdigest(),
                "model_catalog_advertised_exact_model": False,
                "cli_executable_sha256": "d" * 64,
                "cli_version": "codex-cli synthetic",
                "maximum_concurrent_calls": 1,
                "packet_count": 1,
                "claimed_packet_ids": [packet_id],
                "completed_packet_ids": [packet_id],
                "unstarted_packet_ids": [],
                "failure_reasons": {},
                "status": "complete",
            }
        ),
    )
    handoff.publish_native_tool_result(
        scope=scope,
        packet_id=packet_id,
        tool_call_id=turn_id,
        model="gpt-6.1-sol",
        response_bytes=answer,
    )


def test_native_tool_results_round_trip_exact_packet_and_call_bindings(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000321"
    packets = (packet("a" * 64), packet("b" * 64, "task-2"))
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=False)
    scope = root / stage_uuid / "references"

    async def run():
        collecting = asyncio.create_task(
            queue.collect(
                stage_uuid=stage_uuid,
                phase="references",
                prepared_packets=inputs(*packets),
                deadline_monotonic=time.monotonic() + 3,
            )
        )
        requests = await wait_for_requests(scope, 2)
        expected_packets = {row["packet_id"]: row for row in packets}
        # Directory enumeration is unordered; return the second packet first
        # and prove collection still binds and orders results by packet identity.
        requests.sort(key=lambda path: json.loads(path.read_bytes())["packet_id"], reverse=True)
        for index, request_path in enumerate(requests):
            request = json.loads(request_path.read_bytes())
            assert base64.b64decode(request["packet_bytes_base64"]) == expected_packets[request["packet_id"]]["bytes"]
            handoff.publish_native_tool_result(
                scope=scope,
                packet_id=request["packet_id"],
                tool_call_id=f"native-call-{index}",
                model="gpt-6.1-sol",
                response_bytes=b'{"synthetic":"raw tool result"}',
            )
        return await collecting

    submissions = asyncio.run(run())
    assert [row.packet_id for row in submissions] == ["a" * 64, "b" * 64]
    assert all(row.input_sha256 == packet_row["sha256"] for row, packet_row in zip(submissions, packets, strict=True))
    assert all(row.response_bytes == b'{"synthetic":"raw tool result"}' for row in submissions)
    assert (scope / "results-closed.json").exists()
    assert os.stat(root).st_mode & 0o077 == 0
    assert os.stat(scope / "handoff-manifest.json").st_mode & 0o077 == 0


def test_malformed_or_tampered_tool_result_is_rejected_without_partial_return(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000322"
    rows = (packet("c" * 64), packet("d" * 64, "task-2"))
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=False)
    scope = root / stage_uuid / "references"

    async def run():
        collecting = asyncio.create_task(
            queue.collect(
                stage_uuid=stage_uuid,
                phase="references",
                prepared_packets=inputs(*rows),
                deadline_monotonic=time.monotonic() + 3,
            )
        )
        paths = await wait_for_requests(scope, 2)
        for index, path in enumerate(paths):
            request = json.loads(path.read_bytes())
            handoff.publish_native_tool_result(
                scope=scope,
                packet_id=request["packet_id"],
                tool_call_id=f"native-call-{index}",
                model="gpt-6.1-sol",
                response_bytes=b'{"synthetic":true}',
            )
        result = scope / "results" / f"{'c' * 64}.result.json"
        result.write_bytes(result.read_bytes() + b" ")
        return await collecting

    with pytest.raises(handoff.NativeHandoffError, match="grader-native-result-binding-invalid"):
        asyncio.run(run())
    assert not (scope / "results-closed.json").exists()


def test_missing_tool_result_times_out_and_handoff_cannot_be_retried(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000323"
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=False)
    prepared = inputs(packet("e" * 64))

    async def run():
        return await queue.collect(
            stage_uuid=stage_uuid,
            phase="references",
            prepared_packets=prepared,
            deadline_monotonic=time.monotonic() + 0.05,
            poll_interval=0.01,
        )

    with pytest.raises(handoff.NativeHandoffError, match="grader-native-handoff-deadline-expired"):
        asyncio.run(run())
    with pytest.raises(handoff.NativeHandoffError, match="grader-handoff-already-created-no-resume"):
        asyncio.run(run())


def test_invalid_model_stage_or_packet_prevents_handoff_creation(tmp_path):
    root = tmp_path / "private"
    with pytest.raises(handoff.NativeHandoffError, match="native-grader-model-mismatch"):
        handoff.NativeGraderHandoff(root, model="unapproved-alias")
    queue = handoff.NativeGraderHandoff(root)
    bad = packet("f" * 64)
    bad["sha256"] = "0" * 64

    async def run():
        await queue.collect(
            stage_uuid="../escape",
            phase="references",
            prepared_packets=inputs(bad),
            deadline_monotonic=time.monotonic() + 1,
        )

    with pytest.raises(handoff.NativeHandoffError, match="prepared-packet-binding-invalid"):
        asyncio.run(run())
    assert not (root / ".." / "escape").exists()


@pytest.mark.parametrize("tamper", [False, True])
def test_required_host_transcripts_bind_complete_fake_protocol_before_submission(tmp_path, tamper):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000324"
    row = packet("1" * 64)
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=True)
    scope = root / stage_uuid / "references"

    async def run():
        collecting = asyncio.create_task(
            queue.collect(
                stage_uuid=stage_uuid,
                phase="references",
                prepared_packets=inputs(row),
                deadline_monotonic=time.monotonic() + 3,
            )
        )
        requests = await wait_for_requests(scope, 1)
        publish_synthetic_host_evidence(scope, stage_uuid, requests[0], tamper=tamper)
        return await collecting

    if tamper:
        with pytest.raises(handoff.NativeHandoffError, match="transcript-result-mismatch"):
            asyncio.run(run())
        assert not (scope / "results-closed.json").exists()
    else:
        submissions = asyncio.run(run())
        assert len(submissions) == 1
        assert submissions[0].response_bytes == b'{"synthetic":"grade"}'
        closed = json.loads((scope / "results-closed.json").read_bytes())
        assert closed["native_host_transcript_schema"] == "coverage-native-host-transcript/1"
        assert set(closed["native_host_transcript_sha256"]) == {row["packet_id"]}
        assert (
            stat.S_IMODE((scope / "native-host" / "transcripts" / f"{row['packet_id']}.transcript.json").stat().st_mode)
            == 0o600
        )


def test_required_host_transcript_missing_is_terminal_without_submission(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000325"
    row = packet("2" * 64)
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=True)
    scope = root / stage_uuid / "references"

    async def run():
        collecting = asyncio.create_task(
            queue.collect(
                stage_uuid=stage_uuid,
                phase="references",
                prepared_packets=inputs(row),
                deadline_monotonic=time.monotonic() + 0.08,
                poll_interval=0.01,
            )
        )
        requests = await wait_for_requests(scope, 1)
        request = json.loads(requests[0].read_bytes())
        handoff.publish_native_tool_result(
            scope=scope,
            packet_id=request["packet_id"],
            tool_call_id="metadata-only-call-id",
            model="gpt-6.1-sol",
            response_bytes=b'{"synthetic":true}',
        )
        return await collecting

    with pytest.raises(handoff.NativeHandoffError, match="host-inventory-incomplete-terminal"):
        asyncio.run(run())
    assert not (scope / "results-closed.json").exists()


def test_failed_terminal_dispatch_summary_stops_collection_promptly_and_retains_artifacts(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000326"
    packets = (packet("3" * 64, "task-1"), packet("4" * 64, "task-2"))
    queue = handoff.NativeGraderHandoff(root, require_native_host_transcripts=True)
    scope = root / stage_uuid / "references"

    async def run():
        collecting = asyncio.create_task(
            queue.collect(
                stage_uuid=stage_uuid,
                phase="references",
                prepared_packets=inputs(*packets),
                deadline_monotonic=time.monotonic() + 30,
                poll_interval=0.01,
            )
        )
        await wait_for_requests(scope, 2)
        manifest = json.loads((scope / "handoff-manifest.json").read_bytes())
        rows = manifest["packets"]
        failed_id = rows[0]["packet_id"]
        unstarted_ids = sorted({row["packet_id"] for row in rows} - {failed_id})
        request_path = scope / "requests" / f"{failed_id}.request.json"
        request_bytes = request_path.read_bytes()
        request = json.loads(request_bytes)
        audit_root = scope / "native-host"
        claim_root = audit_root / "claims"
        failure_root = audit_root / "failures"
        transcript_root = audit_root / "transcripts"
        for directory in (audit_root, claim_root, failure_root, transcript_root):
            directory.mkdir(mode=0o700, parents=True, exist_ok=True)
            directory.chmod(0o700)
        cli_sha = "e" * 64
        reason = "native-turn-start-rejected"
        model_list = {"id": 2, "result": {"data": [], "nextCursor": None}}
        model_list_bytes = canonical(model_list)
        _write_private(audit_root / "model-list.response.json", model_list_bytes)
        _write_private(
            claim_root / f"{failed_id}.claim.json",
            canonical(
                {
                    "schema": "coverage-native-host-claim/1",
                    "stage_uuid": stage_uuid,
                    "phase": "references",
                    "packet_id": failed_id,
                    "packet_sha256": request["packet_sha256"],
                    "request_sha256": sha(request_bytes),
                    "cli_executable_sha256": cli_sha,
                    "model": "gpt-6.1-sol",
                    "reserved_before_dispatch": True,
                }
            ),
        )
        _write_private(
            failure_root / f"{failed_id}.failure.json",
            canonical({"schema": "coverage-native-host-failure/1", "packet_id": failed_id, "reason": reason}),
        )
        summary = {
            "schema": "coverage-native-host-dispatch-summary/1",
            "stage_uuid": stage_uuid,
            "phase": "references",
            "model": "gpt-6.1-sol",
            "model_list_response_sha256": sha(model_list_bytes),
            "model_catalog_advertised_exact_model": False,
            "cli_executable_sha256": cli_sha,
            "cli_version": "codex-cli synthetic",
            "maximum_concurrent_calls": 1,
            "packet_count": 2,
            "claimed_packet_ids": [failed_id],
            "completed_packet_ids": [],
            "unstarted_packet_ids": unstarted_ids,
            "failure_reasons": {failed_id: reason},
            "status": "failed-terminal",
        }
        summary_path = audit_root / "dispatch-summary.json"
        _write_private(summary_path, canonical(summary))
        started = time.monotonic()
        with pytest.raises(handoff.NativeHandoffError, match="dispatch-failed-terminal"):
            await asyncio.wait_for(collecting, timeout=1)
        assert time.monotonic() - started < 1
        assert summary_path.exists()
        assert (claim_root / f"{failed_id}.claim.json").exists()
        assert (failure_root / f"{failed_id}.failure.json").exists()
        assert not (scope / "results-closed.json").exists()

    asyncio.run(run())
