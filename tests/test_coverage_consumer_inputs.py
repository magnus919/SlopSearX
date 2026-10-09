"""Equal source budgets and assigned-citation boundaries for paired consumers."""

import hashlib
import json

import pytest

from scripts.coverage_consumer_inputs import (
    ConsumerInputError,
    prepare_pair,
    restore_answer_citations,
    task_capture_contexts,
    validate_answer,
)


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _fixture():
    ids = [f"c{i}" for i in range(12)]
    captures = {
        f"s{i}": {
            "state": "success",
            "context": f"Public evidence {i}. " + "α" * 605,
            "context_sha256": _sha((f"Public evidence {i}. " + "α" * 605).encode()),
        }
        for i in range(12)
    }
    return {
        "task_index": 1,
        "task_id": "D-R01",
        "task_question": "What does the public evidence establish?",
        "purpose": "Compare supported limitations.",
        "critical_checks": [{"id": "f1", "description": "State supported limits."}],
        "card_ids": ids,
        "source_id_by_card_id": dict(zip(ids, captures)),
        "captures": captures,
        "expected_capture_inventory_sha256": _sha(
            json.dumps(captures, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        ),
        "orders": {"w0": ids, "candidate": list(reversed(ids))},
    }


def _rebind(kwargs):
    kwargs["expected_capture_inventory_sha256"] = _sha(
        json.dumps(
            kwargs["captures"], ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
        ).encode()
    )


def test_equal_budgets_order_is_only_treatment_and_no_arm_label_is_visible():
    pair = prepare_pair(**_fixture())
    w0, candidate = pair.answers
    assert w0.delivered_source_ids == tuple(f"s{i}" for i in range(5))
    assert candidate.delivered_source_ids == tuple(f"s{i}" for i in range(11, 6, -1))
    for assignment in pair.answers:
        body = json.loads(assignment.body)
        assert "arm" not in body and "arm_id" not in body
        assert len(body["cards"]) == 10
        assert len(body["opening_outcomes"]) == 10
        assert len(body["delivered_sources"]) == 5
        assert assignment.body_sha256 == _sha(assignment.body)
    assert json.loads(w0.body)["task"] == json.loads(candidate.body)["task"]


def test_no_rescue_beyond_prefix_and_failures_remain_explicit():
    kwargs = _fixture()
    for i in range(10):
        kwargs["captures"][f"s{i}"] = {"state": "barrier_detected", "context": None, "context_sha256": None}
    _rebind(kwargs)
    pair = prepare_pair(**kwargs)
    assert not pair.answers[0].delivered_source_ids
    outcomes = json.loads(pair.answers[0].body)["opening_outcomes"]
    assert {row["state"] for row in outcomes} == {"barrier_detected"}
    assert len(pair.answers[1].delivered_source_ids) == 2


def test_same_source_has_same_codes_across_arms_and_unicode_offsets_are_lossless():
    kwargs = _fixture()
    kwargs["orders"]["candidate"] = list(kwargs["orders"]["w0"])
    pair = prepare_pair(**kwargs)
    assert pair.answers[0].body == pair.answers[1].body
    catalog = json.loads(pair.private_catalog_bytes)
    for source_id in pair.answers[0].delivered_source_ids:
        rows = [row for row in catalog.values() if row["source_id"] == source_id]
        assert "".join(row["text"] for row in rows) == kwargs["captures"][source_id]["context"]
        for row in rows:
            assert kwargs["captures"][source_id]["context"][row["start"] : row["end"]] == row["text"]


def test_duplicate_opening_does_not_consume_another_successful_source():
    kwargs = _fixture()
    kwargs["source_id_by_card_id"]["c1"] = "s0"
    del kwargs["captures"]["s1"]
    _rebind(kwargs)
    pair = prepare_pair(**kwargs)
    body = json.loads(pair.answers[0].body)
    assert body["opening_outcomes"][1] == {"card_id": "c1", "state": "duplicate", "delivered": False}
    assert pair.answers[0].delivered_source_ids == ("s0", "s2", "s3", "s4", "s5")


@pytest.mark.parametrize(
    "mutation,reason",
    [
        ("inventory", "capture-pin"),
        ("context", "context-pin"),
        ("overlong", "successful-context-bound"),
        ("order", "order-not-complete-permutation"),
        ("missing", "capture-coverage"),
    ],
)
def test_input_drift_and_overflow_rejected_without_trimming(mutation, reason):
    kwargs = _fixture()
    if mutation == "inventory":
        kwargs["expected_capture_inventory_sha256"] = "0" * 64
    elif mutation == "context":
        kwargs["captures"]["s0"]["context"] += "changed"
        _rebind(kwargs)
    elif mutation == "overlong":
        kwargs["captures"]["s0"]["context"] = "a" * 8001
        _rebind(kwargs)
    elif mutation == "order":
        kwargs["orders"]["candidate"] = kwargs["card_ids"][:10]
    else:
        del kwargs["captures"]["s0"]
        _rebind(kwargs)
    with pytest.raises(ConsumerInputError, match=reason):
        prepare_pair(**kwargs)


def test_citation_from_other_arm_rejected_even_when_valid_in_full_catalog():
    pair = prepare_pair(**_fixture())
    wrong_code = pair.answers[1].delivered_evidence_ids[0]
    answer = {
        "facets": [
            {
                "facet_id": "f1",
                "conclusion": "Limited evidence.",
                "claims": [{"text": "A claim.", "evidence_ids": [wrong_code]}],
            }
        ]
    }
    with pytest.raises(ConsumerInputError, match="answer-undelivered-citation"):
        validate_answer(answer, pair.answers[0], critical_check_ids=["f1"])


def test_abstention_is_structural_only_and_word_overflow_is_not_repaired():
    assignment = prepare_pair(**_fixture()).answers[0]
    answer = {"facets": [{"facet_id": "f1", "conclusion": "Insufficient evidence.", "claims": []}]}
    receipt = validate_answer(answer, assignment, critical_check_ids=["f1"])
    assert receipt["status"] == "structurally-valid-not-semantically-graded"
    answer["facets"][0]["conclusion"] = "word " * 801
    with pytest.raises(ConsumerInputError, match="answer-word-cap"):
        validate_answer(answer, assignment, critical_check_ids=["f1"])


def test_missing_facet_stays_invalid():
    assignment = prepare_pair(**_fixture()).answers[0]
    with pytest.raises(ConsumerInputError, match="answer-facet-coverage"):
        validate_answer({"facets": []}, assignment, critical_check_ids=["f1"])


def _capture_join_fixture():
    text = "The same newly captured public page."
    owner = {
        "task_id": "task-a",
        "source_id": "s-shared",
        "url": "https://example.org/public",
        "result_index": 1,
        "title": "A public document",
        "engine": "github",
        "status": "captured",
        "context_artifact": "context-0001.json",
        "context_sha256": _sha(text.encode()),
        "context_characters": len(text),
    }
    duplicate = {
        **owner,
        "task_id": "task-b",
        "status": "duplicate_url_not_attempted",
        "context_artifact": None,
        "context_sha256": None,
    }
    inventory = {
        "schema": "coverage-source-capture-inventory/1",
        "stage_uuid": "new-stage",
        "status": "complete",
        "sources": [owner, duplicate],
    }
    artifact = {
        "schema": "coverage-source-context/1",
        "task_id": owner["task_id"],
        "source_id": owner["source_id"],
        "source_url": owner["url"],
        "result_index": 1,
        "title": owner["title"],
        "engine": owner["engine"],
        "context": text,
        "context_sha256": owner["context_sha256"],
        "context_characters": len(text),
    }
    raw = json.dumps(inventory).encode()
    return {
        "task_id": "task-b",
        "stage_uuid": "new-stage",
        "source_ids": ["s-shared"],
        "capture_inventory_bytes": raw,
        "expected_capture_inventory_sha256": _sha(raw),
        "context_artifacts": {"context-0001.json": json.dumps(artifact).encode()},
    }


def test_cross_task_duplicate_reuses_only_the_same_stage_exact_context():
    kwargs = _capture_join_fixture()
    duplicate = task_capture_contexts(**kwargs)
    kwargs["task_id"] = "task-a"
    original = task_capture_contexts(**kwargs)
    assert original == duplicate
    assert original["s-shared"]["state"] == "success"


@pytest.mark.parametrize(
    "mutation,reason",
    [("stage", "capture-stage-mismatch"), ("context", "capture-context-integrity"), ("partial", "capture-incomplete")],
)
def test_capture_join_rejects_old_stage_mutated_context_and_incomplete_inventory(mutation, reason):
    kwargs = _capture_join_fixture()
    if mutation == "stage":
        kwargs["stage_uuid"] = "old-stage"
    elif mutation == "context":
        artifact = json.loads(kwargs["context_artifacts"]["context-0001.json"])
        artifact["context"] += " altered"
        kwargs["context_artifacts"]["context-0001.json"] = json.dumps(artifact).encode()
    else:
        inventory = json.loads(kwargs["capture_inventory_bytes"])
        inventory["status"] = "terminal-incomplete"
        kwargs["capture_inventory_bytes"] = json.dumps(inventory).encode()
        kwargs["expected_capture_inventory_sha256"] = _sha(kwargs["capture_inventory_bytes"])
    with pytest.raises(ConsumerInputError, match=reason):
        task_capture_contexts(**kwargs)


def test_duplicate_without_successful_owner_is_unavailable_not_rescued():
    kwargs = _capture_join_fixture()
    inventory = json.loads(kwargs["capture_inventory_bytes"])
    inventory["sources"][0]["status"] = "source_response_failure"
    kwargs["capture_inventory_bytes"] = json.dumps(inventory).encode()
    kwargs["expected_capture_inventory_sha256"] = _sha(kwargs["capture_inventory_bytes"])
    result = task_capture_contexts(**kwargs)
    assert result["s-shared"] == {"state": "duplicate_url_not_attempted", "context": None, "context_sha256": None}


def test_original_passage_hash_restored_only_in_typed_citation_fields():
    pair = prepare_pair(**_fixture())
    assignment = pair.answers[0]
    code = assignment.delivered_evidence_ids[0]
    answer = {
        "facets": [
            {
                "facet_id": "f1",
                "conclusion": f"The code {code} is merely a label.",
                "claims": [{"text": f"Keep {code} in prose unchanged.", "evidence_ids": [code]}],
            }
        ]
    }
    restored = restore_answer_citations(
        answer,
        assignment,
        critical_check_ids=["f1"],
        catalog_bytes=pair.private_catalog_bytes,
        expected_catalog_sha256=pair.catalog_sha256,
    )
    catalog = json.loads(pair.private_catalog_bytes)
    original = catalog[code]["original_evidence_id"]
    assert len(original) == 64
    assert restored["facets"][0]["claims"][0]["evidence_ids"] == [original]
    assert restored["facets"][0]["conclusion"] == answer["facets"][0]["conclusion"]
    assert restored["facets"][0]["claims"][0]["text"] == answer["facets"][0]["claims"][0]["text"]
    assert answer["facets"][0]["claims"][0]["evidence_ids"] == [code]
