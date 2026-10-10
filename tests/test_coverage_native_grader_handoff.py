import asyncio
import base64
import hashlib
import json
import os
import time
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


def test_native_tool_results_round_trip_exact_packet_and_call_bindings(tmp_path):
    root = tmp_path / "private"
    stage_uuid = "00000000-0000-0000-0000-000000000321"
    packets = (packet("a" * 64), packet("b" * 64, "task-2"))
    queue = handoff.NativeGraderHandoff(root)
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
    queue = handoff.NativeGraderHandoff(root)
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
    queue = handoff.NativeGraderHandoff(root)
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
