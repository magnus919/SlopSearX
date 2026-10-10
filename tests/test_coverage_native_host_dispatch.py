from __future__ import annotations

import asyncio
import hashlib
import json
import stat
import textwrap
import time
import uuid
from pathlib import Path

import pytest

from scripts import coverage_assessment_packets
from scripts import coverage_native_host_dispatch as native
from tests.test_coverage_assessment_packets import _answer_stage
from tests.test_coverage_assessment_packets import _fixture as answer_packet_fixture


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def packet(packet_id: str, *, task_id: str = "D1E0001") -> bytes:
    return canonical(
        {
            "packet_id": packet_id,
            "task_id": task_id,
            "assessor_id": "A",
            "role": "card",
            "model_input": {"rubric": "synthetic fixture"},
        }
    )


def write_phase_deadline(scope: Path, stage_uuid: str, phase: str, deadline: float) -> None:
    path = scope / "phase-deadline.json"
    path.write_bytes(
        canonical(
            {
                "schema": "coverage-native-phase-deadline/1",
                "stage_uuid": stage_uuid,
                "phase": phase,
                "deadline_monotonic": deadline,
            }
        )
    )
    path.chmod(0o600)


def write_scope(root: Path, count: int = 1) -> tuple[Path, str, list[str]]:
    stage_uuid = str(uuid.uuid4())
    scope = root / stage_uuid / "references"
    requests_dir = scope / "requests"
    results_dir = scope / "results"
    requests_dir.mkdir(mode=0o700, parents=True)
    (root / stage_uuid).chmod(0o700)
    scope.chmod(0o700)
    requests_dir.chmod(0o700)
    results_dir.mkdir(mode=0o700)
    rows = []
    packet_ids = []
    for index in range(count):
        packet_id = hashlib.sha256(f"packet-{index}".encode()).hexdigest()
        packet_ids.append(packet_id)
        raw = packet(packet_id, task_id=f"D{index + 1}E0001")
        row = {
            "packet_id": packet_id,
            "task_id": f"D{index + 1}E0001",
            "assessor_id": "A",
            "role": "card",
            "packet_sha256": hashlib.sha256(raw).hexdigest(),
            "byte_count": len(raw),
        }
        rows.append(row)
        envelope = {
            "schema": "coverage-native-grader-handoff/1",
            "stage_uuid": stage_uuid,
            "phase": "references",
            "model": native.MODEL,
            **row,
            "packet_bytes_base64": __import__("base64").b64encode(raw).decode(),
        }
        path = requests_dir / f"{packet_id}.request.json"
        path.write_bytes(canonical(envelope))
        path.chmod(0o600)
    manifest = {
        "schema": "coverage-native-grader-handoff/1",
        "stage_uuid": stage_uuid,
        "phase": "references",
        "model": native.MODEL,
        "status": "awaiting-native-tool-results",
        "packet_count": count,
        "packets": rows,
    }
    (scope / "handoff-manifest.json").write_bytes(canonical(manifest))
    (scope / "handoff-manifest.json").chmod(0o600)
    write_phase_deadline(scope, stage_uuid, "references", time.monotonic() + 3600)
    return scope, stage_uuid, packet_ids


def write_prepared_answer_scope(root: Path) -> tuple[Path, str, list[str]]:
    prepared, captures = answer_packet_fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    prepared_answers = coverage_assessment_packets.prepare_answer_assessment_packets(
        prepared=prepared,
        answer_inputs=answer_inputs,
        answer_outputs=answer_outputs,
        source_outputs=source_outputs,
    )
    stage_uuid = prepared.packet_stage_uuid
    scope = root / stage_uuid / "answers"
    requests_dir = scope / "requests"
    results_dir = scope / "results"
    requests_dir.mkdir(mode=0o700, parents=True)
    (root / stage_uuid).chmod(0o700)
    scope.chmod(0o700)
    requests_dir.chmod(0o700)
    results_dir.mkdir(mode=0o700)
    rows = []
    packet_ids = []
    for packet_row in prepared_answers.packets:
        packet_id = packet_row["packet_id"]
        packet_ids.append(packet_id)
        row = {
            "packet_id": packet_id,
            "task_id": packet_row["task_id"],
            "assessor_id": packet_row["assessor_id"],
            "role": packet_row["role"],
            "packet_sha256": packet_row["sha256"],
            "byte_count": packet_row["byte_count"],
        }
        rows.append(row)
        envelope = {
            "schema": "coverage-native-grader-handoff/1",
            "stage_uuid": stage_uuid,
            "phase": "answers",
            "model": native.MODEL,
            **row,
            "packet_bytes_base64": __import__("base64").b64encode(packet_row["bytes"]).decode(),
        }
        path = requests_dir / f"{packet_id}.request.json"
        path.write_bytes(canonical(envelope))
        path.chmod(0o600)
    manifest = {
        "schema": "coverage-native-grader-handoff/1",
        "stage_uuid": stage_uuid,
        "phase": "answers",
        "model": native.MODEL,
        "status": "awaiting-native-tool-results",
        "packet_count": len(rows),
        "packets": rows,
    }
    manifest_path = scope / "handoff-manifest.json"
    manifest_path.write_bytes(canonical(manifest))
    manifest_path.chmod(0o600)
    write_phase_deadline(scope, stage_uuid, "answers", time.monotonic() + 3600)
    return scope, stage_uuid, packet_ids


def fake_codex(tmp_path: Path) -> tuple[Path, str]:
    script = tmp_path / "fake-codex"
    script.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json, os, sys, time, uuid

            def emit(value):
                print(json.dumps(value, separators=(",", ":")), flush=True)

            if sys.argv[1:] == ["--version"]:
                print("codex-cli synthetic-test")
                raise SystemExit(0)
            if sys.argv[1:3] != ["app-server", "--listen"]:
                raise SystemExit(2)
            for line in sys.stdin:
                request = json.loads(line)
                method = request.get("method")
                if method == "initialize":
                    emit({"id": request["id"], "result": {"codexHome": "/synthetic"}})
                elif method == "initialized":
                    pass
                elif method == "model/list":
                    emit({
                        "id": request["id"],
                        "result": {
                            "data": [{"id": "gpt-5.6-sol", "model": "gpt-5.6-sol", "supportedReasoningEfforts": []}],
                            "nextCursor": None,
                        },
                    })
                elif method == "thread/start":
                    emit({
                        "id": request["id"],
                        "result": {
                            "model": "gpt-6.1-sol",
                            "modelProvider": "synthetic-host",
                            "thread": {"id": str(uuid.uuid4())},
                        },
                    })
                elif method == "turn/start":
                    turn_id = str(uuid.uuid4())
                    thread_id = request["params"]["threadId"]
                    emit({
                        "id": request["id"],
                        "result": {"turn": {"id": turn_id, "status": "inProgress", "items": []}},
                    })
                    emit({
                        "method": "turn/started",
                        "params": {"threadId": thread_id, "turn": {"id": turn_id, "status": "inProgress", "items": []}},
                    })
                    time.sleep(0.04)
                    text = request["params"]["input"][0]["text"]
                    items = [
                        {"type": "userMessage", "id": "user-1", "content": [{"type": "text", "text": text}]},
                        {"type": "agentMessage", "id": "assistant-1", "text": json.dumps({"synthetic": True})},
                    ]
                    completion_status = "failed" if os.environ.get("FAKE_CODEX_FAIL_TURN") else "completed"
                    emit({
                        "method": "turn/completed",
                        "params": {
                            "threadId": thread_id,
                            "turn": {
                                "id": turn_id,
                                "status": completion_status,
                                "items": items,
                                "itemsView": "full",
                            },
                        },
                    })
                else:
                    raise SystemExit("unexpected RPC")
            """
        ).lstrip()
    )
    script.chmod(0o700)
    return script, hashlib.sha256(script.read_bytes()).hexdigest()


def make_transcript(packet_id: str, packet_bytes: bytes, *, tamper: str | None = None) -> bytes:
    stage_uuid = str(uuid.uuid4())
    thread_id = str(uuid.uuid4())
    turn_id = str(uuid.uuid4())
    text = packet_bytes.decode()
    requests = [
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {"name": "coverage-native-grader", "version": "1"},
                "capabilities": {"experimentalApi": True},
            },
        },
        {"jsonrpc": "2.0", "method": "initialized", "params": {}},
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "thread/start",
            "params": {
                "model": native.MODEL,
                "allowProviderModelFallback": False,
                "approvalPolicy": "never",
                "sandbox": "read-only",
                "ephemeral": True,
                "environments": [],
                "dynamicTools": [],
                "cwd": "/tmp",
            },
        },
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "turn/start",
            "params": {"threadId": thread_id, "model": native.MODEL, "input": [{"type": "text", "text": text}]},
        },
    ]
    if tamper == "input":
        requests[-1]["params"]["input"][0]["text"] = "changed"
    if tamper == "experimental-api-optin":
        requests[0]["params"]["capabilities"]["experimentalApi"] = False
    if tamper == "extra-tool-policy":
        requests[2]["params"]["tools"] = ["shell"]
    events = [
        {"direction": "client", "message": requests[0]},
        {"direction": "server", "message": {"id": 1, "result": {}}},
        {"direction": "client", "message": requests[1]},
        {"direction": "client", "message": requests[2]},
        {
            "direction": "server",
            "message": {
                "id": 2,
                "result": {
                    "model": native.MODEL if tamper != "wrong-model" else "gpt-5.6-sol",
                    "modelProvider": "synthetic-host",
                    "thread": {"id": thread_id},
                },
            },
        },
        {"direction": "client", "message": requests[3]},
        {"direction": "server", "message": {"id": 3, "result": {"turn": {"id": turn_id}}}},
        {
            "direction": "server",
            "message": {
                "method": "turn/started",
                "params": {"threadId": thread_id, "turn": {"id": turn_id, "status": "inProgress"}},
            },
        },
    ]
    items = [
        {
            "type": "userMessage",
            "id": "u",
            "content": [{"type": "text", "text": text if tamper != "input" else "changed"}],
        },
        {"type": "agentMessage", "id": "a", "text": '{"result":"synthetic"}'},
    ]
    if tamper == "tool":
        items.append({"type": "mcpToolCall", "id": "mcp-1"})
    if tamper == "incomplete":
        status = "failed"
    else:
        status = "completed"
    events.append(
        {
            "direction": "server",
            "message": {
                "method": "turn/completed",
                "params": {
                    "threadId": thread_id,
                    "turn": {
                        "id": turn_id,
                        "status": status,
                        "items": items,
                        "itemsView": tamper if tamper in {"summary", "notLoaded"} else "full",
                    },
                },
            },
        }
    )
    if tamper == "rerouted":
        events.insert(
            -1,
            {
                "direction": "server",
                "message": {
                    "method": "model/rerouted",
                    "params": {
                        "fromModel": native.MODEL,
                        "toModel": "gpt-5.6-sol",
                        "threadId": thread_id,
                        "turnId": turn_id,
                        "reason": "test",
                    },
                },
            },
        )
    catalog_row = {"model": "gpt-6.1-sol", "id": "gpt-6.1-sol", "supportedReasoningEfforts": []}
    value = {
        "schema": native.TRANSCRIPT_SCHEMA,
        "stage_uuid": stage_uuid,
        "phase": "references",
        "packet_id": packet_id,
        "packet_sha256": hashlib.sha256(packet_bytes).hexdigest(),
        "packet_byte_count": len(packet_bytes),
        "model": native.MODEL,
        "model_catalog_row_sha256": hashlib.sha256(canonical(catalog_row)).hexdigest(),
        "model_catalog_response_sha256": "b" * 64,
        "cli_executable_sha256": "c" * 64,
        "cli_version": "codex-cli synthetic",
        "events": events,
    }
    value["_test_identity"] = {"stage_uuid": stage_uuid, "thread_id": thread_id}
    return canonical(value)


def test_model_catalog_absence_is_metadata_not_dispatch_gate() -> None:
    response = {
        "id": 2,
        "result": {
            "data": [{"id": "gpt-5.6-sol", "model": "gpt-5.6-sol", "supportedReasoningEfforts": []}],
            "nextCursor": None,
        },
    }
    row_sha, response_sha = native.inspect_model_catalog(response)
    assert row_sha is None
    assert response_sha == hashlib.sha256(canonical(response)).hexdigest()
    versioned = {"jsonrpc": "2.0", **response}
    assert native.inspect_model_catalog(versioned) == (None, hashlib.sha256(canonical(versioned)).hexdigest())
    with pytest.raises(native.NativeHostError, match="response-binding"):
        native.inspect_model_catalog({"jsonrpc": "1.0", **response})


@pytest.mark.parametrize(
    "tamper",
    [
        "input",
        "wrong-model",
        "rerouted",
        "tool",
        "incomplete",
        "extra-tool-policy",
        "experimental-api-optin",
        "summary",
        "notLoaded",
    ],
)
def test_transcript_rejects_mismatched_or_nonterminal_invocation(tamper: str) -> None:
    packet_id = "a" * 64
    raw_packet = packet(packet_id)
    transcript = json.loads(make_transcript(packet_id, raw_packet, tamper=tamper))
    identity = transcript.pop("_test_identity")
    with pytest.raises(native.NativeHostError):
        native.verify_transcript(
            canonical(transcript),
            stage_uuid=identity["stage_uuid"],
            phase="references",
            packet_id=packet_id,
            packet_sha256=hashlib.sha256(raw_packet).hexdigest(),
            expected_catalog_row_sha256=transcript["model_catalog_row_sha256"],
            expected_catalog_response_sha256=transcript["model_catalog_response_sha256"],
        )


def test_transcript_roundtrip_binds_packet_and_returns_exact_final_text() -> None:
    packet_id = "a" * 64
    raw_packet = packet(packet_id)
    transcript = json.loads(make_transcript(packet_id, raw_packet))
    identity = transcript.pop("_test_identity")
    verified = native.verify_transcript(
        canonical(transcript),
        stage_uuid=identity["stage_uuid"],
        phase="references",
        packet_id=packet_id,
        packet_sha256=hashlib.sha256(raw_packet).hexdigest(),
        expected_catalog_row_sha256=transcript["model_catalog_row_sha256"],
        expected_catalog_response_sha256=transcript["model_catalog_response_sha256"],
    )
    assert verified.response_bytes == b'{"result":"synthetic"}'
    assert verified.thread_id == identity["thread_id"]

    # The installed generated schema defaults Turn.itemsView to "full".
    completed = next(
        event["message"]
        for event in transcript["events"]
        if event["direction"] == "server" and event["message"].get("method") == "turn/completed"
    )
    del completed["params"]["turn"]["itemsView"]
    defaulted = native.verify_transcript(
        canonical(transcript),
        stage_uuid=identity["stage_uuid"],
        phase="references",
        packet_id=packet_id,
        packet_sha256=hashlib.sha256(raw_packet).hexdigest(),
        expected_catalog_row_sha256=transcript["model_catalog_row_sha256"],
        expected_catalog_response_sha256=transcript["model_catalog_response_sha256"],
    )
    assert defaulted.response_bytes == verified.response_bytes


def test_actual_prepared_answer_packet_shape_loads_without_inner_role(tmp_path: Path) -> None:
    scope, stage_uuid, packet_ids = write_prepared_answer_scope(tmp_path)
    _, manifest, requests = native._load_scope(scope, stage_uuid=stage_uuid, phase="answers")
    assert manifest["packet_count"] == 32
    assert [request["row"]["packet_id"] for request in requests] == [row["packet_id"] for row in manifest["packets"]]
    first_payload = json.loads(requests[0]["bytes"])
    assert first_payload["packet_kind"] == "answer"
    assert "role" not in first_payload
    assert len(packet_ids) == 32


@pytest.mark.asyncio
async def test_fake_app_server_dispatch_is_two_wide_and_one_shot(
    tmp_path: Path,
) -> None:
    scope, stage_uuid, packet_ids = write_scope(tmp_path, count=5)
    codex, digest = fake_codex(tmp_path)
    summary = await native.dispatch_handoff_scope(
        scope_path=scope,
        stage_uuid=stage_uuid,
        phase="references",
        codex_executable=str(codex),
        expected_cli_sha256=digest,
        deadline_monotonic=asyncio.get_running_loop().time() + 30,
    )
    assert summary["status"] == "complete"
    assert summary["maximum_concurrent_calls"] == 2
    assert set(summary["completed_packet_ids"]) == set(packet_ids)
    assert all(
        stat.S_IMODE(path.stat().st_mode) == 0o600 for path in (scope / "native-host" / "transcripts").glob("*.json")
    )
    assert len(list((scope / "results").glob("*.result.json"))) == 5
    with pytest.raises(native.NativeHostError, match="already-consumed"):
        await native.dispatch_handoff_scope(
            scope_path=scope,
            stage_uuid=stage_uuid,
            phase="references",
            codex_executable=str(codex),
            expected_cli_sha256=digest,
            deadline_monotonic=time.monotonic() + 30,
        )


@pytest.mark.asyncio
async def test_bad_cli_pin_fails_before_native_scope_or_results(tmp_path: Path) -> None:
    scope, stage_uuid, _ = write_scope(tmp_path)
    codex, _digest = fake_codex(tmp_path)
    with pytest.raises(native.NativeHostError, match="source-pin-mismatch"):
        await native.dispatch_handoff_scope(
            scope_path=scope,
            stage_uuid=stage_uuid,
            phase="references",
            codex_executable=str(codex),
            expected_cli_sha256="0" * 64,
            deadline_monotonic=time.monotonic() + 30,
        )
    assert not (scope / "native-host").exists()
    assert not list((scope / "results").glob("*.result.json"))


@pytest.mark.asyncio
async def test_expired_registered_phase_deadline_blocks_even_with_fresh_cli_budget(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    scope, stage_uuid, _ = write_scope(tmp_path)
    write_phase_deadline(scope, stage_uuid, "references", time.monotonic() - 1)
    codex, digest = fake_codex(tmp_path)
    catalog_called = False

    async def forbidden_catalog(*_args, **_kwargs):
        nonlocal catalog_called
        catalog_called = True
        raise AssertionError("expired stage must fail before app-server access")

    monkeypatch.setattr(native, "_request_model_catalog", forbidden_catalog)
    with pytest.raises(native.NativeHostError, match="stage-deadline-expired"):
        await native.dispatch_handoff_scope(
            scope_path=scope,
            stage_uuid=stage_uuid,
            phase="references",
            codex_executable=str(codex),
            expected_cli_sha256=digest,
            deadline_monotonic=time.monotonic() + 28_800,
        )
    assert not catalog_called
    assert not (scope / "native-host").exists()
    assert not list((scope / "results").glob("*.result.json"))


@pytest.mark.asyncio
async def test_failed_native_turn_is_terminal_without_result_or_retry(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    scope, stage_uuid, packet_ids = write_scope(tmp_path)
    codex, digest = fake_codex(tmp_path)
    monkeypatch.setenv("FAKE_CODEX_FAIL_TURN", "1")
    with pytest.raises(native.NativeHostError, match="incomplete-terminal"):
        await native.dispatch_handoff_scope(
            scope_path=scope,
            stage_uuid=stage_uuid,
            phase="references",
            codex_executable=str(codex),
            expected_cli_sha256=digest,
            deadline_monotonic=time.monotonic() + 30,
        )
    audit = scope / "native-host"
    summary = json.loads((audit / "dispatch-summary.json").read_bytes())
    assert summary["status"] == "failed-terminal"
    assert summary["claimed_packet_ids"] == packet_ids
    assert summary["completed_packet_ids"] == []
    assert list((audit / "failures").glob("*.failure.json"))
    assert not list((scope / "results").glob("*.result.json"))
    with pytest.raises(native.NativeHostError, match="already-consumed"):
        await native.dispatch_handoff_scope(
            scope_path=scope,
            stage_uuid=stage_uuid,
            phase="references",
            codex_executable=str(codex),
            expected_cli_sha256=digest,
            deadline_monotonic=time.monotonic() + 30,
        )
