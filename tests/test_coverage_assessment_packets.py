"""Synthetic-only coverage packet preparation contract tests."""

import hashlib
import json
import uuid

import pytest

from scripts import coverage_assessment_packets as packets
from scripts.coverage_consumer_inputs import prepare_pair


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _json(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _fixture(snippet_chars=0, cards_per_task=6):
    source_stage = str(uuid.UUID(int=101))
    packet_stage = str(uuid.UUID(int=202))
    tasks = []
    cases = []
    source_manifest_rows = []
    capture_rows = []
    artifacts = {}
    captures_by_task = {}
    for task_index in range(1, 9):
        task_id = f"D-R{task_index:02d}"
        cards = []
        card_ids = []
        source_map = {}
        captures = {}
        facets = [
            {"facet_id": f"D-Q{task_index:02d}-F1", "definition": "Declared operating constraints."},
            {"facet_id": f"D-Q{task_index:02d}-F2", "definition": "Declared failure behavior."},
        ]
        cases.append(
            {
                "task_id": task_id,
                "critical_facets": [
                    {**facets[0], "critical_checks": ["State limits.", "State exclusions."]},
                    {**facets[1], "critical_checks": ["State failure mode.", "State recovery behavior."]},
                ],
            }
        )
        for card_index in range(1, cards_per_task + 1):
            card_id = f"c{task_index:02d}-{card_index:02d}"
            source_id = f"s{task_index:02d}-{card_index:02d}"
            card_ids.append(card_id)
            source_map[card_id] = source_id
            projection = {
                "id": card_id,
                "title": f"Synthetic result {card_index}",
                "url": f"https://example.test/{task_index}/{card_index}",
                "snippet": "Public synthetic evidence context." + ("x" * snippet_chars),
            }
            full_metadata = {"title": projection["title"], "url": projection["url"], "engine": "synthetic"}
            cards.append(
                {
                    "card_id": card_id,
                    "full_metadata": full_metadata,
                    "full_metadata_sha256": _sha(_json(full_metadata) + b"\n"),
                    "ranking_projection": projection,
                    "ranking_projection_sha256": _sha(_json(projection) + b"\n"),
                }
            )
            text = f"Task {task_index}, source {card_index}. " + ("Evidence. " * 90)
            context_sha = _sha(text.encode())
            artifact_name = f"context-{len(artifacts) + 1:04d}.json"
            artifacts[artifact_name] = _json(
                {
                    "schema": "coverage-source-context/1",
                    "task_id": task_id,
                    "source_id": source_id,
                    "source_url": f"https://example.test/{task_index}/{card_index}",
                    "result_index": card_index - 1,
                    "title": f"Synthetic result {card_index}",
                    "engine": "synthetic",
                    "context": text,
                    "context_sha256": context_sha,
                    "context_characters": len(text),
                }
            )
            captures[source_id] = {"state": "success", "context": text, "context_sha256": context_sha}
            source_manifest_rows.append({"task_id": task_id, "source_id": source_id})
            capture_rows.append(
                {
                    "task_id": task_id,
                    "source_id": source_id,
                    "status": "captured",
                    "context_artifact": artifact_name,
                    "url": f"https://example.test/{task_index}/{card_index}",
                    "result_index": card_index - 1,
                    "title": f"Synthetic result {card_index}",
                    "engine": "synthetic",
                    "context_sha256": context_sha,
                    "context_characters": len(text),
                }
            )
        tasks.append(
            {
                "task_id": task_id,
                "query": f"Synthetic task {task_index} question?",
                "purpose": "Test source-bounded evidence handling.",
                "facets": [
                    {"id": row["facet_id"], "description": row["definition"]} for row in cases[-1]["critical_facets"]
                ],
                "cards": cards,
                "native_order": card_ids,
                "source_id_by_card_id": source_map,
            }
        )
        captures_by_task[task_id] = (card_ids, source_map, captures)
    inventory = {
        "schema": "coverage-source-capture-inventory/1",
        "stage_uuid": source_stage,
        "status": "complete",
        "sources": capture_rows,
    }
    inventory_bytes = _json(inventory)
    capture_manifest = {
        "schema": "coverage-source-capture-manifest/1",
        "stage_uuid": source_stage,
        "sources": source_manifest_rows,
    }
    capture_manifest_bytes = _json(capture_manifest)
    bindings = []
    for task in tasks:
        for rank, card in enumerate(task["cards"], 1):
            bindings.append(
                {
                    "task_id": task["task_id"],
                    "card_id": card["card_id"],
                    "source_id": task["source_id_by_card_id"][card["card_id"]],
                    "native_rank": rank,
                    "full_metadata_sha256": card["full_metadata_sha256"],
                    "ranking_projection_sha256": card["ranking_projection_sha256"],
                }
            )
    pipeline = {
        "schema": "coverage-pipeline-inputs/1",
        "stage_uuid": source_stage,
        "capture_manifest": capture_manifest,
        "capture_manifest_bytes": capture_manifest_bytes,
        "capture_manifest_sha256": _sha(capture_manifest_bytes),
        "card_bindings": bindings,
        "card_bindings_sha256": _sha(_json(bindings) + b"\n"),
        "tasks": tasks,
    }
    prepared = packets.prepare_assessment_packets(
        packet_stage_uuid=packet_stage,
        pipeline_inputs=pipeline,
        research_cases=cases,
        capture_inventory_bytes=inventory_bytes,
        expected_capture_inventory_sha256=_sha(inventory_bytes),
        context_artifacts=artifacts,
    )
    return prepared, captures_by_task


def _answer_stage(prepared, captures_by_task):
    answer_inputs = {}
    answer_outputs = {}
    source_outputs = {}
    for task_index, (task_id, (card_ids, source_map, _)) in enumerate(captures_by_task.items(), 1):
        contexts = prepared.task_contexts[task_id]
        captures_sha = _sha(_json(contexts))
        orders = {"w0": card_ids, "candidate": list(reversed(card_ids))}
        pair = prepare_pair(
            task_index=task_index,
            task_id=task_id,
            task_question=f"Synthetic task {task_index} question?",
            purpose="Test source-bounded evidence handling.",
            critical_checks=list(prepared.task_checks[task_id]),
            card_ids=card_ids,
            source_id_by_card_id=source_map,
            captures=contexts,
            expected_capture_inventory_sha256=captures_sha,
            orders=orders,
        )
        consumer_catalog = json.loads(pair.private_catalog_bytes)
        assert set(consumer_catalog) == set(prepared.task_catalogs[task_id])
        for code, row in consumer_catalog.items():
            assert row == {
                key: prepared.task_catalogs[task_id][code][key]
                for key in (
                    "source_id",
                    "context_sha256",
                    "start",
                    "end",
                    "original_evidence_id",
                    "text",
                    "passage_sha256",
                )
            }
        answer_inputs[task_id] = {row.arm: row.body for row in pair.answers}
        answer_outputs[task_id] = {}
        for row in pair.answers:
            delivered = set(row.delivered_evidence_ids)
            code = next(iter(delivered))
            answer_outputs[task_id][row.arm] = {
                "facets": [
                    {
                        "facet_id": check["id"],
                        "conclusion": f"Synthetic conclusion for {check['id']}.",
                        "claims": [{"text": f"Synthetic claim for {check['id']}.", "evidence_ids": [code]}],
                    }
                    for check in prepared.task_checks[task_id]
                ]
            }
        source_outputs[task_id] = {}
        for assessor in ("R1", "R2"):
            rows = []
            for packet in prepared.preassessment_packets:
                if packet["task_id"] != task_id or packet["role"] != "source" or packet["assessor_id"] != assessor:
                    continue
                doc = json.loads(packet["bytes"])
                model_input = doc["model_input"]
                if not model_input["model_call_required"]:
                    continue
                for source in model_input["sources"]:
                    rows.append(
                        {
                            "source_id": source["source_id"],
                            "facets": [
                                {
                                    "facet_id": check["id"],
                                    "disposition": "evidence_available",
                                    "scope_match": "match",
                                    "entailment": "supports",
                                    "applicability": "applicable",
                                    "not_assessable_reason": "",
                                    "evidence_ids": [],
                                    "reason": "Synthetic typed fixture.",
                                    "limitations": [],
                                }
                                for check in prepared.task_checks[task_id]
                            ],
                        }
                    )
            # The validator accepts chunk envelopes. Each packet is a distinct assigned chunk.
            source_outputs[task_id][assessor] = []
            for packet in prepared.preassessment_packets:
                if packet["task_id"] != task_id or packet["role"] != "source" or packet["assessor_id"] != assessor:
                    continue
                doc = json.loads(packet["bytes"])
                model_input = doc["model_input"]
                if model_input["model_call_required"]:
                    source_outputs[task_id][assessor].append(
                        {
                            "task_id": task_id,
                            "assessor_id": assessor,
                            "chunk_id": model_input["chunk_id"],
                            "source_assessments": [
                                row
                                for row in rows
                                if row["source_id"] in {s["source_id"] for s in model_input["sources"]}
                            ],
                        }
                    )
    return answer_inputs, answer_outputs, source_outputs


def test_preassessment_packets_blind_complete_and_keep_ranking_facets_separate_from_checks():
    prepared, _ = _fixture()
    assert len(prepared.preassessment_packets) == 32
    assert len(json.loads(prepared.preassessment_manifest_bytes)["tasks"]) == 8
    for packet in prepared.preassessment_packets:
        doc = json.loads(packet["bytes"])
        assert packet["sha256"] == _sha(packet["bytes"])
        assert "candidate" not in packet["bytes"].decode() and "w0" not in packet["bytes"].decode()
        if packet["role"] == "card":
            assert len(doc["model_input"]["facets"]) == 2
            assert len(doc["model_input"]["cards"]) == 6
            assert all("card_id" not in row for row in doc["model_input"]["cards"])
        else:
            assert len(doc["model_input"]["facets"]) == 4
            assert len(doc["model_input"]["sources"]) == 6
            assert doc["model_input"]["chunk_id"]


def test_answer_packets_join_fixed_inputs_restore_citations_and_use_blind_arm_ids():
    prepared, captures = _fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    result = packets.prepare_answer_assessment_packets(
        prepared=prepared,
        answer_inputs=answer_inputs,
        answer_outputs=answer_outputs,
        source_outputs=source_outputs,
    )
    assert len(result.packets) == 32
    manifest = json.loads(result.manifest_bytes)
    assert manifest["model_calls_performed"] == 0 and manifest["admission_created"] is False
    for packet in result.packets:
        doc = json.loads(packet["bytes"])
        assert packet["sha256"] == _sha(packet["bytes"])
        assert doc["blind_arm_id"] not in {"w0", "candidate"}
        assert len(doc["task"]["critical_checks"]) == 4
        assert len(doc["answer"]) == 4
        assert all(
            code in {row["original_evidence_id"] for row in prepared.task_catalogs[packet["task_id"]].values()}
            for facet in doc["answer"]
            for claim in facet["claims"]
            for code in claim["evidence_ids"]
        )
        assert len(doc["same_assessor_source_assessment"]["tasks"][0]["source_assessments"]) == 6
        not_delivered = [row["source_id"] for row in doc["opening_outcomes"] if not row["delivered"]]
        for source_id in not_delivered:
            context = prepared.task_contexts[packet["task_id"]][source_id].get("context")
            if context:
                assert context.encode() not in packet["bytes"]


def test_answer_packet_rejects_incomplete_checklist_and_unassigned_citations():
    prepared, captures = _fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    first_task = next(iter(answer_outputs))
    answer_outputs[first_task]["w0"]["facets"].pop()
    with pytest.raises(packets.PacketPreparationError, match="answer-output-check-coverage"):
        packets.prepare_answer_assessment_packets(
            prepared=prepared,
            answer_inputs=answer_inputs,
            answer_outputs=answer_outputs,
            source_outputs=source_outputs,
        )


def test_source_outputs_must_cover_all_four_check_ids():
    prepared, captures = _fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    first_task = next(iter(source_outputs))
    source_outputs[first_task]["R1"][0]["source_assessments"][0]["facets"].pop()
    with pytest.raises(packets.PacketPreparationError, match="source-output-critical-check-coverage"):
        packets.prepare_answer_assessment_packets(
            prepared=prepared,
            answer_inputs=answer_inputs,
            answer_outputs=answer_outputs,
            source_outputs=source_outputs,
        )


def test_answer_input_schema_drift_is_rejected():
    prepared, captures = _fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    first_task = next(iter(answer_inputs))
    arm = "w0"
    body = json.loads(answer_inputs[first_task][arm])
    body["output_schema"] = {}
    answer_inputs[first_task][arm] = _json(body)
    with pytest.raises(packets.PacketPreparationError, match="answer-input-output-schema"):
        packets.prepare_answer_assessment_packets(
            prepared=prepared,
            answer_inputs=answer_inputs,
            answer_outputs=answer_outputs,
            source_outputs=source_outputs,
        )


def test_input_cap_is_a_hard_failure_without_packet_trimming():
    with pytest.raises(packets.PacketPreparationError, match="model-input-byte-cap"):
        _fixture(snippet_chars=70_000)


def test_source_chunks_are_bounded_to_four_complete_twenty_source_chunks():
    prepared, _ = _fixture(cards_per_task=80)
    for task_id in prepared.task_metadata:
        for assessor in ("R1", "R2"):
            rows = [
                json.loads(packet["bytes"])["model_input"]
                for packet in prepared.preassessment_packets
                if packet["task_id"] == task_id and packet["role"] == "source" and packet["assessor_id"] == assessor
            ]
            assert len(rows) == 4
            assert [len(row["source_inventory"]) for row in rows] == [20, 20, 20, 20]
            assert (
                len({source for row in rows for source in (item["source_id"] for item in row["source_inventory"])})
                == 80
            )


def test_answer_opening_state_cannot_be_rewritten_or_rescued():
    prepared, captures = _fixture()
    answer_inputs, answer_outputs, source_outputs = _answer_stage(prepared, captures)
    task_id = next(iter(answer_inputs))
    body = json.loads(answer_inputs[task_id]["w0"])
    body["opening_outcomes"][0]["state"] = "not_acquired"
    answer_inputs[task_id]["w0"] = _json(body)
    with pytest.raises(packets.PacketPreparationError, match="answer-input-opening-outcome-drift"):
        packets.prepare_answer_assessment_packets(
            prepared=prepared,
            answer_inputs=answer_inputs,
            answer_outputs=answer_outputs,
            source_outputs=source_outputs,
        )
