"""Prepare equal-budget paired answer inputs; no models or semantic grading.

Both arms consume the same immutable capture inventory. Ordering alone chooses
which five successful sources in the first ten cards are delivered. Citation
codes identify exact 600-character passages; they make no entailment claim.
All outputs are private preparation artifacts, not a registered admission.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Mapping

from scripts.coverage_study_core import StudyError, _strict_json

MAX_CARDS = 80
PREFIX = 10
MAX_SUCCESSES = 5
CONTEXT_CHARS = 8000
PASSAGE_CHARS = 600
MAX_INPUT_BYTES = 384_000
_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")


class ConsumerInputError(ValueError):
    """A sealed pool, capture, order or answer exceeds the fixed contract."""


def _canonical(value: object) -> bytes:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
            "utf-8"
        )
    except (ValueError, TypeError, UnicodeEncodeError) as exc:
        raise ConsumerInputError("invalid-json") from exc


def _digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


@dataclass(frozen=True)
class AnswerInput:
    arm: str
    body: bytes
    body_sha256: str
    delivered_source_ids: tuple[str, ...]
    delivered_evidence_ids: tuple[str, ...]


@dataclass(frozen=True)
class PairedInputs:
    answers: tuple[AnswerInput, AnswerInput]
    private_catalog_bytes: bytes
    catalog_sha256: str
    capture_inventory_sha256: str


def task_capture_contexts(
    *,
    task_id: str,
    stage_uuid: str,
    source_ids: list[str],
    capture_inventory_bytes: bytes,
    expected_capture_inventory_sha256: str,
    context_artifacts: Mapping[str, bytes],
) -> dict[str, dict]:
    """Join canonical URL aliases to exact same-stage capture artifacts.

    An inventory marked complete can contain ordinary source failures and
    unsupported URLs. Those are availability outcomes, not captured evidence.
    A duplicate can reuse only the successful owner present in this sealed
    stage; provenance and each original outcome remain in the input inventory.
    """
    if _digest(capture_inventory_bytes) != expected_capture_inventory_sha256:
        raise ConsumerInputError("capture-inventory-pin")
    try:
        inventory = _strict_json(capture_inventory_bytes, "capture-inventory")
    except StudyError as exc:
        raise ConsumerInputError("capture-inventory-json") from exc
    if type(inventory) is not dict or inventory.get("schema") != "coverage-source-capture-inventory/1":
        raise ConsumerInputError("capture-inventory-schema")
    if inventory.get("stage_uuid") != stage_uuid:
        raise ConsumerInputError("capture-stage-mismatch")
    if inventory.get("status") not in {"complete", "complete-with-source-failures"}:
        raise ConsumerInputError("capture-incomplete")
    rows = inventory.get("sources")
    if type(rows) is not list or any(type(row) is not dict for row in rows):
        raise ConsumerInputError("capture-inventory-rows")
    by_source: dict[str, dict] = {}
    task_rows: dict[str, dict] = {}
    for row in rows:
        source_id = row.get("source_id")
        if type(source_id) is not str or not _ID.fullmatch(source_id):
            raise ConsumerInputError("capture-inventory-source-id")
        if row.get("task_id") == task_id:
            if source_id in task_rows:
                raise ConsumerInputError("duplicate-task-source-row")
            task_rows[source_id] = row
        if row.get("status") == "captured":
            if source_id in by_source:
                raise ConsumerInputError("duplicate-captured-owner")
            by_source[source_id] = row
    if set(task_rows) != set(source_ids) or len(set(source_ids)) != len(source_ids):
        raise ConsumerInputError("task-source-coverage")
    result = {}
    for source_id in source_ids:
        row = task_rows[source_id]
        owner = by_source.get(source_id)
        if row.get("status") not in {"captured", "duplicate_url_not_attempted"} or owner is None:
            result[source_id] = {"state": str(row.get("status", "unknown")), "context": None, "context_sha256": None}
            continue
        artifact_name = owner.get("context_artifact")
        if type(artifact_name) is not str or not re.fullmatch(r"context-[0-9]{4}\.json", artifact_name):
            raise ConsumerInputError("capture-context-artifact-name")
        raw = context_artifacts.get(artifact_name)
        if type(raw) is not bytes or len(raw) > 64_000:
            raise ConsumerInputError("capture-context-artifact-missing-or-large")
        try:
            artifact = _strict_json(raw, "capture-context")
        except StudyError as exc:
            raise ConsumerInputError("capture-context-json") from exc
        if type(artifact) is not dict or artifact.get("schema") != "coverage-source-context/1":
            raise ConsumerInputError("capture-context-schema")
        for field, source_field in (
            ("task_id", "task_id"),
            ("source_id", "source_id"),
            ("source_url", "url"),
            ("result_index", "result_index"),
            ("title", "title"),
            ("engine", "engine"),
        ):
            if artifact.get(field) != owner.get(source_field):
                raise ConsumerInputError("capture-context-provenance")
        text = artifact.get("context")
        if type(text) is not str or len(text) > CONTEXT_CHARS:
            raise ConsumerInputError("capture-context-bound")
        text_sha = _digest(text.encode("utf-8"))
        if text_sha != artifact.get("context_sha256") or text_sha != owner.get("context_sha256"):
            raise ConsumerInputError("capture-context-integrity")
        if len(text) != artifact.get("context_characters") or len(text) != owner.get("context_characters"):
            raise ConsumerInputError("capture-context-character-count")
        result[source_id] = {"state": "success", "context": text, "context_sha256": text_sha}
    return result


def prepare_pair(
    *,
    task_index: int,
    task_id: str,
    task_question: str,
    purpose: str,
    critical_checks: list[dict],
    card_ids: list[str],
    source_id_by_card_id: Mapping[str, str],
    captures: Mapping[str, dict],
    expected_capture_inventory_sha256: str,
    orders: Mapping[str, list[str]],
) -> PairedInputs:
    """Bind identical context availability and budgets before either answer.

    The external source-qualified caller supplies and pins this full inventory.
    Failed/unknown source states stay explicit. They are never replaced with
    sources outside the fixed first-ten opening budget.
    """
    if type(task_index) is not int or not 1 <= task_index <= 8:
        raise ConsumerInputError("task-index")
    if not isinstance(task_id, str) or not _ID.fullmatch(task_id):
        raise ConsumerInputError("task-id")
    if any(type(value) is not str or not value.strip() for value in (task_question, purpose)):
        raise ConsumerInputError("task-text")
    if type(card_ids) is not list or not 1 <= len(card_ids) <= MAX_CARDS:
        raise ConsumerInputError("card-count")
    if any(type(value) is not str or not _ID.fullmatch(value) for value in card_ids) or len(set(card_ids)) != len(
        card_ids
    ):
        raise ConsumerInputError("card-membership")
    if set(source_id_by_card_id) != set(card_ids):
        raise ConsumerInputError("source-map-coverage")
    source_ids = set(source_id_by_card_id.values())
    if any(type(value) is not str or not _ID.fullmatch(value) for value in source_ids):
        raise ConsumerInputError("source-id")
    if set(captures) != source_ids:
        raise ConsumerInputError("capture-coverage")
    capture_raw = _canonical(dict(captures))
    capture_sha = _digest(capture_raw)
    if capture_sha != expected_capture_inventory_sha256:
        raise ConsumerInputError("capture-pin")
    if set(orders) != {"w0", "candidate"}:
        raise ConsumerInputError("arm-coverage")
    for order in orders.values():
        if (
            type(order) is not list
            or len(order) != len(card_ids)
            or set(order) != set(card_ids)
            or len(set(order)) != len(order)
        ):
            raise ConsumerInputError("order-not-complete-permutation")
    if type(critical_checks) is not list or not critical_checks:
        raise ConsumerInputError("critical-checks")
    check_ids = []
    for check in critical_checks:
        if type(check) is not dict or set(check) != {"id", "description"}:
            raise ConsumerInputError("critical-check-shape")
        if type(check["id"]) is not str or not _ID.fullmatch(check["id"]):
            raise ConsumerInputError("critical-check-id")
        if type(check["description"]) is not str or not check["description"].strip():
            raise ConsumerInputError("critical-check-description")
        check_ids.append(check["id"])
    if len(set(check_ids)) != len(check_ids):
        raise ConsumerInputError("duplicate-critical-check")

    # A task-wide table fixes identities across both arms while each arm receives
    # only its own delivered passages. Sorting never alters retrieval order.
    catalog: dict[str, dict] = {}
    evidence_by_source: dict[str, list[str]] = {}
    next_code = 1
    for source_id in sorted(source_ids):
        capture = captures[source_id]
        if type(capture) is not dict or set(capture) != {"state", "context", "context_sha256"}:
            raise ConsumerInputError("capture-row")
        state = capture["state"]
        if type(state) is not str or not state:
            raise ConsumerInputError("capture-state")
        evidence_by_source[source_id] = []
        if state != "success":
            if capture["context"] is not None or capture["context_sha256"] is not None:
                raise ConsumerInputError("failed-capture-context")
            continue
        text = capture["context"]
        if type(text) is not str or len(text) > CONTEXT_CHARS:
            raise ConsumerInputError("successful-context-bound")
        text_sha = _digest(text.encode("utf-8"))
        if text_sha != capture["context_sha256"]:
            raise ConsumerInputError("context-pin")
        for start in range(0, len(text), PASSAGE_CHARS):
            code = f"D{task_index}E{next_code:04d}"
            next_code += 1
            passage = text[start : start + PASSAGE_CHARS]
            catalog[code] = {
                "source_id": source_id,
                "original_evidence_id": _digest(
                    f"{source_id}\0{text_sha}\0{start}\0{start + len(passage)}".encode("utf-8")
                ),
                "context_sha256": text_sha,
                "start": start,
                "end": start + len(passage),
                "text": passage,
                "passage_sha256": _digest(passage.encode("utf-8")),
            }
            evidence_by_source[source_id].append(code)

    schema = {
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
                        "facet_id": {"enum": check_ids},
                        "conclusion": {"type": "string"},
                        "claims": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": False,
                                "required": ["text", "evidence_ids"],
                                "properties": {
                                    "text": {"type": "string"},
                                    "evidence_ids": {"type": "array", "items": {"type": "string"}, "uniqueItems": True},
                                },
                            },
                        },
                    },
                },
            }
        },
    }
    answers = []
    for arm in ("w0", "candidate"):
        prefix = orders[arm][:PREFIX]
        outcomes, delivered, delivered_ids, evidence_ids = [], [], [], []
        seen = set()
        for card_id in prefix:
            source_id = source_id_by_card_id[card_id]
            capture = captures[source_id]
            state = "duplicate" if source_id in seen else capture["state"]
            seen.add(source_id)
            accepted = state == "success" and len(delivered) < MAX_SUCCESSES
            outcomes.append({"card_id": card_id, "state": state, "delivered": accepted})
            if accepted:
                codes = evidence_by_source[source_id]
                delivered.append(
                    {
                        "source_id": source_id,
                        "passages": [{"evidence_id": code, "text": catalog[code]["text"]} for code in codes],
                    }
                )
                delivered_ids.append(source_id)
                evidence_ids.extend(codes)
        payload = {
            "task": {
                "task_id": task_id,
                "kind": "research",
                "query": task_question,
                "purpose": purpose,
                "critical_checks": critical_checks,
            },
            "cards": prefix,
            "opening_outcomes": outcomes,
            "delivered_sources": delivered,
            "output_schema": schema,
        }
        body = _canonical(payload)
        if len(body) > MAX_INPUT_BYTES:
            raise ConsumerInputError("answer-input-byte-cap")
        answers.append(AnswerInput(arm, body, _digest(body), tuple(delivered_ids), tuple(evidence_ids)))
    catalog_raw = _canonical(catalog)
    return PairedInputs((answers[0], answers[1]), catalog_raw, _digest(catalog_raw), capture_sha)


def validate_answer(answer: object, assignment: AnswerInput, *, critical_check_ids: list[str]) -> dict:
    """Check exact shape, assigned citation identities and word cap only.

    This does not judge support, applicability, correctness or completion.
    Invalid output is terminal evidence, never repaired or semantically graded
    into a structural pass.
    """
    if type(answer) is not dict or set(answer) != {"facets"} or type(answer["facets"]) is not list:
        raise ConsumerInputError("answer-shape")
    facets = answer["facets"]
    if [row.get("facet_id") if type(row) is dict else None for row in facets] != critical_check_ids:
        raise ConsumerInputError("answer-facet-coverage")
    words = 0
    for facet in facets:
        if (
            set(facet) != {"facet_id", "conclusion", "claims"}
            or type(facet["conclusion"]) is not str
            or not facet["conclusion"].strip()
            or type(facet["claims"]) is not list
        ):
            raise ConsumerInputError("answer-facet-shape")
        words += len(facet["conclusion"].split())
        for claim in facet["claims"]:
            if (
                type(claim) is not dict
                or set(claim) != {"text", "evidence_ids"}
                or type(claim["text"]) is not str
                or not claim["text"].strip()
            ):
                raise ConsumerInputError("answer-claim-shape")
            codes = claim["evidence_ids"]
            if (
                type(codes) is not list
                or any(type(code) is not str for code in codes)
                or len(set(codes)) != len(codes)
                or not set(codes).issubset(assignment.delivered_evidence_ids)
            ):
                raise ConsumerInputError("answer-undelivered-citation")
            words += len(claim["text"].split())
    if words > 800:
        raise ConsumerInputError("answer-word-cap")
    return {
        "status": "structurally-valid-not-semantically-graded",
        "words": words,
        "answer_sha256": _digest(_canonical(answer)),
    }


def restore_answer_citations(
    answer: object,
    assignment: AnswerInput,
    *,
    critical_check_ids: list[str],
    catalog_bytes: bytes,
    expected_catalog_sha256: str,
) -> dict:
    """Restore original passage hashes for unchanged downstream validators.

    Only typed citation fields are rewritten; prose and its factual claims
    remain byte-for-byte values. This is identity restoration, not repair.
    """
    validate_answer(answer, assignment, critical_check_ids=critical_check_ids)
    if _digest(catalog_bytes) != expected_catalog_sha256:
        raise ConsumerInputError("catalog-pin")
    try:
        catalog = _strict_json(catalog_bytes, "consumer-catalog")
    except StudyError as exc:
        raise ConsumerInputError("catalog-json") from exc
    if type(catalog) is not dict:
        raise ConsumerInputError("catalog-shape")
    restored = json.loads(_canonical(answer))
    for facet in restored["facets"]:
        for claim in facet["claims"]:
            original_ids = []
            for code in claim["evidence_ids"]:
                row = catalog.get(code)
                if type(row) is not dict or row.get("source_id") not in assignment.delivered_source_ids:
                    raise ConsumerInputError("catalog-source-binding")
                text_sha = row.get("context_sha256")
                original = _digest(
                    f"{row['source_id']}\0{text_sha}\0{row.get('start')}\0{row.get('end')}".encode("utf-8")
                )
                if original != row.get("original_evidence_id"):
                    raise ConsumerInputError("catalog-evidence-binding")
                original_ids.append(original)
            claim["evidence_ids"] = original_ids
    return restored
