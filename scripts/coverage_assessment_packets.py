"""Fresh, blinded assessment-packet preparation for the coverage study.

This module prepares bytes and bindings only. It does not dispatch models,
parse assessor responses, repair answers, or make semantic judgments. Frozen
EXP-077 schemas and rubric text are read as public reference source files; no
prior EXP-077/EXP-100 results or grades are imported.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import secrets
import uuid
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Mapping

from scripts.coverage_consumer_inputs import task_capture_contexts

MAX_PACKET_BYTES = 384_000
MAX_TASKS = 8
MAX_CARDS_PER_TASK = 80
MAX_SOURCES_PER_TASK = 80
MAX_SOURCE_CHUNK = 20
MAX_SOURCE_CHUNKS_PER_TASK = 4
MAX_GRADER_SUBMISSIONS = 112
MAX_COMBINED_ADMISSIONS = 130
PASSAGE_CHARS = 600
MAX_CONTEXT_CHARS = 8_000
MAX_ANSWER_WORDS = 800
_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:-]{0,127}\Z")
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_CODE = re.compile(r"D[1-8]E[0-9]{4}\Z")
_REFERENCE = Path(__file__).resolve().parents[1] / "docs/experiments/evidence/EXP-077"


class PacketPreparationError(ValueError):
    """A packet, inventory, or binding falls outside the preparation contract."""


@dataclass(frozen=True)
class PreparedAssessmentPackets:
    packet_stage_uuid: str
    source_stage_uuid: str
    preassessment_packets: tuple[dict[str, object], ...]
    preassessment_manifest_bytes: bytes
    private_binding_bytes: bytes
    task_contexts: Mapping[str, Mapping[str, Mapping[str, object]]]
    task_catalogs: Mapping[str, Mapping[str, Mapping[str, object]]]
    task_metadata: Mapping[str, Mapping[str, object]]
    task_checks: Mapping[str, tuple[dict[str, str], ...]]
    card_identity_maps: Mapping[str, Mapping[str, str]]
    blind_arm_labels: Mapping[str, Mapping[str, str]]
    reference_sha256: Mapping[str, str]
    private_binding_sha256: str


@dataclass(frozen=True)
class PreparedAnswerPackets:
    packets: tuple[dict[str, object], ...]
    manifest_bytes: bytes
    manifest_sha256: str


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise PacketPreparationError("canonical-json-invalid") from exc


def _parse(raw: bytes, label: str) -> object:
    if type(raw) is not bytes or not raw or len(raw) > MAX_PACKET_BYTES:
        raise PacketPreparationError(f"{label}-bytes-invalid")

    def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in items:
            if key in result:
                raise PacketPreparationError(f"{label}-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda _value: (_ for _ in ()).throw(PacketPreparationError(f"{label}-nonfinite")),
        )
    except PacketPreparationError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PacketPreparationError(f"{label}-json-invalid") from exc


def _require_uuid(value: object, label: str) -> str:
    if type(value) is not str:
        raise PacketPreparationError(f"{label}-uuid-invalid")
    try:
        if str(uuid.UUID(value)) != value:
            raise ValueError
    except ValueError as exc:
        raise PacketPreparationError(f"{label}-uuid-invalid") from exc
    return value


def _require_id(value: object, label: str) -> str:
    if type(value) is not str or not _ID.fullmatch(value):
        raise PacketPreparationError(f"{label}-invalid")
    return value


def _digest_id(*parts: str) -> str:
    return _sha("\0".join(parts).encode("utf-8"))


@lru_cache(maxsize=None)
def _reference(name: str) -> tuple[object, bytes]:
    if name not in {
        "card-assessment.schema.json",
        "source-chunk.schema.json",
        "answerer.schema.json",
        "answer-assessment.schema.json",
        "card-assessor-prompt.txt",
        "source-assessor-prompt.txt",
        "answer-assessor-prompt.txt",
    }:
        raise PacketPreparationError("reference-name-invalid")
    path = _REFERENCE / name
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise PacketPreparationError("frozen-reference-unavailable") from exc
    if name.endswith(".json"):
        obj = _parse(raw, "frozen-schema")
        if type(obj) is not dict:
            raise PacketPreparationError("frozen-schema-shape")
        return obj, raw
    try:
        return raw.decode("utf-8"), raw
    except UnicodeDecodeError as exc:
        raise PacketPreparationError("frozen-prompt-encoding") from exc


def _packet(
    *,
    packet_id: str,
    task_id: str,
    assessor_id: str,
    role: str,
    model_input: dict[str, object],
    schema: dict,
    prompt: str,
) -> dict[str, object]:
    payload = {
        "schema": "coverage-assessment-model-packet/1",
        "packet_id": packet_id,
        "task_id": task_id,
        "assessor_id": assessor_id,
        "role": role,
        "prompt": prompt,
        "model_input": model_input,
        "output_schema": schema,
    }
    raw = _canonical(payload)
    if len(raw) > MAX_PACKET_BYTES:
        raise PacketPreparationError("model-input-byte-cap")
    return {
        "packet_id": packet_id,
        "task_id": task_id,
        "assessor_id": assessor_id,
        "role": role,
        "bytes": raw,
        "sha256": _sha(raw),
        "byte_count": len(raw),
    }


def _facet_and_check_contract(case: dict, task: dict) -> tuple[list[dict[str, str]], tuple[dict[str, str], ...]]:
    raw_facets = case.get("critical_facets")
    if type(raw_facets) is not list or len(raw_facets) != 2:
        raise PacketPreparationError("caller-facet-count")
    facets: list[dict[str, str]] = []
    checks: list[dict[str, str]] = []
    seen_facets: set[str] = set()
    for facet in raw_facets:
        if type(facet) is not dict:
            raise PacketPreparationError("caller-facet-shape")
        facet_id = _require_id(facet.get("facet_id"), "caller-facet-id")
        definition = facet.get("definition")
        if type(definition) is not str or not definition.strip():
            raise PacketPreparationError("caller-facet-definition")
        if facet_id in seen_facets:
            raise PacketPreparationError("caller-facet-duplicate")
        seen_facets.add(facet_id)
        facets.append({"id": facet_id, "description": definition})
        raw_checks = facet.get("critical_checks")
        if type(raw_checks) is not list or len(raw_checks) != 2:
            raise PacketPreparationError("facet-critical-check-count")
        for index, text in enumerate(raw_checks, start=1):
            if type(text) is not str or not text.strip():
                raise PacketPreparationError("critical-check-description")
            checks.append({"id": _require_id(f"{facet_id}-C{index}", "critical-check-id"), "description": text})
    if task.get("facets") != facets:
        raise PacketPreparationError("ranking-facet-binding-mismatch")
    if len({row["id"] for row in checks}) != 4:
        raise PacketPreparationError("critical-check-identity-collision")
    return facets, tuple(checks)


def _task_catalog(task_index: int, contexts: Mapping[str, Mapping[str, object]]) -> dict[str, dict[str, object]]:
    catalog: dict[str, dict[str, object]] = {}
    sequence = 1
    for source_id in sorted(contexts):
        capture = contexts[source_id]
        if capture.get("state") != "success":
            continue
        text = capture.get("context")
        if type(text) is not str or len(text) > MAX_CONTEXT_CHARS:
            raise PacketPreparationError("captured-context-bound")
        context_sha = _sha(text.encode("utf-8"))
        if context_sha != capture.get("context_sha256"):
            raise PacketPreparationError("captured-context-digest")
        for start in range(0, len(text), PASSAGE_CHARS):
            passage = text[start : start + PASSAGE_CHARS]
            code = f"D{task_index}E{sequence:04d}"
            sequence += 1
            original_id = _sha(f"{source_id}\0{context_sha}\0{start}\0{start + len(passage)}".encode("utf-8"))
            catalog[code] = {
                "code": code,
                "source_id": source_id,
                "context_sha256": context_sha,
                "start": start,
                "end": start + len(passage),
                "original_evidence_id": original_id,
                "text": passage,
                "passage_sha256": _sha(passage.encode("utf-8")),
            }
    if sequence > 10_000:
        raise PacketPreparationError("citation-code-capacity")
    return catalog


def _codes_for_source(catalog: Mapping[str, Mapping[str, object]], source_id: str) -> list[str]:
    return [code for code, row in catalog.items() if row.get("source_id") == source_id]


def _closed_schema(reference: str) -> dict:
    obj, _raw = _reference(reference)
    return copy.deepcopy(obj)


def _shuffle_key(stage_uuid: str, task_id: str, role: str, identity: str) -> str:
    return _digest_id(stage_uuid, task_id, role, identity)


def prepare_assessment_packets(
    *,
    packet_stage_uuid: str,
    pipeline_inputs: dict,
    research_cases: list[dict],
    capture_inventory_bytes: bytes,
    expected_capture_inventory_sha256: str,
    context_artifacts: Mapping[str, bytes],
) -> PreparedAssessmentPackets:
    """Freeze blind card/source assignments and the pre-answer arm-label map.

    The returned packet bytes are inputs only. Caller-provided packet stage IDs
    must be fresh; no gate or study admission is inferred from successful
    preparation.
    """
    packet_stage_uuid = _require_uuid(packet_stage_uuid, "packet-stage")
    if type(pipeline_inputs) is not dict or pipeline_inputs.get("schema") != "coverage-pipeline-inputs/1":
        raise PacketPreparationError("pipeline-inputs-schema")
    source_stage_uuid = _require_uuid(pipeline_inputs.get("stage_uuid"), "source-stage")
    if packet_stage_uuid == source_stage_uuid:
        raise PacketPreparationError("packet-stage-must-be-fresh")
    tasks = pipeline_inputs.get("tasks")
    if (
        type(tasks) is not list
        or len(tasks) != MAX_TASKS
        or type(research_cases) is not list
        or len(research_cases) != MAX_TASKS
    ):
        raise PacketPreparationError("task-count")
    case_by_task = {}
    for case in research_cases:
        if type(case) is not dict:
            raise PacketPreparationError("research-case-shape")
        task_id = _require_id(case.get("task_id"), "research-case-task-id")
        if task_id in case_by_task:
            raise PacketPreparationError("research-case-duplicate")
        case_by_task[task_id] = case
    if type(pipeline_inputs.get("capture_manifest")) is not dict:
        raise PacketPreparationError("pipeline-capture-manifest")
    capture_manifest_bytes = pipeline_inputs.get("capture_manifest_bytes")
    if type(capture_manifest_bytes) is not bytes or _sha(capture_manifest_bytes) != pipeline_inputs.get(
        "capture_manifest_sha256"
    ):
        raise PacketPreparationError("pipeline-capture-manifest-pin")
    if _parse(capture_manifest_bytes, "pipeline-capture-manifest") != pipeline_inputs["capture_manifest"]:
        raise PacketPreparationError("pipeline-capture-manifest-bytes-mismatch")
    if type(pipeline_inputs.get("card_bindings")) is not list:
        raise PacketPreparationError("pipeline-card-bindings")
    # The upstream pipeline's sealed JSON artifacts are newline-terminated;
    # model packets use a separate, non-newline canonical encoding.
    if _sha(_canonical(pipeline_inputs["card_bindings"]) + b"\n") != pipeline_inputs.get("card_bindings_sha256"):
        raise PacketPreparationError("pipeline-card-bindings-pin")
    if pipeline_inputs["capture_manifest"].get("stage_uuid") != source_stage_uuid:
        raise PacketPreparationError("pipeline-source-stage-mismatch")
    source_rows = pipeline_inputs["capture_manifest"].get("sources")
    if type(source_rows) is not list or any(type(row) is not dict for row in source_rows):
        raise PacketPreparationError("pipeline-source-inventory")
    source_manifest_by_task: dict[str, set[str]] = {}
    for row in source_rows:
        task_value = _require_id(row.get("task_id"), "pipeline-source-task-id")
        source_value = _require_id(row.get("source_id"), "pipeline-source-id")
        if source_value in source_manifest_by_task.setdefault(task_value, set()):
            raise PacketPreparationError("pipeline-source-duplicate")
        source_manifest_by_task[task_value].add(source_value)
    if type(pipeline_inputs.get("card_bindings")) is not list:
        raise PacketPreparationError("pipeline-card-bindings")
    bindings_by_card: dict[tuple[str, str], dict[str, object]] = {}
    for binding in pipeline_inputs["card_bindings"]:
        if type(binding) is not dict:
            raise PacketPreparationError("pipeline-card-binding-row")
        key = (
            _require_id(binding.get("task_id"), "binding-task-id"),
            _require_id(binding.get("card_id"), "binding-card-id"),
        )
        if key in bindings_by_card:
            raise PacketPreparationError("pipeline-card-binding-duplicate")
        bindings_by_card[key] = binding
    capture_inventory = _parse(capture_inventory_bytes, "capture-inventory")
    if _sha(capture_inventory_bytes) != expected_capture_inventory_sha256:
        raise PacketPreparationError("capture-inventory-pin")
    if type(capture_inventory) is not dict or capture_inventory.get("stage_uuid") != source_stage_uuid:
        raise PacketPreparationError("capture-inventory-stage")

    packets: list[dict[str, object]] = []
    task_contexts: dict[str, dict[str, dict[str, object]]] = {}
    task_catalogs: dict[str, dict[str, dict[str, object]]] = {}
    task_metadata: dict[str, dict[str, object]] = {}
    task_checks: dict[str, tuple[dict[str, str], ...]] = {}
    card_identity_maps: dict[str, dict[str, str]] = {}
    arm_labels: dict[str, dict[str, str]] = {}
    task_manifest_rows = []
    observed_binding_keys: set[tuple[str, str]] = set()

    card_schema_reference, card_schema_raw = _reference("card-assessment.schema.json")
    source_schema_reference, source_schema_raw = _reference("source-chunk.schema.json")
    _answer_schema_reference, answer_schema_raw = _reference("answer-assessment.schema.json")
    card_prompt, card_prompt_raw = _reference("card-assessor-prompt.txt")
    source_prompt, source_prompt_raw = _reference("source-assessor-prompt.txt")
    _answer_prompt_reference, answer_prompt_raw = _reference("answer-assessor-prompt.txt")
    schemas_hashes = {
        name: _sha(raw)
        for name, raw in (
            ("card-assessment.schema.json", card_schema_raw),
            ("source-chunk.schema.json", source_schema_raw),
            ("answer-assessment.schema.json", answer_schema_raw),
            ("card-assessor-prompt.txt", card_prompt_raw),
            ("source-assessor-prompt.txt", source_prompt_raw),
            ("answer-assessor-prompt.txt", answer_prompt_raw),
        )
    }

    for task_index, task in enumerate(tasks, start=1):
        if type(task) is not dict:
            raise PacketPreparationError("task-row-shape")
        task_id = _require_id(task.get("task_id"), "task-id")
        case = case_by_task.get(task_id)
        if case is None:
            raise PacketPreparationError("task-case-mismatch")
        facets, checks = _facet_and_check_contract(case, task)
        if task_id in task_metadata:
            raise PacketPreparationError("task-duplicate")
        cards = task.get("cards")
        card_ids = task.get("native_order")
        source_map = task.get("source_id_by_card_id")
        if type(card_ids) is list:
            for card_id_value in card_ids:
                _require_id(card_id_value, "card-order-id")
        if type(source_map) is dict:
            for card_key, source_value in source_map.items():
                _require_id(card_key, "source-map-card-id")
                _require_id(source_value, "source-map-source-id")
        if (
            type(cards) is not list
            or not 1 <= len(cards) <= MAX_CARDS_PER_TASK
            or type(card_ids) is not list
            or len(set(card_ids)) != len(card_ids)
            or len(card_ids) != len(cards)
            or set(card_ids) != {row.get("card_id") for row in cards if type(row) is dict}
            or type(source_map) is not dict
            or set(source_map) != set(card_ids)
        ):
            raise PacketPreparationError("complete-card-inventory-required")
        if source_manifest_by_task.get(task_id) != set(source_map.values()):
            raise PacketPreparationError("task-source-manifest-coverage")
        projections: dict[str, dict[str, str]] = {}
        card_hashes: dict[str, str] = {}
        card_map: dict[str, str] = {}
        for card in cards:
            if type(card) is not dict:
                raise PacketPreparationError("card-row-shape")
            card_id = _require_id(card.get("card_id"), "card-id")
            projection = card.get("ranking_projection")
            full_metadata = card.get("full_metadata")
            if (
                type(projection) is not dict
                or set(projection) != {"id", "title", "url", "snippet"}
                or projection.get("id") != card_id
                or any(type(projection.get(key)) is not str for key in ("title", "url", "snippet"))
                or type(full_metadata) is not dict
                or _sha(_canonical(full_metadata) + b"\n") != card.get("full_metadata_sha256")
                or _sha(_canonical(projection) + b"\n") != card.get("ranking_projection_sha256")
            ):
                raise PacketPreparationError("ranking-projection-shape")
            binding = bindings_by_card.get((task_id, card_id))
            if (
                binding is None
                or binding.get("source_id") != source_map.get(card_id)
                or type(binding.get("native_rank")) is not int
                or binding.get("native_rank") != card_ids.index(card_id) + 1
                or binding.get("full_metadata_sha256") != card.get("full_metadata_sha256")
                or binding.get("ranking_projection_sha256") != card.get("ranking_projection_sha256")
            ):
                raise PacketPreparationError("card-binding-provenance")
            observed_binding_keys.add((task_id, card_id))
            digest = _sha(
                _canonical(
                    {"stage_uuid": packet_stage_uuid, "task_id": task_id, "card_id": card_id, "card": projection}
                )
            )
            if digest in card_map:
                raise PacketPreparationError("card-fingerprint-duplicate")
            projections[card_id] = {key: projection[key] for key in ("title", "url", "snippet")}
            card_hashes[card_id] = digest
            card_map[digest] = card_id
        if set(source_map.values()) - {row.get("source_id") for row in source_rows}:
            raise PacketPreparationError("card-source-not-in-capture-manifest")
        unique_source_ids = list(dict.fromkeys(source_map[card_id] for card_id in card_ids))
        contexts = task_capture_contexts(
            task_id=task_id,
            stage_uuid=source_stage_uuid,
            source_ids=unique_source_ids,
            capture_inventory_bytes=capture_inventory_bytes,
            expected_capture_inventory_sha256=expected_capture_inventory_sha256,
            context_artifacts=context_artifacts,
        )
        catalog = _task_catalog(task_index, contexts)
        task_contexts[task_id] = contexts
        task_catalogs[task_id] = catalog
        task_checks[task_id] = checks
        card_identity_maps[task_id] = card_map
        task_metadata[task_id] = {
            "task_id": task_id,
            "query": task.get("query"),
            "purpose": task.get("purpose"),
            "facets": facets,
            "card_ids": list(card_ids),
            "source_ids": unique_source_ids,
            "source_id_by_card_id": dict(source_map),
            "source_inventory": [
                {
                    "source_id": source_id,
                    "state": contexts[source_id]["state"],
                    "context_sha256": contexts[source_id].get("context_sha256"),
                    "context_characters": len(contexts[source_id]["context"])
                    if type(contexts[source_id].get("context")) is str
                    else None,
                }
                for source_id in unique_source_ids
            ],
        }
        arm_labels[task_id] = {arm: secrets.token_hex(16) for arm in ("w0", "candidate")}
        if len(set(arm_labels[task_id].values())) != 2:
            raise PacketPreparationError("blind-arm-label-collision")

        # A/B receive the same complete card set under opaque fingerprints,
        # independently hash-shuffled so native position is not exposed.
        for assessor in ("A", "B"):
            ordered_card_ids = sorted(
                card_ids, key=lambda card_id: _shuffle_key(packet_stage_uuid, task_id, assessor, card_id)
            )
            card_rows = [{"card_sha256": card_hashes[card_id], **projections[card_id]} for card_id in ordered_card_ids]
            schema = copy.deepcopy(card_schema_reference)
            records = schema["properties"]["records"]
            records["minItems"] = len(card_rows)
            records["maxItems"] = len(card_rows)
            record_props = records["items"]["properties"]
            record_props["card_sha256"] = {"enum": [card_hashes[card_id] for card_id in ordered_card_ids]}
            for key in ("facets", "uncertain"):
                record_props[key]["items"] = {"enum": [row["id"] for row in facets]}
            record_props["facet_anchors"]["propertyNames"] = {"enum": [row["id"] for row in facets]}
            packet_id = _digest_id(packet_stage_uuid, task_id, "card", assessor)
            model_input = {
                "task_id": task_id,
                "task": {"query": task.get("query"), "purpose": task.get("purpose")},
                "cards": card_rows,
                "facets": facets,
            }
            packets.append(
                _packet(
                    packet_id=packet_id,
                    task_id=task_id,
                    assessor_id=assessor,
                    role="card",
                    model_input=model_input,
                    schema=schema,
                    prompt=str(card_prompt),
                )
            )

        source_rows_for_task = [{"source_id": source_id, **contexts[source_id]} for source_id in unique_source_ids]
        source_rows_for_task.sort(
            key=lambda row: _shuffle_key(packet_stage_uuid, task_id, "source-inventory", str(row["source_id"]))
        )
        for assessor in ("R1", "R2"):
            shuffled = sorted(
                source_rows_for_task,
                key=lambda row: _shuffle_key(packet_stage_uuid, task_id, assessor, str(row["source_id"])),
            )
            chunk_count = (len(shuffled) + MAX_SOURCE_CHUNK - 1) // MAX_SOURCE_CHUNK
            if chunk_count > MAX_SOURCE_CHUNKS_PER_TASK:
                raise PacketPreparationError("source-chunk-count")
            for chunk_index in range(chunk_count):
                rows = shuffled[chunk_index * MAX_SOURCE_CHUNK : (chunk_index + 1) * MAX_SOURCE_CHUNK]
                chunk_id = f"{packet_stage_uuid}:{task_id}:{assessor}:{chunk_index + 1:02d}"
                packet_id = _digest_id(packet_stage_uuid, task_id, "source", assessor, str(chunk_index + 1))
                source_inventory = []
                assessable = []
                for row in rows:
                    item = {"source_id": row["source_id"], "state": row["state"]}
                    if row.get("state") == "success":
                        text = row.get("context")
                        if type(text) is not str:
                            raise PacketPreparationError("successful-source-context-missing")
                        codes = _codes_for_source(catalog, str(row["source_id"]))
                        item["context"] = text
                        item["evidence_ids"] = codes
                        assessable.append(
                            {
                                "source_id": row["source_id"],
                                "context": text,
                                "evidence_ids": codes,
                                "passages": [{"evidence_id": code, "text": catalog[code]["text"]} for code in codes],
                            }
                        )
                    source_inventory.append(item)
                schema = copy.deepcopy(source_schema_reference)
                schema["properties"]["task_id"] = {"const": task_id}
                schema["properties"]["assessor_id"] = {"const": assessor}
                schema["properties"]["chunk_id"] = {"const": chunk_id}
                assessment_props = schema["properties"]["source_assessments"]
                assessment_props["minItems"] = len(assessable)
                assessment_props["maxItems"] = len(assessable)
                assessment_props["items"]["properties"]["source_id"] = {
                    "enum": [row["source_id"] for row in assessable]
                }
                facets_schema = assessment_props["items"]["properties"]["facets"]
                facets_schema["minItems"] = len(checks)
                facets_schema["maxItems"] = len(checks)
                facets_schema["items"]["properties"]["facet_id"] = {"enum": [row["id"] for row in checks]}
                model_input = {
                    "task_id": task_id,
                    "assessor_id": assessor,
                    "chunk_id": chunk_id,
                    "facets": list(checks),
                    "source_inventory": source_inventory,
                    "sources": assessable,
                    "model_call_required": bool(assessable),
                }
                packets.append(
                    _packet(
                        packet_id=packet_id,
                        task_id=task_id,
                        assessor_id=assessor,
                        role="source",
                        model_input=model_input,
                        schema=schema,
                        prompt=str(source_prompt),
                    )
                )
        task_manifest_rows.append(
            {
                "task_id": task_id,
                "task_index": task_index,
                "card_count": len(cards),
                "source_count": len(unique_source_ids),
                "catalog_count": len(catalog),
                "facets": facets,
                "critical_checks": list(checks),
                "card_fingerprints": {card_id: card_hashes[card_id] for card_id in card_ids},
                "source_inventory": task_metadata[task_id]["source_inventory"],
                "catalog_sha256": _sha(_canonical(catalog)),
            }
        )

    if (
        len(arm_labels) != MAX_TASKS
        or set(source_manifest_by_task) != set(task_metadata)
        or observed_binding_keys != set(bindings_by_card)
        or len(packets) > MAX_GRADER_SUBMISSIONS - 32
    ):
        raise PacketPreparationError("blind-map-task-coverage")
    packet_rows = [
        {key: packet[key] for key in ("packet_id", "task_id", "assessor_id", "role", "sha256", "byte_count")}
        for packet in packets
    ]
    manifest = {
        "schema": "coverage-assessment-preparation-manifest/1",
        "packet_stage_uuid": packet_stage_uuid,
        "source_stage_uuid": source_stage_uuid,
        "capture_inventory_sha256": expected_capture_inventory_sha256,
        "capture_manifest_sha256": pipeline_inputs.get("capture_manifest_sha256"),
        "card_inventory_sha256": pipeline_inputs.get("card_bindings_sha256"),
        "reference_sha256": schemas_hashes,
        "tasks": task_manifest_rows,
        "packets": packet_rows,
        "grader_packet_count": len(packets),
        "grader_submission_ceiling": MAX_GRADER_SUBMISSIONS,
        "combined_admission_ceiling": MAX_COMBINED_ADMISSIONS,
        "model_calls_performed": 0,
        "admission_created": False,
        "quality_credit": False,
    }
    manifest_bytes = _canonical(manifest)
    private_binding = {
        "schema": "coverage-assessment-private-bindings/1",
        "packet_stage_uuid": packet_stage_uuid,
        "source_stage_uuid": source_stage_uuid,
        "blind_arm_labels": arm_labels,
        "task_card_identity_maps": card_identity_maps,
        "task_catalogs": task_catalogs,
        "manifest_sha256": _sha(manifest_bytes),
        "arm_label_map_sha256": _sha(_canonical(arm_labels)),
    }
    binding_bytes = _canonical(private_binding)
    return PreparedAssessmentPackets(
        packet_stage_uuid=packet_stage_uuid,
        source_stage_uuid=source_stage_uuid,
        preassessment_packets=tuple(packets),
        preassessment_manifest_bytes=manifest_bytes,
        private_binding_bytes=binding_bytes,
        task_contexts=task_contexts,
        task_catalogs=task_catalogs,
        task_metadata=task_metadata,
        task_checks=task_checks,
        card_identity_maps=card_identity_maps,
        blind_arm_labels=arm_labels,
        reference_sha256=schemas_hashes,
        private_binding_sha256=_sha(binding_bytes),
    )


def _answer_schema(
    task_id: str, packet_id: str, assessor_id: str, blind_arm_id: str, check_ids: list[str], source_ids: list[str]
) -> dict:
    schema = _closed_schema("answer-assessment.schema.json")
    schema["properties"]["packet_id"] = {"const": packet_id}
    schema["properties"]["assessor_id"] = {"const": assessor_id}
    tasks = schema["properties"]["tasks"]
    tasks["minItems"] = 1
    tasks["maxItems"] = 1
    task_schema = schema["$defs"]["answerTask"]
    task_schema["properties"]["task_id"] = {"const": task_id}
    arms = task_schema["properties"]["arms"]
    arms["minItems"] = 1
    arms["maxItems"] = 1
    arm = schema["$defs"]["answerArm"]
    arm["properties"]["arm_id"] = {"const": blind_arm_id}
    assessments = arm["properties"]["facet_assessments"]
    assessments["minItems"] = len(check_ids)
    assessments["maxItems"] = len(check_ids)
    schema["$defs"]["answerFacet"]["properties"]["facet_id"] = {"enum": check_ids}
    schema["$defs"]["answerEvidenceDisposition"]["properties"]["source_id"] = {"enum": source_ids}
    evidence = schema["$defs"]["answerFacet"]["properties"]["evidence_dispositions"]
    evidence["minItems"] = len(source_ids)
    evidence["maxItems"] = len(source_ids)
    return schema


def _validate_answer_input(
    raw: bytes,
    *,
    task_id: str,
    checks: tuple[dict[str, str], ...],
    card_to_source: Mapping[str, str],
    contexts: Mapping[str, Mapping[str, object]],
    catalog: Mapping[str, Mapping[str, object]],
    expected_query: object,
    expected_purpose: object,
) -> dict[str, object]:
    value = _parse(raw, "answer-input")
    if type(value) is not dict or set(value) != {
        "task",
        "cards",
        "opening_outcomes",
        "delivered_sources",
        "output_schema",
    }:
        raise PacketPreparationError("answer-input-envelope")
    task = value["task"]
    if (
        type(task) is not dict
        or task.get("task_id") != task_id
        or task.get("query") != expected_query
        or task.get("purpose") != expected_purpose
        or task.get("critical_checks") != list(checks)
    ):
        raise PacketPreparationError("answer-input-task-binding")
    cards = value["cards"]
    outcomes = value["opening_outcomes"]
    delivered = value["delivered_sources"]
    if (
        type(cards) is not list
        or not 1 <= len(cards) <= 10
        or len(cards) != min(10, len(card_to_source))
        or any(type(card) is not str or card not in card_to_source for card in cards)
        or len(set(cards)) != len(cards)
        or type(outcomes) is not list
        or len(outcomes) != len(cards)
        or [row.get("card_id") if type(row) is dict else None for row in outcomes] != cards
        or type(delivered) is not list
    ):
        raise PacketPreparationError("answer-input-opening-binding")
    expected_delivered = []
    seen_sources: set[str] = set()
    delivered_count = 0
    for row in outcomes:
        if type(row) is not dict or set(row) != {"card_id", "state", "delivered"} or type(row["delivered"]) is not bool:
            raise PacketPreparationError("answer-input-outcome-shape")
        source_id = card_to_source[row["card_id"]]
        expected_state = "duplicate" if source_id in seen_sources else str(contexts[source_id]["state"])
        expected_flag = expected_state == "success" and delivered_count < 5
        if row["state"] != expected_state or row["delivered"] is not expected_flag:
            raise PacketPreparationError("answer-input-opening-outcome-drift")
        if expected_flag:
            expected_delivered.append(source_id)
            delivered_count += 1
        seen_sources.add(source_id)
    if len(expected_delivered) > 5 or len(delivered) != len(expected_delivered):
        raise PacketPreparationError("answer-input-delivery-count")
    delivered_contexts: list[dict[str, object]] = []
    for row, expected_source_id in zip(delivered, expected_delivered):
        if type(row) is not dict or set(row) != {"source_id", "passages"} or row.get("source_id") != expected_source_id:
            raise PacketPreparationError("answer-input-delivered-source-order")
        passages = row.get("passages")
        expected_codes = _codes_for_source(catalog, expected_source_id)
        if (
            type(passages) is not list
            or [p.get("evidence_id") if type(p) is dict else None for p in passages] != expected_codes
        ):
            raise PacketPreparationError("answer-input-delivered-code-coverage")
        for passage, code in zip(passages, expected_codes):
            if set(passage) != {"evidence_id", "text"} or passage["text"] != catalog[code]["text"]:
                raise PacketPreparationError("answer-input-passage-binding")
        capture = contexts[expected_source_id]
        expected_text = capture.get("context")
        reconstructed = "".join(str(passage["text"]) for passage in passages)
        if reconstructed != expected_text:
            raise PacketPreparationError("answer-input-context-binding")
        delivered_contexts.append(
            {
                "source_id": expected_source_id,
                "context": reconstructed,
                "context_sha256": capture.get("context_sha256"),
                "evidence_ids": expected_codes,
                "passages": [dict(passage) for passage in passages],
            }
        )
    expected_schema = {
        "type": "object",
        "additionalProperties": False,
        "required": ["facets"],
        "properties": {
            "facets": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["facet_id", "conclusion", "claims"],
                    "properties": {
                        "facet_id": {"enum": [row["id"] for row in checks]},
                        "conclusion": {"type": "string"},
                        "claims": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": False,
                                "required": ["text", "evidence_ids"],
                                "properties": {
                                    "text": {"type": "string"},
                                    "evidence_ids": {
                                        "type": "array",
                                        "items": {"type": "string"},
                                        "uniqueItems": True,
                                    },
                                },
                            },
                        },
                    },
                },
            }
        },
    }
    if value["output_schema"] != expected_schema:
        raise PacketPreparationError("answer-input-output-schema")
    return {
        "task": task,
        "cards": list(cards),
        "opening_outcomes": copy.deepcopy(outcomes),
        "delivered_contexts": delivered_contexts,
        "delivered_source_ids": expected_delivered,
    }


def _freeze_answer(
    answer: object,
    checks: tuple[dict[str, str], ...],
    delivered_codes: set[str],
    catalog: Mapping[str, Mapping[str, object]],
) -> tuple[list[dict[str, object]], int]:
    if type(answer) is not dict or set(answer) != {"facets"} or type(answer["facets"]) is not list:
        raise PacketPreparationError("answer-output-shape")
    facets = answer["facets"]
    check_ids = [row["id"] for row in checks]
    if [row.get("facet_id") if type(row) is dict else None for row in facets] != check_ids:
        raise PacketPreparationError("answer-output-check-coverage")
    total_words = 0
    frozen = []
    for facet in facets:
        if type(facet) is not dict or set(facet) != {"facet_id", "conclusion", "claims"}:
            raise PacketPreparationError("answer-output-facet-shape")
        if type(facet["conclusion"]) is not str or not facet["conclusion"].strip() or type(facet["claims"]) is not list:
            raise PacketPreparationError("answer-output-facet-fields")
        total_words += len(facet["conclusion"].split())
        claims = []
        for index, claim in enumerate(facet["claims"], start=1):
            if type(claim) is not dict or set(claim) != {"text", "evidence_ids"}:
                raise PacketPreparationError("answer-output-claim-shape")
            if type(claim["text"]) is not str or not claim["text"].strip() or type(claim["evidence_ids"]) is not list:
                raise PacketPreparationError("answer-output-claim-fields")
            codes = claim["evidence_ids"]
            if any(type(code) is not str or not _CODE.fullmatch(code) for code in codes) or len(codes) != len(
                set(codes)
            ):
                raise PacketPreparationError("answer-output-citation-shape")
            if not set(codes).issubset(delivered_codes):
                raise PacketPreparationError("answer-output-undelivered-citation")
            restored = []
            for code in codes:
                row = catalog[code]
                if _sha(str(row["text"]).encode("utf-8")) != row["passage_sha256"]:
                    raise PacketPreparationError("answer-output-catalog-integrity")
                restored.append(row["original_evidence_id"])
            total_words += len(claim["text"].split())
            claims.append(
                {
                    "claim_id": f"{facet['facet_id']}:C{index:03d}",
                    "text": claim["text"],
                    "evidence_ids": restored,
                }
            )
        frozen.append({"facet_id": facet["facet_id"], "conclusion": facet["conclusion"], "claims": claims})
    if total_words > MAX_ANSWER_WORDS:
        raise PacketPreparationError("answer-output-word-cap")
    return frozen, total_words


def _validate_source_outputs(
    prepared: PreparedAssessmentPackets, task_id: str, assessor: str, outputs: list[dict]
) -> dict[str, object]:
    if type(outputs) is not list:
        raise PacketPreparationError("source-output-list")
    expected_packets = [
        packet
        for packet in prepared.preassessment_packets
        if packet["task_id"] == task_id and packet["role"] == "source" and packet["assessor_id"] == assessor
    ]
    expected: dict[str, dict] = {}
    for packet in expected_packets:
        doc = _parse(packet["bytes"], "source-packet")
        model_input = doc["model_input"]
        if model_input["model_call_required"]:
            expected[model_input["chunk_id"]] = model_input
    observed = {}
    for output in outputs:
        if type(output) is not dict or set(output) != {"task_id", "assessor_id", "chunk_id", "source_assessments"}:
            raise PacketPreparationError("source-output-envelope")
        chunk_id = output["chunk_id"]
        if (
            chunk_id not in expected
            or chunk_id in observed
            or output["task_id"] != task_id
            or output["assessor_id"] != assessor
        ):
            raise PacketPreparationError("source-output-assignment-binding")
        plan = expected[chunk_id]
        rows = output["source_assessments"]
        successful_ids = [row["source_id"] for row in plan["sources"]]
        if (
            type(rows) is not list
            or [row.get("source_id") if type(row) is dict else None for row in rows] != successful_ids
        ):
            raise PacketPreparationError("source-output-inventory-coverage")
        for row in rows:
            if type(row) is not dict or set(row) != {"source_id", "facets"}:
                raise PacketPreparationError("source-output-row-shape")
            facets = row.get("facets")
            if type(facets) is not list or [
                facet.get("facet_id") if type(facet) is dict else None for facet in facets
            ] != [check["id"] for check in prepared.task_checks[task_id]]:
                raise PacketPreparationError("source-output-critical-check-coverage")
            codes = set()
            for source in plan["sources"]:
                if source["source_id"] == row["source_id"]:
                    codes = set(source["evidence_ids"])
                    break
            for facet in facets:
                if type(facet) is not dict or set(facet) != {
                    "facet_id",
                    "disposition",
                    "scope_match",
                    "entailment",
                    "applicability",
                    "not_assessable_reason",
                    "evidence_ids",
                    "reason",
                    "limitations",
                }:
                    raise PacketPreparationError("source-output-facet-shape")
                if (
                    type(facet["disposition"]) is not str
                    or facet["disposition"]
                    not in {"not_acquired", "acquired_unusable", "evidence_available", "not_assessable"}
                    or type(facet["scope_match"]) is not str
                    or facet["scope_match"] not in {"match", "mismatch", "uncertain", "not_assessable"}
                    or type(facet["entailment"]) is not str
                    or facet["entailment"]
                    not in {"supports", "contradicts", "qualifies", "does_not_entail", "uncertain", "not_assessable"}
                    or type(facet["applicability"]) is not str
                    or facet["applicability"] not in {"applicable", "not_applicable", "uncertain", "not_assessable"}
                    or type(facet["not_assessable_reason"]) is not str
                    or type(facet["reason"]) is not str
                    or type(facet["limitations"]) is not list
                    or any(type(item) is not str for item in facet["limitations"])
                    or type(facet["evidence_ids"]) is not list
                    or any(type(code) is not str for code in facet["evidence_ids"])
                    or len(facet["evidence_ids"]) != len(set(facet["evidence_ids"]))
                    or not set(facet["evidence_ids"]).issubset(codes)
                ):
                    raise PacketPreparationError("source-output-citation-binding")
        observed[chunk_id] = output
    if set(observed) != set(expected):
        raise PacketPreparationError("source-output-chunk-coverage")
    return {
        "packet_kind": "source",
        "assessor_id": assessor,
        "tasks": [
            {
                "task_id": task_id,
                "source_assessments": [
                    row for chunk_id in sorted(observed) for row in observed[chunk_id]["source_assessments"]
                ],
            }
        ],
    }


def prepare_answer_assessment_packets(
    *,
    prepared: PreparedAssessmentPackets,
    answer_inputs: Mapping[str, Mapping[str, bytes]],
    answer_outputs: Mapping[str, Mapping[str, object]],
    source_outputs: Mapping[str, Mapping[str, list[dict]]],
) -> PreparedAnswerPackets:
    """Prepare exactly 32 blind R1/R2 answer-assessment packets after answers.

    Only fixed schema fields and citation identifiers are validated. Answer
    text is preserved exactly; no semantic assessment or response repair runs.
    """
    if type(prepared) is not PreparedAssessmentPackets:
        raise PacketPreparationError("prepared-assessment-stage-required")
    expected_tasks = set(prepared.task_metadata)
    if (
        not isinstance(answer_inputs, Mapping)
        or not isinstance(answer_outputs, Mapping)
        or not isinstance(source_outputs, Mapping)
        or set(answer_inputs) != expected_tasks
        or set(answer_outputs) != expected_tasks
        or set(source_outputs) != expected_tasks
    ):
        raise PacketPreparationError("answer-stage-task-coverage")
    answer_prompt, answer_prompt_raw = _reference("answer-assessor-prompt.txt")
    _answer_schema_reference, answer_schema_raw = _reference("answer-assessment.schema.json")
    if _sha(answer_prompt_raw) != prepared.reference_sha256.get("answer-assessor-prompt.txt") or _sha(
        answer_schema_raw
    ) != prepared.reference_sha256.get("answer-assessment.schema.json"):
        raise PacketPreparationError("frozen-answer-reference-drift")
    packets: list[dict[str, object]] = []
    manifest_rows = []
    for task_id in prepared.task_metadata:
        if (
            not isinstance(answer_inputs[task_id], Mapping)
            or not isinstance(answer_outputs[task_id], Mapping)
            or not isinstance(source_outputs[task_id], Mapping)
        ):
            raise PacketPreparationError("answer-stage-task-mapping")
        if set(answer_inputs[task_id]) != {"w0", "candidate"} or set(answer_outputs[task_id]) != {"w0", "candidate"}:
            raise PacketPreparationError("answer-stage-arm-coverage")
        if set(source_outputs[task_id]) != {"R1", "R2"}:
            raise PacketPreparationError("answer-stage-source-assessor-coverage")
        metadata = prepared.task_metadata[task_id]
        source_ids = list(metadata["source_ids"])
        card_to_source = metadata["source_id_by_card_id"]
        contexts = prepared.task_contexts[task_id]
        catalog = prepared.task_catalogs[task_id]
        checks = prepared.task_checks[task_id]
        grader_source_views = {
            assessor: _validate_source_outputs(prepared, task_id, assessor, source_outputs[task_id][assessor])
            for assessor in ("R1", "R2")
        }
        for logical_arm in ("w0", "candidate"):
            request_bytes = answer_inputs[task_id][logical_arm]
            if type(request_bytes) is not bytes or len(request_bytes) > MAX_PACKET_BYTES:
                raise PacketPreparationError("answer-input-byte-cap")
            view = _validate_answer_input(
                request_bytes,
                task_id=task_id,
                checks=checks,
                card_to_source=card_to_source,
                contexts=contexts,
                catalog=catalog,
                expected_query=metadata.get("query"),
                expected_purpose=metadata.get("purpose"),
            )
            delivered_codes = {
                code for source_id in view["delivered_source_ids"] for code in _codes_for_source(catalog, source_id)
            }
            frozen_answer, word_count = _freeze_answer(
                answer_outputs[task_id][logical_arm], checks, delivered_codes, catalog
            )
            blind_arm_id = prepared.blind_arm_labels[task_id][logical_arm]
            delivered_contexts = view["delivered_contexts"]
            blind_opening = []
            for row in view["opening_outcomes"]:
                card_id = row["card_id"]
                blind_opening.append(
                    {
                        "card_ref": next(
                            digest
                            for digest, original_id in prepared.card_identity_maps[task_id].items()
                            if original_id == card_id
                        ),
                        "state": row["state"],
                        "delivered": row["delivered"],
                        "source_id": card_to_source[card_id],
                    }
                )
            source_inventory = [
                {
                    "source_id": source_id,
                    "state": contexts[source_id]["state"],
                    "context_sha256": contexts[source_id].get("context_sha256"),
                }
                for source_id in source_ids
            ]
            for assessor in ("R1", "R2"):
                packet_id = _digest_id(prepared.packet_stage_uuid, task_id, "answer", blind_arm_id, assessor)
                schema = _answer_schema(
                    task_id,
                    packet_id,
                    assessor,
                    blind_arm_id,
                    [row["id"] for row in checks],
                    source_ids,
                )
                payload = {
                    "schema": "coverage-answer-assessment-packet/1",
                    "packet_id": packet_id,
                    "assessor_id": assessor,
                    "packet_kind": "answer",
                    "task_id": task_id,
                    "blind_arm_id": blind_arm_id,
                    "task": {
                        "task_id": task_id,
                        "query": view["task"].get("query"),
                        "purpose": view["task"].get("purpose"),
                        "critical_checks": list(checks),
                    },
                    "opening_outcomes": blind_opening,
                    "pool_source_inventory": source_inventory,
                    "delivered_contexts": delivered_contexts,
                    "answer": frozen_answer,
                    "same_assessor_source_assessment": grader_source_views[assessor],
                    "answer_input_sha256": _sha(request_bytes),
                    "answer_sha256": _sha(_canonical(answer_outputs[task_id][logical_arm])),
                    "answer_word_count": word_count,
                    "output_schema": schema,
                    "prompt": answer_prompt,
                }
                raw = _canonical(payload)
                if len(raw) > MAX_PACKET_BYTES:
                    raise PacketPreparationError("answer-assessment-input-byte-cap")
                packets.append(
                    {
                        "packet_id": packet_id,
                        "task_id": task_id,
                        "assessor_id": assessor,
                        "role": "answer",
                        "bytes": raw,
                        "sha256": _sha(raw),
                        "byte_count": len(raw),
                    }
                )
                manifest_rows.append(
                    {
                        "packet_id": packet_id,
                        "task_id": task_id,
                        "assessor_id": assessor,
                        "blind_arm_id": blind_arm_id,
                        "answer_input_sha256": _sha(request_bytes),
                        "answer_sha256": _sha(_canonical(answer_outputs[task_id][logical_arm])),
                        "sha256": _sha(raw),
                        "byte_count": len(raw),
                    }
                )
    if len(packets) != 32:
        raise PacketPreparationError("answer-assessment-packet-count")
    manifest = {
        "schema": "coverage-answer-assessment-manifest/1",
        "packet_stage_uuid": prepared.packet_stage_uuid,
        "source_stage_uuid": prepared.source_stage_uuid,
        "private_binding_sha256": prepared.private_binding_sha256,
        "answer_prompt_sha256": _sha(answer_prompt_raw),
        "packets": manifest_rows,
        "grader_packet_count": len(packets),
        "grader_submission_ceiling": MAX_GRADER_SUBMISSIONS,
        "combined_admission_ceiling": MAX_COMBINED_ADMISSIONS,
        "model_calls_performed": 0,
        "admission_created": False,
        "quality_credit": False,
    }
    manifest_bytes = _canonical(manifest)
    return PreparedAnswerPackets(tuple(packets), manifest_bytes, _sha(manifest_bytes))


__all__ = [
    "MAX_PACKET_BYTES",
    "MAX_GRADER_SUBMISSIONS",
    "MAX_COMBINED_ADMISSIONS",
    "PacketPreparationError",
    "PreparedAnswerPackets",
    "PreparedAssessmentPackets",
    "prepare_answer_assessment_packets",
    "prepare_assessment_packets",
]
