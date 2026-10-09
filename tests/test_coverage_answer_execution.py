from __future__ import annotations

import hashlib
import json
import os
import uuid
from datetime import datetime, timedelta, timezone

import httpx
import pytest

from scripts import coverage_answer_execution as execution
from scripts import coverage_consumer_inputs as consumer
from scripts import coverage_stage_orchestration as orchestration


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _pair(task_index: int, task_id: str) -> execution.AnswerTask:
    context = "The release process signs artifacts and records provenance. " * 8
    context_sha = _sha(context.encode())
    captures = {"source-1": {"state": "success", "context": context, "context_sha256": context_sha}}
    capture_sha = _sha(json.dumps(captures, sort_keys=True, separators=(",", ":")).encode())
    checks = (
        {"id": f"{task_id}-facet-C1", "description": "Which provenance evidence is documented?"},
        {"id": f"{task_id}-facet-C2", "description": "What limitations remain?"},
    )
    pairs = consumer.prepare_pair(
        task_index=task_index,
        task_id=task_id,
        task_question="How does the documented release process record provenance?",
        purpose="Find evidence about release provenance and its limitations.",
        critical_checks=[dict(row) for row in checks],
        card_ids=["card-a", "card-b"],
        source_id_by_card_id={"card-a": "source-1", "card-b": "source-1"},
        captures=captures,
        expected_capture_inventory_sha256=capture_sha,
        orders={"w0": ["card-a", "card-b"], "candidate": ["card-b", "card-a"]},
    )
    return execution.AnswerTask(
        task_id=task_id,
        kind="research",
        query="How does the documented release process record provenance?",
        purpose="Find evidence about release provenance and its limitations.",
        critical_checks=checks,
        answers=pairs.answers,
        catalog_bytes=pairs.private_catalog_bytes,
        catalog_sha256=pairs.catalog_sha256,
    )


def _tasks() -> tuple[execution.AnswerTask, ...]:
    return tuple(_pair(index, f"task-{index}") for index in range(1, 9))


class _Verifier:
    def verify(self, receipt_bytes, expected_receipt_sha256, bindings):
        assert _sha(receipt_bytes) == expected_receipt_sha256
        self.bindings = dict(bindings)
        return execution.VerifiedAnswerPermit(
            status="verified-admitted",
            stage_uuid=bindings["stage_uuid"],
            source_revision=bindings["source_revision"],
            protocol_sha256=bindings["protocol_sha256"],
            cohorts_sha256=bindings["cohorts_sha256"],
            operation_manifest_sha256=bindings["operation_manifest_sha256"],
            endpoint_sha256=bindings["endpoint_sha256"],
            endpoint_security_mode=bindings["endpoint_security_mode"],
            operation_ids=bindings["operation_ids"],
            max_calls=18,
            request_bytes_maximum=384_000,
            response_bytes_maximum=2_000_000,
            receipt_sha256=expected_receipt_sha256,
        )


class _Lease:
    def __init__(self):
        self.seen = []

    def consume_once(self, permit, *, operation_id, request_sha256):
        self.seen.append((operation_id, request_sha256))
        return {
            "status": "consumed",
            "stage_uuid": permit.stage_uuid,
            "operation_id": operation_id,
            "request_sha256": request_sha256,
            "receipt_sha256": "a" * 64,
        }


def _response(content: str) -> httpx.Response:
    envelope = {"choices": [{"message": {"content": content}}], "usage": {"prompt_tokens": 12}}
    return httpx.Response(200, content=json.dumps(envelope).encode())


def _answer_content(task: execution.AnswerTask) -> str:
    return json.dumps(
        {
            "facets": [
                {"facet_id": check["id"], "conclusion": "Evidence is limited.", "claims": []}
                for check in task.critical_checks
            ]
        }
    )


def _stage_inputs(tmp_path, *, tasks=None, permit_bytes=b"external admitted receipt"):
    tasks = _tasks() if tasks is None else tasks
    requests = execution._make_requests(tasks)
    manifest, manifest_sha = execution._request_manifest(requests)
    stage_uuid = str(uuid.uuid4())
    now = datetime.now(timezone.utc)
    start = now - timedelta(seconds=1)
    deadline = start + timedelta(hours=8)
    roots = []
    for name in ("leases", "archives", "results"):
        root = tmp_path / name
        root.mkdir(mode=0o700)
        os.chmod(root, 0o700)
        roots.append(root)
    return {
        "stage_uuid": stage_uuid,
        "source_revision": "b" * 40,
        "protocol_sha256": "c" * 64,
        "cohorts_sha256": "d" * 64,
        "answer_manifest_sha256": manifest_sha,
        "stage_started_utc": start.isoformat(),
        "stage_deadline_utc": deadline.isoformat(),
        "tasks": tasks,
        "endpoint": "https://answer.example/v1",
        "api_key": "synthetic-test-key",
        "permit_bytes": permit_bytes,
        "expected_permit_sha256": _sha(permit_bytes),
        "permit_verifier": _Verifier(),
        "one_shot_lease": _Lease(),
        "lease_root": roots[0],
        "archive_root": roots[1],
        "result_root": roots[2],
        "transport": None,
    }


@pytest.mark.asyncio
async def test_executes_two_readiness_and_sixteen_paired_answers_serially(tmp_path):
    args = _stage_inputs(tmp_path)
    dispatches = []
    task_by_id = {task.task_id: task for task in args["tasks"]}

    async def handler(request):
        body = json.loads(request.content)
        dispatches.append(body)
        user = body["messages"][1]["content"]
        if user.startswith("This is a synthetic endpoint readiness check"):
            return _response('{"ready":true}')
        task_id = json.loads(user)["task_id"]
        return _response(_answer_content(task_by_id[task_id]))

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)

    assert result.status == "complete-structurally-valid-not-semantically-graded"
    assert result.owned_http_calls == 18
    assert len(dispatches) == 18
    assert len(args["one_shot_lease"].seen) == 18
    assert all(call["model"] == "free" for call in dispatches)
    assert all(call["max_tokens"] == 8192 for call in dispatches)
    assert all(call["temperature"] == 0 for call in dispatches)
    assert all(call["response_format"] == {"type": "json_object"} for call in dispatches)
    assert result.usage_status == "reported"
    assert all(row["usage_status"] == "reported" for row in result.operations)
    assert all(row["provider_usage"] == {"prompt_tokens": 12} for row in result.operations)
    assert all(row["status"] in {"readiness-complete", "answer-structurally-valid"} for row in result.operations)
    terminal_path = args["result_root"] / f"{args['stage_uuid']}.answer-terminal-inventory.json"
    restored = {task.task_id: {} for task in args["tasks"]}
    for task in args["tasks"]:
        for arm in ("w0", "candidate"):
            operation_id = f"answer-{task.task_id}-{arm}"
            path = args["result_root"] / f"{args['stage_uuid']}.{operation_id}.answer.json"
            restored[task.task_id][arm] = json.loads(path.read_bytes())
    answer_inputs = {task.task_id: {answer.arm: answer.body for answer in task.answers} for task in args["tasks"]}
    evidence = orchestration.AnswerEvidence(
        result=result,
        answer_inputs=answer_inputs,
        restored_answers=restored,
        terminal_inventory_bytes=terminal_path.read_bytes(),
        archive_root=args["archive_root"],
        result_root=args["result_root"],
    )
    receipt = orchestration._validate_answer_evidence(
        evidence,
        args["tasks"],
        expected_inputs=answer_inputs,
        stage_uuid=args["stage_uuid"],
        source_revision=args["source_revision"],
    )
    assert len(receipt) == 64
    restored["task-1"]["w0"] = {"facets": []}
    with pytest.raises(orchestration.OrchestrationError, match="answerer-exposed-output-mismatch"):
        orchestration._validate_answer_evidence(
            evidence,
            args["tasks"],
            expected_inputs=answer_inputs,
            stage_uuid=args["stage_uuid"],
            source_revision=args["source_revision"],
        )


@pytest.mark.asyncio
async def test_http_failure_is_archived_and_stops_all_later_slots(tmp_path):
    args = _stage_inputs(tmp_path)
    calls = 0

    async def handler(_request):
        nonlocal calls
        calls += 1
        return httpx.Response(503, content=b"unavailable")

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)

    assert result.status == "terminal-incomplete"
    assert calls == result.owned_http_calls == 1
    assert result.operations[0]["status"] == "http-error"
    assert all(row["status"] == "not-invoked-after-terminal-stop" for row in result.operations[1:])
    archives = list((args["archive_root"] / args["stage_uuid"]).glob("*/response.bin"))
    assert len(archives) == 1 and archives[0].read_bytes() == b"unavailable"


@pytest.mark.asyncio
async def test_answer_validation_failure_is_terminal_without_repair(tmp_path):
    args = _stage_inputs(tmp_path)
    calls = 0

    async def handler(request):
        nonlocal calls
        calls += 1
        if calls <= 2:
            return _response('{"ready":true}')
        return _response('{"facets":[]}')

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)

    assert calls == result.owned_http_calls == 3
    assert result.operations[2]["status"] == "invalid-answer-output"
    assert all(row["status"] == "not-invoked-after-terminal-stop" for row in result.operations[3:])
    # The exact successful API response is durable before validation fails.
    archived = args["archive_root"] / args["stage_uuid"] / "answer-task-1-w0" / "response.bin"
    assert b'\\"facets\\":[]' in archived.read_bytes()


@pytest.mark.asyncio
async def test_reflected_key_is_suppressed_and_later_slots_are_uninvoked(tmp_path):
    args = _stage_inputs(tmp_path)
    calls = 0

    async def handler(_request):
        nonlocal calls
        calls += 1
        return httpx.Response(200, content=(b"partial " + args["api_key"].encode()))

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)

    assert calls == result.owned_http_calls == 1
    assert result.operations[0]["status"] == "credential-reflection-suppressed"
    assert result.operations[0]["terminal_reason"] == "credential-reflection-suppressed"
    terminal = json.loads((args["result_root"] / f"{args['stage_uuid']}.answer-terminal-inventory.json").read_bytes())
    assert terminal["terminal_reason"] == "credential-reflection-suppressed"
    response = next((args["archive_root"] / args["stage_uuid"]).glob("*/response.bin"))
    assert args["api_key"].encode() not in response.read_bytes()
    assert all(row["status"] == "not-invoked-after-terminal-stop" for row in result.operations[1:])


@pytest.mark.asyncio
async def test_partial_key_echo_suffix_is_also_suppressed(tmp_path):
    args = _stage_inputs(tmp_path)
    prefix = args["api_key"].encode()[:10]

    async def handler(_request):
        return httpx.Response(200, content=b"partial echo: " + prefix)

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)

    assert result.operations[0]["status"] == "credential-reflection-suppressed"
    assert result.operations[0]["terminal_reason"] == "credential-reflection-suppressed"
    terminal = json.loads((args["result_root"] / f"{args['stage_uuid']}.answer-terminal-inventory.json").read_bytes())
    assert terminal["terminal_reason"] == "credential-reflection-suppressed"
    assert all(row["status"] == "not-invoked-after-terminal-stop" for row in result.operations[1:])
    response = next((args["archive_root"] / args["stage_uuid"]).glob("*/response.bin"))
    assert prefix not in response.read_bytes()


@pytest.mark.asyncio
async def test_public_plaintext_endpoint_rejected_before_admission_or_lease(tmp_path):
    args = _stage_inputs(tmp_path)
    args["endpoint"] = "http://answer.example/v1"
    called = []
    args["permit_verifier"] = type("Verifier", (), {"verify": lambda *_args: called.append("permit")})()
    args["one_shot_lease"] = type("Lease", (), {"consume_once": lambda *_args, **_kwargs: called.append("lease")})()
    with pytest.raises(execution.AnswerExecutionError, match="answer-plaintext-endpoint-not-permitted"):
        await execution.execute_answer_stage(**args)
    assert called == []
    assert not list(args["lease_root"].iterdir())


def test_trusted_private_http_requires_explicit_option_and_binds_mode():
    for host in ("localhost", "127.0.0.1", "10.2.3.4", "agent-svc"):
        url, mode = execution._chat_url(f"http://{host}/v1", allow_trusted_private_http=True)
        assert url == f"http://{host}/v1/chat/completions"
        assert mode == "trusted-private-http"
    for host in ("answer.example", "8.8.8.8"):
        with pytest.raises(execution.AnswerExecutionError):
            execution._chat_url(f"http://{host}/v1", allow_trusted_private_http=True)
    requests = execution._make_requests(_tasks())
    https_manifest, _ = execution._request_manifest(requests)
    private_manifest, _ = execution._request_manifest(requests, endpoint_security_mode="trusted-private-http")
    assert https_manifest != private_manifest


@pytest.mark.asyncio
async def test_explicit_private_http_mode_is_bound_into_permit_and_lease(tmp_path):
    args = _stage_inputs(tmp_path)
    args["endpoint"] = "http://answer-svc/v1"
    args["allow_trusted_private_http"] = True
    args["answer_manifest_sha256"] = execution._request_manifest(
        execution._make_requests(args["tasks"]), endpoint_security_mode="trusted-private-http"
    )[1]
    dispatches = 0

    async def handler(request):
        nonlocal dispatches
        dispatches += 1
        body = json.loads(request.content)
        if body["messages"][1]["content"].startswith("This is a synthetic endpoint readiness check"):
            return _response('{"ready":true}')
        user_payload = json.loads(body["messages"][1]["content"])
        task = next(task for task in args["tasks"] if task.task_id == user_payload["task_id"])
        return _response(_answer_content(task))

    args["transport"] = httpx.MockTransport(handler)
    result = await execution.execute_answer_stage(**args)
    assert result.status == "complete-structurally-valid-not-semantically-graded"
    assert dispatches == 18
    assert args["permit_verifier"].bindings["endpoint_security_mode"] == "trusted-private-http"


@pytest.mark.asyncio
async def test_permit_binding_mismatch_stops_before_lease_or_http(tmp_path):
    args = _stage_inputs(tmp_path)
    args["answer_manifest_sha256"] = "0" * 64
    called = False

    async def handler(_request):
        nonlocal called
        called = True
        return _response('{"ready":true}')

    args["transport"] = httpx.MockTransport(handler)
    with pytest.raises(execution.AnswerExecutionError, match="answer-operation-manifest-pin-mismatch"):
        await execution.execute_answer_stage(**args)
    assert not called
    assert args["one_shot_lease"].seen == []


def test_plan_is_exactly_two_readiness_plus_ordered_pairs_and_uses_flat_schema():
    requests = execution._make_requests(_tasks())
    assert len(requests) == 18
    assert [request.kind for request in requests[:2]] == ["readiness", "readiness"]
    assert [request.operation_id for request in requests[2:6]] == [
        "answer-task-1-w0",
        "answer-task-1-candidate",
        "answer-task-2-w0",
        "answer-task-2-candidate",
    ]
    body = json.loads(requests[2].body)
    user = json.loads(body["messages"][1]["content"])
    assert set(user) == {
        "task_id",
        "kind",
        "query",
        "purpose",
        "critical_checks",
        "cards",
        "outcomes",
        "sources",
        "schema",
    }
    assert "arm" not in user and "catalog_sha256" not in user


def test_provider_usage_preserves_only_reported_strict_nonnegative_integers():
    assert execution._reported_usage({}) == ("unavailable", {})
    assert execution._reported_usage({"usage": {"prompt_tokens": 0}}) == ("reported", {"prompt_tokens": 0})
    assert execution._reported_usage({"usage": {"completion_tokens": 7}}) == (
        "reported",
        {"completion_tokens": 7},
    )
    assert execution._reported_usage({"usage": {"prompt_tokens": True}}) == ("invalid", {})
    assert execution._reported_usage({"usage": {"total_tokens": -1}}) == ("invalid", {})
