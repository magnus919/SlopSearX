"""Native Codex-host grader dispatcher and transcript verifier.

This is a host-side bridge, not a grading policy. It accepts only frozen
handoff request files, issues each packet once through the local Codex
app-server, archives the JSON-RPC event chain, and then submits the exact final
assistant text to the existing native-grader handoff. A failed invocation is
terminal for that packet; no retry or resume is provided.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import os
import re
import stat
import tempfile
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from scripts.coverage_native_grader_handoff import publish_native_tool_result

MODEL = "gpt-6.1-sol"
TRANSCRIPT_SCHEMA = "coverage-native-host-transcript/1"
MAX_PACKET_BYTES = 384_000
MAX_TRANSCRIPT_BYTES = 12_000_000
MAX_EVENT_LINE_BYTES = 3_000_000
MAX_IN_FLIGHT = 2
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z")


class NativeHostError(RuntimeError):
    """Native host dispatch failed; the reserved packet must not be retried."""


@dataclass(frozen=True)
class VerifiedTranscript:
    thread_id: str
    turn_id: str
    model_provider: str
    response_bytes: bytes


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise NativeHostError("native-transcript-canonical-json-invalid") from exc


def _strict(raw: bytes, *, maximum: int = MAX_TRANSCRIPT_BYTES) -> object:
    if type(raw) is not bytes or not raw or len(raw) > maximum:
        raise NativeHostError("native-transcript-bytes-invalid")

    def pairs(rows: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in rows:
            if key in result:
                raise NativeHostError("native-transcript-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise NativeHostError("native-transcript-json-invalid") from exc


def _write_exclusive(path: Path, raw: bytes) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.parent.is_symlink() or stat.S_IMODE(path.parent.stat(follow_symlinks=False).st_mode) & 0o077:
        raise NativeHostError("native-transcript-directory-not-private")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, 0o600)
    except FileExistsError as exc:
        raise NativeHostError("native-packet-already-claimed") from exc
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(descriptor)
    parent_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)


def _read_private(path: Path, *, maximum: int) -> bytes:
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    except OSError as exc:
        raise NativeHostError("native-request-unavailable") from exc
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) & 0o077 or metadata.st_size > maximum:
            raise NativeHostError("native-private-file-invalid")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            raise NativeHostError("native-private-file-over-cap")
        return raw
    finally:
        os.close(descriptor)


def inspect_model_catalog(response: object, *, expected_model: str = MODEL) -> tuple[str | None, str]:
    """Record catalog metadata without making it an execution gate."""
    if type(response) is not dict or set(response) not in ({"id", "result"}, {"jsonrpc", "id", "result"}):
        raise NativeHostError("native-model-catalog-response-invalid")
    if response.get("jsonrpc", "2.0") != "2.0" or response.get("id") != 2:
        raise NativeHostError("native-model-catalog-response-binding")
    result = response.get("result")
    if type(result) is not dict or type(result.get("data")) is not list:
        raise NativeHostError("native-model-catalog-shape")
    rows = [row for row in result["data"] if type(row) is dict and row.get("model") == expected_model]
    if len(rows) > 1:
        raise NativeHostError("native-model-catalog-duplicate")
    row_sha = None
    if rows:
        row = rows[0]
        if row.get("id") != expected_model or type(row.get("supportedReasoningEfforts")) is not list:
            raise NativeHostError("native-model-catalog-row-invalid")
        row_sha = _sha(_canonical(row))
    return row_sha, _sha(_canonical(response))


def verify_transcript(
    raw: bytes,
    *,
    stage_uuid: str,
    phase: str,
    packet_id: str,
    packet_sha256: str,
    expected_catalog_row_sha256: str | None,
    expected_catalog_response_sha256: str,
    expected_model: str = MODEL,
) -> VerifiedTranscript:
    """Verify a complete app-server event transcript against one frozen packet."""
    value = _strict(raw)
    required = {
        "schema",
        "stage_uuid",
        "phase",
        "packet_id",
        "packet_sha256",
        "packet_byte_count",
        "model",
        "model_catalog_row_sha256",
        "model_catalog_response_sha256",
        "cli_executable_sha256",
        "cli_version",
        "events",
    }
    if type(value) is not dict or set(value) != required or value.get("schema") != TRANSCRIPT_SCHEMA:
        raise NativeHostError("native-transcript-schema")
    if (
        value.get("stage_uuid") != stage_uuid
        or value.get("phase") != phase
        or value.get("packet_id") != packet_id
        or value.get("packet_sha256") != packet_sha256
        or value.get("model") != expected_model
        or value.get("model_catalog_row_sha256") != expected_catalog_row_sha256
        or type(value.get("packet_byte_count")) is not int
        or value["packet_byte_count"] <= 0
        or type(value.get("cli_version")) is not str
        or not value["cli_version"]
        or not _SHA.fullmatch(str(value.get("packet_sha256")))
        or (expected_catalog_row_sha256 is not None and not _SHA.fullmatch(expected_catalog_row_sha256))
        or value.get("model_catalog_response_sha256") != expected_catalog_response_sha256
        or not _SHA.fullmatch(str(value.get("model_catalog_response_sha256")))
        or not _SHA.fullmatch(str(value.get("cli_executable_sha256")))
        or type(value.get("events")) is not list
    ):
        raise NativeHostError("native-transcript-binding-mismatch")
    try:
        if str(uuid.UUID(stage_uuid)) != stage_uuid:
            raise ValueError
    except (TypeError, ValueError) as exc:
        raise NativeHostError("native-stage-uuid-invalid") from exc

    events = value["events"]
    client_requests = []
    client_notifications = []
    server_messages = []
    for event in events:
        if type(event) is not dict or set(event) != {"direction", "message"}:
            raise NativeHostError("native-event-row-invalid")
        message = event.get("message")
        if type(message) is not dict or message.get("jsonrpc", "2.0") != "2.0":
            raise NativeHostError("native-event-message-invalid")
        if event["direction"] == "client":
            if "method" not in message:
                raise NativeHostError("native-client-request-invalid")
            if "id" in message:
                client_requests.append(message)
            else:
                client_notifications.append(message)
        elif event["direction"] == "server":
            if "method" not in message and "id" not in message:
                raise NativeHostError("native-server-message-invalid")
            if "method" in message and "id" in message:
                raise NativeHostError("native-server-request-not-supported")
            server_messages.append(message)
        else:
            raise NativeHostError("native-event-direction-invalid")

    expected_methods = ["initialize", "thread/start", "turn/start"]
    if [message.get("method") for message in client_requests] != expected_methods:
        raise NativeHostError("native-client-request-sequence")
    if client_notifications != [{"jsonrpc": "2.0", "method": "initialized", "params": {}}]:
        raise NativeHostError("native-client-notification-sequence")
    init, start_thread, start_turn = client_requests
    init_params = init.get("params")
    if (
        init.get("id") != 1
        or type(init_params) is not dict
        or set(init_params) != {"clientInfo", "capabilities"}
        or init_params.get("clientInfo") != {"name": "coverage-native-grader", "version": "1"}
        or init_params.get("capabilities") != {"experimentalApi": True}
    ):
        raise NativeHostError("native-initialize-capabilities")
    thread_params = start_thread.get("params")
    if (
        start_thread.get("id") != 2
        or type(thread_params) is not dict
        or set(thread_params)
        != {
            "model",
            "allowProviderModelFallback",
            "approvalPolicy",
            "sandbox",
            "ephemeral",
            "environments",
            "dynamicTools",
            "cwd",
        }
        or thread_params.get("model") != expected_model
        or thread_params.get("allowProviderModelFallback") is not False
        or thread_params.get("approvalPolicy") != "never"
        or thread_params.get("sandbox") != "read-only"
        or thread_params.get("ephemeral") is not True
        or thread_params.get("environments") != []
        or thread_params.get("dynamicTools") != []
    ):
        raise NativeHostError("native-thread-start-policy")
    turn_params = start_turn.get("params")
    if (
        start_turn.get("id") != 3
        or type(turn_params) is not dict
        or set(turn_params) != {"threadId", "model", "input"}
        or turn_params.get("threadId") is None
        or turn_params.get("model") != expected_model
        or type(turn_params.get("input")) is not list
        or len(turn_params["input"]) != 1
        or type(turn_params["input"][0]) is not dict
        or set(turn_params["input"][0]) != {"type", "text"}
        or turn_params["input"][0].get("type") != "text"
        or type(turn_params["input"][0].get("text")) is not str
    ):
        raise NativeHostError("native-turn-start-binding")
    input_bytes = turn_params["input"][0]["text"].encode("utf-8")
    if len(input_bytes) != value["packet_byte_count"] or _sha(input_bytes) != packet_sha256:
        raise NativeHostError("native-input-packet-bytes-mismatch")

    response_rows = [message for message in server_messages if "id" in message]
    response_ids = [message.get("id") for message in response_rows]
    if len(response_ids) != len(set(response_ids)):
        raise NativeHostError("native-server-response-duplicate")
    responses = {message.get("id"): message for message in response_rows}
    if set(responses) != {1, 2, 3}:
        raise NativeHostError("native-server-response-inventory")
    if any("error" in responses[request_id] for request_id in (1, 2, 3)):
        raise NativeHostError("native-server-request-failed")
    thread_result = responses[2].get("result")
    if (
        type(thread_result) is not dict
        or thread_result.get("model") != expected_model
        or type(thread_result.get("modelProvider")) is not str
        or not thread_result["modelProvider"]
        or type(thread_result.get("thread")) is not dict
        or type(thread_result["thread"].get("id")) is not str
    ):
        raise NativeHostError("native-thread-result-model-mismatch")
    thread_id = thread_result["thread"]["id"]
    if turn_params.get("threadId") != thread_id:
        raise NativeHostError("native-turn-thread-mismatch")
    turn_result = responses[3].get("result")
    if type(turn_result) is not dict or type(turn_result.get("turn")) is not dict:
        raise NativeHostError("native-turn-start-response-invalid")
    turn_id = turn_result["turn"].get("id")
    if type(turn_id) is not str or not _UUID.fullmatch(turn_id):
        raise NativeHostError("native-turn-id-invalid")

    started = []
    completed = []
    reroutes = []
    for message in server_messages:
        method = message.get("method")
        params = message.get("params")
        if method == "model/rerouted":
            reroutes.append(message)
        elif method == "turn/started" and type(params) is dict:
            if params.get("threadId") == thread_id:
                started.append(params.get("turn"))
        elif method == "turn/completed" and type(params) is dict:
            if params.get("threadId") == thread_id:
                completed.append(params.get("turn"))
    if reroutes or len(started) != 1 or len(completed) != 1:
        raise NativeHostError("native-turn-event-chain-invalid")
    if type(started[0]) is not dict or started[0].get("id") != turn_id:
        raise NativeHostError("native-turn-start-event-mismatch")
    turn = completed[0]
    if type(turn) is not dict or turn.get("id") != turn_id or turn.get("status") != "completed":
        raise NativeHostError("native-turn-not-complete")
    items_view = turn.get("itemsView", "full")
    if items_view != "full":
        raise NativeHostError("native-completed-items-not-full")
    items = turn.get("items")
    if type(items) is not list:
        raise NativeHostError("native-completed-items-missing")
    user_items = [item for item in items if type(item) is dict and item.get("type") == "userMessage"]
    assistant_items = [item for item in items if type(item) is dict and item.get("type") == "agentMessage"]
    if len(user_items) != 1 or len(assistant_items) != 1:
        raise NativeHostError("native-message-inventory-invalid")
    if any(
        type(item) is not dict or item.get("type") not in {"userMessage", "agentMessage", "reasoning"} for item in items
    ):
        raise NativeHostError("native-tool-or-unsupported-item-present")
    user_content = user_items[0].get("content")
    if (
        type(user_content) is not list
        or len(user_content) != 1
        or type(user_content[0]) is not dict
        or user_content[0].get("type") != "text"
        or user_content[0].get("text") != turn_params["input"][0]["text"]
    ):
        raise NativeHostError("native-persisted-input-mismatch")
    response = assistant_items[0].get("text")
    if type(response) is not str or not response:
        raise NativeHostError("native-assistant-response-missing")
    response_bytes = response.encode("utf-8")
    if len(response_bytes) > 2_000_000:
        raise NativeHostError("native-assistant-response-over-cap")
    return VerifiedTranscript(thread_id, turn_id, thread_result["modelProvider"], response_bytes)


async def _read_message(reader: asyncio.StreamReader, *, deadline: float) -> dict[str, Any]:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise NativeHostError("native-host-deadline-expired")
    try:
        raw = await asyncio.wait_for(reader.readline(), timeout=remaining)
    except TimeoutError as exc:
        raise NativeHostError("native-host-deadline-expired") from exc
    if not raw or len(raw) > MAX_EVENT_LINE_BYTES or not raw.endswith(b"\n"):
        raise NativeHostError("native-host-event-line-invalid")
    value = _strict(raw[:-1], maximum=MAX_EVENT_LINE_BYTES)
    if type(value) is not dict:
        raise NativeHostError("native-host-event-not-object")
    return value


async def _write_request(writer: asyncio.StreamWriter, request: dict[str, object]) -> None:
    writer.write(_canonical(request) + b"\n")
    await writer.drain()


async def _invoke_codex(
    *,
    codex_executable: str,
    packet_bytes: bytes,
    stage_uuid: str,
    phase: str,
    packet_id: str,
    expected_catalog_row_sha256: str | None,
    expected_catalog_response_sha256: str,
    cli_executable_sha256: str,
    cli_version: str,
    deadline: float,
) -> tuple[bytes, VerifiedTranscript]:
    try:
        packet_text = packet_bytes.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise NativeHostError("native-packet-not-utf8") from exc
    events: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="coverage-native-grader-") as cwd:
        process = await asyncio.create_subprocess_exec(
            codex_executable,
            "app-server",
            "--listen",
            "stdio://",
            cwd=cwd,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            limit=MAX_EVENT_LINE_BYTES + 1,
        )
        assert process.stdin is not None and process.stdout is not None
        try:
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
                        "model": MODEL,
                        "allowProviderModelFallback": False,
                        "approvalPolicy": "never",
                        "sandbox": "read-only",
                        "ephemeral": True,
                        "environments": [],
                        "dynamicTools": [],
                        "cwd": cwd,
                    },
                },
            ]
            for request in requests:
                events.append({"direction": "client", "message": request})
                await _write_request(process.stdin, request)
                if "id" not in request:
                    continue
                while True:
                    message = await _read_message(process.stdout, deadline=deadline)
                    events.append({"direction": "server", "message": message})
                    if message.get("id") == request["id"]:
                        if "error" in message:
                            raise NativeHostError("native-app-server-request-failed")
                        if request["id"] == 1:
                            break
                        if request["id"] == 2:
                            if message.get("result", {}).get("model") != MODEL:
                                raise NativeHostError("native-thread-model-not-selected")
                            thread_id = message.get("result", {}).get("thread", {}).get("id")
                            if type(thread_id) is not str:
                                raise NativeHostError("native-thread-id-missing")
                            break
            turn_start = {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "turn/start",
                "params": {"threadId": thread_id, "model": MODEL, "input": [{"type": "text", "text": packet_text}]},
            }
            events.append({"direction": "client", "message": turn_start})
            await _write_request(process.stdin, turn_start)
            turn_id = None
            turn_start_response = False
            while True:
                message = await _read_message(process.stdout, deadline=deadline)
                events.append({"direction": "server", "message": message})
                if message.get("id") == 3:
                    if "error" in message:
                        raise NativeHostError("native-turn-start-rejected")
                    turn_id = message.get("result", {}).get("turn", {}).get("id")
                    turn_start_response = True
                if message.get("method") == "turn/completed":
                    params = message.get("params")
                    if type(params) is dict and params.get("threadId") == thread_id:
                        if not turn_start_response or type(turn_id) is not str:
                            raise NativeHostError("native-turn-completed-before-start")
                        break
            transcript = {
                "schema": TRANSCRIPT_SCHEMA,
                "stage_uuid": stage_uuid,
                "phase": phase,
                "packet_id": packet_id,
                "packet_sha256": _sha(packet_bytes),
                "packet_byte_count": len(packet_bytes),
                "model": MODEL,
                "model_catalog_row_sha256": expected_catalog_row_sha256,
                "model_catalog_response_sha256": expected_catalog_response_sha256,
                "cli_executable_sha256": cli_executable_sha256,
                "cli_version": cli_version,
                "events": events,
            }
            transcript_bytes = _canonical(transcript)
            verified = verify_transcript(
                transcript_bytes,
                stage_uuid=stage_uuid,
                phase=phase,
                packet_id=packet_id,
                packet_sha256=_sha(packet_bytes),
                expected_catalog_row_sha256=expected_catalog_row_sha256,
                expected_catalog_response_sha256=expected_catalog_response_sha256,
            )
            return transcript_bytes, verified
        finally:
            process.stdin.close()
            if process.returncode is None:
                process.terminate()
                try:
                    await asyncio.wait_for(process.wait(), timeout=2)
                except TimeoutError:
                    process.kill()
                    await process.wait()


async def _request_model_catalog(codex_executable: str, *, deadline: float) -> dict[str, object]:
    """Read local catalog metadata. It does not invoke a model or gate dispatch."""
    with tempfile.TemporaryDirectory(prefix="coverage-native-catalog-") as cwd:
        process = await asyncio.create_subprocess_exec(
            codex_executable,
            "app-server",
            "--listen",
            "stdio://",
            cwd=cwd,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            limit=MAX_EVENT_LINE_BYTES + 1,
        )
        assert process.stdin is not None and process.stdout is not None
        try:
            initialize = {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"clientInfo": {"name": "coverage-native-grader", "version": "1"}},
            }
            await _write_request(process.stdin, initialize)
            response = await _read_message(process.stdout, deadline=deadline)
            if response.get("id") != 1 or "error" in response:
                raise NativeHostError("native-app-server-initialize-failed")
            await _write_request(process.stdin, {"jsonrpc": "2.0", "method": "initialized", "params": {}})
            request = {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "model/list",
                "params": {"includeHidden": True, "limit": 500},
            }
            await _write_request(process.stdin, request)
            while True:
                response = await _read_message(process.stdout, deadline=deadline)
                if response.get("id") == 2:
                    if "error" in response:
                        raise NativeHostError("native-model-list-request-failed")
                    return response
        finally:
            process.stdin.close()
            if process.returncode is None:
                process.terminate()
                try:
                    await asyncio.wait_for(process.wait(), timeout=2)
                except TimeoutError:
                    process.kill()
                    await process.wait()


def _cli_identity(executable: str, expected_sha256: str) -> tuple[str, str]:
    path = Path(executable).resolve(strict=True)
    if not path.is_file() or path.is_symlink() or not _SHA.fullmatch(expected_sha256):
        raise NativeHostError("native-cli-identity-invalid")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1_048_576):
            digest.update(block)
    actual = digest.hexdigest()
    if actual != expected_sha256:
        raise NativeHostError("native-cli-source-pin-mismatch")
    try:
        import subprocess

        result = subprocess.run([str(path), "--version"], capture_output=True, timeout=5, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise NativeHostError("native-cli-version-unavailable") from exc
    version = result.stdout.decode("utf-8", "strict").strip()
    if result.returncode != 0 or not version or len(version) > 128:
        raise NativeHostError("native-cli-version-invalid")
    return str(path), version


def _load_scope(
    scope: Path, *, stage_uuid: str, phase: str
) -> tuple[bytes, dict[str, object], list[dict[str, object]]]:
    manifest_raw = _read_private(scope / "handoff-manifest.json", maximum=1_000_000)
    manifest = _strict(manifest_raw, maximum=1_000_000)
    if type(manifest) is not dict or set(manifest) != {
        "schema",
        "stage_uuid",
        "phase",
        "model",
        "status",
        "packet_count",
        "packets",
    }:
        raise NativeHostError("native-handoff-manifest-invalid")
    if (
        manifest.get("schema") != "coverage-native-grader-handoff/1"
        or manifest.get("stage_uuid") != stage_uuid
        or manifest.get("phase") != phase
        or manifest.get("model") != MODEL
        or manifest.get("status") != "awaiting-native-tool-results"
        or type(manifest.get("packets")) is not list
        or manifest.get("packet_count") != len(manifest["packets"])
        or not manifest["packets"]
        or len(manifest["packets"]) > 80
        or _canonical(manifest) != manifest_raw
    ):
        raise NativeHostError("native-handoff-manifest-binding")
    rows = manifest["packets"]
    ids = []
    for row in rows:
        if (
            type(row) is not dict
            or set(row) != {"packet_id", "task_id", "assessor_id", "role", "packet_sha256", "byte_count"}
            or type(row.get("packet_id")) is not str
            or not _SHA.fullmatch(row["packet_id"])
            or type(row.get("packet_sha256")) is not str
            or not _SHA.fullmatch(row["packet_sha256"])
            or type(row.get("byte_count")) is not int
            or not 1 <= row["byte_count"] <= MAX_PACKET_BYTES
            or type(row.get("task_id")) is not str
            or not row["task_id"]
            or row.get("role") not in {"card", "source", "answer"}
            or row.get("assessor_id") not in {"A", "B", "R1", "R2"}
            or row["packet_id"] in ids
        ):
            raise NativeHostError("native-handoff-packet-inventory-invalid")
        ids.append(row["packet_id"])
    if phase == "answers" and (len(rows) != 32 or any(row["role"] != "answer" for row in rows)):
        raise NativeHostError("native-answer-packet-inventory-invalid")
    request_files = sorted((scope / "requests").glob("*.request.json"))
    if [path.name for path in request_files] != sorted(f"{packet_id}.request.json" for packet_id in ids):
        raise NativeHostError("native-request-file-inventory-invalid")
    requests = []
    for row in rows:
        request_raw = _read_private(
            scope / "requests" / f"{row['packet_id']}.request.json", maximum=MAX_PACKET_BYTES * 2
        )
        request = _strict(request_raw, maximum=MAX_PACKET_BYTES * 2)
        expected = {
            "schema": "coverage-native-grader-handoff/1",
            "stage_uuid": stage_uuid,
            "phase": phase,
            "model": MODEL,
            **row,
            "packet_bytes_base64": None,
        }
        if type(request) is not dict or set(request) != set(expected):
            raise NativeHostError("native-request-schema-invalid")
        import base64

        encoded = request.pop("packet_bytes_base64")
        try:
            packet_bytes = base64.b64decode(encoded, validate=True)
        except (TypeError, ValueError) as exc:
            raise NativeHostError("native-request-packet-encoding") from exc
        if (
            request != {key: value for key, value in expected.items() if key != "packet_bytes_base64"}
            or len(packet_bytes) != row["byte_count"]
            or _sha(packet_bytes) != row["packet_sha256"]
        ):
            raise NativeHostError("native-request-packet-binding")
        payload = _strict(packet_bytes, maximum=MAX_PACKET_BYTES)
        if (
            _canonical(payload) != packet_bytes
            or type(payload) is not dict
            or payload.get("packet_id") != row["packet_id"]
            or payload.get("task_id") != row["task_id"]
            or payload.get("assessor_id") != row["assessor_id"]
        ):
            raise NativeHostError("native-request-packet-identity")
        if phase == "answers":
            if payload.get("packet_kind") != "answer" or "role" in payload:
                raise NativeHostError("native-request-packet-identity")
        elif payload.get("role") != row["role"]:
            raise NativeHostError("native-request-packet-identity")
        requests.append({"row": row, "bytes": packet_bytes, "request_sha256": _sha(request_raw)})
    return manifest_raw, manifest, requests


async def dispatch_handoff_scope(
    *,
    scope_path: str | os.PathLike[str],
    stage_uuid: str,
    phase: str,
    codex_executable: str,
    expected_cli_sha256: str,
    deadline_monotonic: float,
) -> dict[str, object]:
    """Dispatch one immutable handoff scope with a two-call ceiling.

    This uses the local Codex CLI/app-server config. It does not alter that
    configuration, infer model availability from catalog metadata, or retry.
    """
    if phase not in {"references", "answers"} or type(deadline_monotonic) not in {int, float}:
        raise NativeHostError("native-dispatch-input-invalid")
    if time.monotonic() >= deadline_monotonic:
        raise NativeHostError("native-dispatch-deadline-invalid")
    scope = Path(scope_path)
    if scope.is_symlink() or not scope.is_dir() or stat.S_IMODE(scope.stat(follow_symlinks=False).st_mode) & 0o077:
        raise NativeHostError("native-handoff-scope-not-private")
    audit_root = scope / "native-host"
    if audit_root.exists() or audit_root.is_symlink():
        raise NativeHostError("native-host-scope-already-consumed")
    existing_results = list((scope / "results").glob("*.result.json"))
    if existing_results:
        raise NativeHostError("native-result-inventory-not-empty")
    executable_path, cli_version = _cli_identity(codex_executable, expected_cli_sha256)
    manifest_raw, _manifest, requests = _load_scope(scope, stage_uuid=stage_uuid, phase=phase)
    catalog = await _request_model_catalog(executable_path, deadline=deadline_monotonic)
    catalog_row_sha256, catalog_response_sha256 = inspect_model_catalog(catalog)
    try:
        audit_root.mkdir(mode=0o700)
    except FileExistsError as exc:
        raise NativeHostError("native-host-scope-already-consumed") from exc
    _write_exclusive(audit_root / "model-list.response.json", _canonical(catalog))
    catalog_file_sha256 = _sha(_canonical(catalog))
    claim_root = audit_root / "claims"
    transcript_root = audit_root / "transcripts"
    failure_root = audit_root / "failures"
    for root in (claim_root, transcript_root, failure_root):
        root.mkdir(mode=0o700)
    _ = manifest_raw
    claimed: set[str] = set()
    completed: set[str] = set()
    failures: dict[str, str] = {}
    stop = asyncio.Event()
    next_index = 0
    index_lock = asyncio.Lock()
    activity_lock = asyncio.Lock()
    active_calls = 0
    maximum_concurrent_calls = 0

    async def worker() -> None:
        nonlocal next_index, active_calls, maximum_concurrent_calls
        while not stop.is_set():
            async with index_lock:
                if stop.is_set() or next_index >= len(requests):
                    return
                request = requests[next_index]
                next_index += 1
            row = request["row"]
            packet_id = row["packet_id"]
            claim = {
                "schema": "coverage-native-host-claim/1",
                "stage_uuid": stage_uuid,
                "phase": phase,
                "packet_id": packet_id,
                "packet_sha256": row["packet_sha256"],
                "request_sha256": request["request_sha256"],
                "cli_executable_sha256": expected_cli_sha256,
                "model": MODEL,
                "reserved_before_dispatch": True,
            }
            active = False
            try:
                _write_exclusive(claim_root / f"{packet_id}.claim.json", _canonical(claim))
                claimed.add(packet_id)
                async with activity_lock:
                    active_calls += 1
                    maximum_concurrent_calls = max(maximum_concurrent_calls, active_calls)
                    active = True
                transcript_bytes, verified = await _invoke_codex(
                    codex_executable=executable_path,
                    packet_bytes=request["bytes"],
                    stage_uuid=stage_uuid,
                    phase=phase,
                    packet_id=packet_id,
                    expected_catalog_row_sha256=catalog_row_sha256,
                    expected_catalog_response_sha256=catalog_response_sha256,
                    cli_executable_sha256=expected_cli_sha256,
                    cli_version=cli_version,
                    deadline=deadline_monotonic,
                )
                async with activity_lock:
                    active_calls -= 1
                    active = False
                if time.monotonic() >= deadline_monotonic:
                    raise NativeHostError("native-dispatch-deadline-expired")
                _write_exclusive(transcript_root / f"{packet_id}.transcript.json", transcript_bytes)
                publish_native_tool_result(
                    scope=scope,
                    packet_id=packet_id,
                    tool_call_id=verified.turn_id,
                    model=MODEL,
                    response_bytes=verified.response_bytes,
                )
                completed.add(packet_id)
            except Exception as exc:
                if active:
                    async with activity_lock:
                        active_calls -= 1
                        active = False
                failures[packet_id] = (
                    exc.args[0] if isinstance(exc, NativeHostError) and exc.args else "native-dispatch-failed"
                )
                stop.set()
                try:
                    _write_exclusive(
                        failure_root / f"{packet_id}.failure.json",
                        _canonical(
                            {
                                "schema": "coverage-native-host-failure/1",
                                "packet_id": packet_id,
                                "reason": failures[packet_id],
                            }
                        ),
                    )
                except NativeHostError:
                    pass

    await asyncio.gather(*(worker() for _ in range(MAX_IN_FLIGHT)))
    status = "complete" if len(completed) == len(requests) else "failed-terminal"
    summary = {
        "schema": "coverage-native-host-dispatch-summary/1",
        "stage_uuid": stage_uuid,
        "phase": phase,
        "model": MODEL,
        "model_list_response_sha256": catalog_file_sha256,
        "model_catalog_advertised_exact_model": catalog_row_sha256 is not None,
        "cli_executable_sha256": expected_cli_sha256,
        "cli_version": cli_version,
        "maximum_concurrent_calls": maximum_concurrent_calls,
        "packet_count": len(requests),
        "claimed_packet_ids": sorted(claimed),
        "completed_packet_ids": sorted(completed),
        "unstarted_packet_ids": sorted({row["row"]["packet_id"] for row in requests} - claimed),
        "failure_reasons": failures,
        "status": status,
    }
    _write_exclusive(audit_root / "dispatch-summary.json", _canonical(summary))
    if status != "complete":
        raise NativeHostError("native-dispatch-incomplete-terminal")
    return summary


def main(argv: list[str] | None = None) -> int:
    """Run the local host dispatcher for one already-created handoff phase."""
    import argparse

    parser = argparse.ArgumentParser(description="Dispatch one frozen grader handoff through Codex app-server.")
    parser.add_argument("--scope", required=True, help="absolute private phase scope containing handoff-manifest.json")
    parser.add_argument("--stage-uuid", required=True)
    parser.add_argument("--phase", required=True, choices=("references", "answers"))
    parser.add_argument("--codex-executable", required=True, help="absolute pinned Codex CLI executable path")
    parser.add_argument("--codex-sha256", required=True, help="out-of-band SHA-256 of the executable")
    parser.add_argument(
        "--deadline-seconds",
        required=True,
        type=int,
        help="remaining registered-stage time, bounded to 1..28800 seconds",
    )
    args = parser.parse_args(argv)
    if not 1 <= args.deadline_seconds <= 28_800:
        parser.error("--deadline-seconds must be in 1..28800")
    try:
        summary = asyncio.run(
            dispatch_handoff_scope(
                scope_path=args.scope,
                stage_uuid=args.stage_uuid,
                phase=args.phase,
                codex_executable=args.codex_executable,
                expected_cli_sha256=args.codex_sha256,
                deadline_monotonic=time.monotonic() + args.deadline_seconds,
            )
        )
    except NativeHostError as exc:
        print(f"native host dispatch stopped: {exc}")
        return 1
    print(
        json.dumps(
            {
                "status": summary["status"],
                "stage_uuid": summary["stage_uuid"],
                "phase": summary["phase"],
                "packet_count": summary["packet_count"],
                "maximum_concurrent_calls": summary["maximum_concurrent_calls"],
                "cli_executable_sha256": summary["cli_executable_sha256"],
                "cli_version": summary["cli_version"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
