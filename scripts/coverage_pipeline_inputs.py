"""Deterministic pre-capture inputs for the coverage-first pipeline.

This module transforms externally pinned durable native-pool snapshots plus
caller-authored cohort rows into a source-capture manifest and immutable
native-order card data. It makes no capture, ranking-provider, quality, or
admission claim.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import uuid
from dataclasses import fields, is_dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Mapping
from urllib.parse import urlsplit

from scripts.coverage_live_acquire import LiveAcquisitionError, verify_pool_snapshot_index
from scripts.coverage_study_acquire import ALLOWED_ENGINES
from slopsearx.adapter import MediaInfo, SearchResult
from slopsearx.merger import _normalise_url
from slopsearx.rerank import MAX_SNIPPET_BYTES, MAX_TITLE_BYTES
from slopsearx.service import _rerank_text, _rerank_url

CAPTURE_SCHEMA = "coverage-source-capture-manifest/1"
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")
_TASK_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")
MAX_CARDS_PER_TASK = 80
MAX_CARDS_PER_STAGE = 640
MAX_SOURCE_URL_BYTES = 4_096


class PipelineInputError(ValueError):
    """Acquisition/cohort data cannot form a complete frozen pipeline input."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise PipelineInputError("pipeline-json-invalid") from exc


def _require_text(value: object, label: str, *, nonempty: bool = True) -> str:
    if type(value) is not str or (nonempty and not value.strip()):
        raise PipelineInputError(f"{label}-invalid")
    try:
        value.encode("utf-8")
    except UnicodeEncodeError as exc:
        raise PipelineInputError(f"{label}-invalid-utf8") from exc
    return value


def _hash_pin(value: object, label: str) -> str:
    if type(value) is not str or _SHA256.fullmatch(value) is None:
        raise PipelineInputError(f"{label}-sha256-invalid")
    return value


def _plain(value: object) -> object:
    """Convert SearchResult metadata to JSON values without dropping fields."""
    if is_dataclass(value) and not isinstance(value, type):
        return {field.name: _plain(getattr(value, field.name)) for field in fields(value)}
    if isinstance(value, Mapping):
        if any(type(key) is not str for key in value):
            raise PipelineInputError("card-metadata-key-invalid")
        return {key: _plain(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        values = [_plain(item) for item in value]
        if any(type(item) is not str for item in values):
            raise PipelineInputError("card-metadata-set-invalid")
        return sorted(values)
    if isinstance(value, (list, tuple)):
        return [_plain(item) for item in value]
    if type(value) in (str, int, float, bool, type(None)):
        if type(value) is float and not math.isfinite(value):
            raise PipelineInputError("card-metadata-nonfinite")
        return value
    raise PipelineInputError("card-metadata-type-invalid")


def _canonical_public_url(value: object) -> tuple[str, str]:
    if type(value) is not str or not value:
        raise PipelineInputError("card-url-invalid")
    try:
        raw = value.encode("utf-8")
        parts = urlsplit(value)
        if (
            len(raw) > MAX_SOURCE_URL_BYTES
            or parts.scheme not in {"http", "https"}
            or not parts.hostname
            or parts.username is not None
            or parts.password is not None
        ):
            raise ValueError
        _ = parts.port
        canonical_url = _normalise_url(value)
    except (ValueError, UnicodeEncodeError) as exc:
        raise PipelineInputError("card-url-invalid") from exc
    return value, canonical_url


def _cohort_task(row: object) -> dict[str, object]:
    if type(row) is not dict:
        raise PipelineInputError("cohort-task-invalid")
    task_id = _require_text(row.get("task_id"), "task-id")
    if _TASK_ID.fullmatch(task_id) is None:
        raise PipelineInputError("task-id-invalid")
    query = _require_text(row.get("search_query"), "task-query")
    purpose = _require_text(row.get("purpose"), "task-purpose")
    task_question = _require_text(row.get("task_question"), "task-question")
    pool_plan = row.get("pool_plan")
    if type(pool_plan) is not dict or pool_plan.get("band") not in {
        "research_le_40",
        "research_41_80",
    }:
        raise PipelineInputError("cohort-pool-plan-invalid")
    engines = pool_plan.get("engines")
    if type(engines) is not list or len(engines) not in {2, 4}:
        raise PipelineInputError("cohort-pool-engines-invalid")
    if any(type(engine) is not str or engine not in ALLOWED_ENGINES for engine in engines) or len(set(engines)) != len(
        engines
    ):
        raise PipelineInputError("cohort-pool-engines-invalid")
    if len(engines) != (2 if pool_plan["band"] == "research_le_40" else 4):
        raise PipelineInputError("cohort-pool-band-engine-count-mismatch")
    raw_facets = row.get("critical_facets")
    if type(raw_facets) is not list or not 2 <= len(raw_facets) <= 3:
        raise PipelineInputError("critical-facets-shape")
    facets: list[dict[str, str]] = []
    seen: set[str] = set()
    for raw in raw_facets:
        if type(raw) is not dict:
            raise PipelineInputError("critical-facet-invalid")
        facet_id = _require_text(raw.get("facet_id"), "critical-facet-id")
        definition = _require_text(raw.get("definition"), "critical-facet-definition")
        if facet_id in seen:
            raise PipelineInputError("critical-facet-duplicate")
        seen.add(facet_id)
        # `critical_checks` are evaluator-facing checks and are deliberately
        # excluded; only caller-declared facet identity/definition is passed.
        facets.append({"id": facet_id, "description": definition})
    return {
        "task_id": task_id,
        "query": query,
        "purpose": task_question + "\n" + purpose,
        "facets": facets,
        "band": pool_plan["band"],
        "pool_engines": engines,
        "intent": _require_text(row.get("intent"), "task-intent"),
    }


def _rehydrate_pool_artifact(artifact: object) -> SimpleNamespace:
    if type(artifact) is not dict or type(artifact.get("operation")) is not dict:
        raise PipelineInputError("pool-snapshot-operation-invalid")
    operation = dict(artifact["operation"])
    raw_response = operation.get("canonical_response")
    if raw_response is not None:
        if type(raw_response) is not dict or type(raw_response.get("results")) is not list:
            raise PipelineInputError("pool-snapshot-response-invalid")
        try:
            results = []
            expected_fields = {field.name for field in fields(SearchResult)}
            for raw_result in raw_response["results"]:
                if type(raw_result) is not dict or set(raw_result) != expected_fields:
                    raise ValueError("search-result-shape")
                row = dict(raw_result)
                if type(row["engines"]) is not list:
                    raise ValueError("search-result-collections")
                row["engines"] = set(row["engines"])
                if "jev_added_engines" in row and type(row["jev_added_engines"]) is not list:
                    raise ValueError("search-result-collections")
                if row["media"] is not None:
                    if type(row["media"]) is not dict:
                        raise ValueError("media-shape")
                    row["media"] = MediaInfo(**row["media"])
                results.append(SearchResult(**row))
            response_row = dict(raw_response)
            scope = response_row.get("scope")
            if type(scope) is not dict or type(scope.get("selected_engines")) is not list:
                raise ValueError("scope-shape")
            response_row["scope"] = SimpleNamespace(**scope)
            response_row["results"] = results
            response_row["engine_outcomes"] = [SimpleNamespace(**row) for row in response_row["engine_outcomes"]]
            response = SimpleNamespace(**response_row)
            operation["canonical_response"] = response
        except (TypeError, KeyError, ValueError) as exc:
            raise PipelineInputError("pool-snapshot-response-invalid") from exc
    if type(operation.get("engines")) is list:
        operation["engines"] = tuple(operation["engines"])
    return SimpleNamespace(**operation)


def build_pipeline_inputs(
    research_cases: list[dict],
    *,
    snapshots_directory: str | Path,
    expected_snapshot_index_sha256: str,
    expected_stage_manifest_sha256: str,
    stage_uuid: str,
    source_revision: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    candidate_identity_sha256: str,
    candidate_endpoint_sha256: str,
) -> dict[str, object]:
    """Build inputs only from externally pinned, durable native-pool snapshots."""
    if type(stage_uuid) is not str:
        raise PipelineInputError("stage-uuid-invalid")
    try:
        if str(uuid.UUID(stage_uuid)) != stage_uuid:
            raise ValueError
    except ValueError as exc:
        raise PipelineInputError("stage-uuid-invalid") from exc
    if type(source_revision) is not str or _GIT_SHA.fullmatch(source_revision) is None:
        raise PipelineInputError("source-revision-invalid")
    pins = {
        "protocol_sha256": _hash_pin(protocol_sha256, "protocol"),
        "cohorts_sha256": _hash_pin(cohorts_sha256, "cohorts"),
        "candidate_identity_sha256": _hash_pin(candidate_identity_sha256, "candidate-identity"),
        "candidate_endpoint_sha256": _hash_pin(candidate_endpoint_sha256, "candidate-endpoint"),
    }
    if type(research_cases) is not list or len(research_cases) != 8:
        raise PipelineInputError("research-case-count")

    cohort_by_task: dict[str, dict[str, object]] = {}
    original_case_by_task: dict[str, dict] = {}
    for row in research_cases:
        item = _cohort_task(row)
        task_id = item["task_id"]
        if task_id in cohort_by_task:
            raise PipelineInputError("cohort-task-duplicate")
        cohort_by_task[task_id] = item
        original_case_by_task[task_id] = row

    try:
        verified = verify_pool_snapshot_index(
            snapshots_directory,
            expected_index_sha256=expected_snapshot_index_sha256,
            expected_stage_manifest_sha256=expected_stage_manifest_sha256,
            expected_stage_uuid=stage_uuid,
            expected_source_revision=source_revision,
        )
    except LiveAcquisitionError as exc:
        raise PipelineInputError(f"pool-snapshot-invalid:{exc}") from exc
    snapshot_index = verified["index"]
    if (
        snapshot_index.get("protocol_sha256") != pins["protocol_sha256"]
        or snapshot_index.get("cohorts_sha256") != pins["cohorts_sha256"]
    ):
        raise PipelineInputError("pool-snapshot-pins-mismatch")

    index_rows = snapshot_index["operations"]
    if type(index_rows) is not list or len(index_rows) != 13:
        raise PipelineInputError("acquisition-operation-count")
    if any(type(row) is not dict or row.get("status") != "complete" for row in index_rows):
        raise PipelineInputError("acquisition-operation-incomplete")
    expected_research_ids = tuple(cohort_by_task)
    operations = []
    navigation_operations = []
    ordered_ids = []
    for row in index_rows:
        operation_id = row.get("operation_id")
        artifact = verified["operations"].get(operation_id)
        op = _rehydrate_pool_artifact(artifact)
        if op.status != "complete" or row.get("kind") != op.kind:
            raise PipelineInputError("acquisition-operation-incomplete")
        ordered_ids.append(operation_id)
        (operations if op.kind == "research" else navigation_operations).append(op)
    if len(operations) != 8 or len(navigation_operations) != 5:
        raise PipelineInputError("acquisition-operation-count")
    if tuple(operation.operation_id for operation in operations) != expected_research_ids:
        raise PipelineInputError("cohort-acquisition-task-mismatch")
    if any(operation.status != "complete" or operation.band_valid is not True for operation in operations):
        raise PipelineInputError("research-pool-not-complete-and-in-band")

    source_by_canonical_url: dict[str, str] = {}
    canonical_url_by_source_id: dict[str, str] = {}
    capture_rows: list[dict[str, object]] = []
    task_outputs: list[dict[str, object]] = []
    all_bindings: list[dict[str, object]] = []
    total_cards = 0

    for operation in operations:
        task = cohort_by_task[operation.operation_id]
        artifact = verified["operations"][operation.operation_id]
        if artifact.get("task_plan") != original_case_by_task[operation.operation_id]:
            raise PipelineInputError("pool-snapshot-task-plan-mismatch")
        if operation.query != task["query"]:
            raise PipelineInputError("cohort-query-acquisition-mismatch")
        if operation.band != task["band"] or tuple(operation.engines) != tuple(task["pool_engines"]):
            raise PipelineInputError("cohort-acquisition-plan-mismatch")
        response = operation.canonical_response
        if response is None or response.query != task["query"]:
            raise PipelineInputError("canonical-pool-missing-or-query-mismatch")
        if response.ranking_explanation != "tier_then_cross_engine_presence":
            raise PipelineInputError("native-prejev-order-unverified")
        if response.scope.selected_engines != list(operation.engines):
            raise PipelineInputError("acquisition-engine-scope-mismatch")
        if artifact.get("native_order") != [f"c{index}" for index in range(len(response.results))]:
            raise PipelineInputError("native-order-snapshot-mismatch")
        if len(response.results) != operation.pool_count or not 1 <= len(response.results) <= MAX_CARDS_PER_TASK:
            raise PipelineInputError("natural-pool-size-invalid")
        pool_count = len(response.results)
        if (operation.band == "research_le_40" and not 1 <= pool_count <= 40) or (
            operation.band == "research_41_80" and not 41 <= pool_count <= 80
        ):
            raise PipelineInputError("natural-pool-band-mismatch")

        card_rows: list[dict[str, object]] = []
        projection_rows: list[dict[str, str]] = []
        native_order: list[str] = []
        source_id_by_card_id: dict[str, str] = {}
        for index, result in enumerate(response.results):
            if not isinstance(result, SearchResult):
                raise PipelineInputError("search-result-type-invalid")
            if type(result.title) is not str or type(result.content) is not str or type(result.engine) is not str:
                raise PipelineInputError("search-result-text-invalid")
            if result.position != index + 1:
                raise PipelineInputError("native-rank-position-mismatch")
            card_id = f"c{index}"
            url, canonical_url = _canonical_public_url(result.url)
            source_id = source_by_canonical_url.get(canonical_url)
            if source_id is None:
                source_id = "s-" + _sha(canonical_url.encode("utf-8"))
                previous = canonical_url_by_source_id.get(source_id)
                if previous is not None and previous != canonical_url:
                    raise PipelineInputError("source-id-collision")
                source_by_canonical_url[canonical_url] = source_id
                canonical_url_by_source_id[source_id] = canonical_url
            if any(existing["source_id"] == source_id for existing in card_rows):
                raise PipelineInputError("duplicate-canonical-url-within-pool")

            metadata = _plain(result)
            card_bytes = _canonical(metadata)
            projection = {
                "id": card_id,
                "title": _rerank_text(result.title, MAX_TITLE_BYTES),
                "url": _rerank_url(result.url),
                "snippet": _rerank_text(result.content, MAX_SNIPPET_BYTES),
            }
            projection_bytes = _canonical(projection)
            card = {
                "card_id": card_id,
                "task_id": operation.operation_id,
                "source_id": source_id,
                "native_rank": index + 1,
                "source_url": url,
                "source_url_sha256": _sha(url.encode("utf-8")),
                "canonical_public_url_sha256": _sha(canonical_url.encode("utf-8")),
                "full_metadata": metadata,
                "full_metadata_sha256": _sha(card_bytes),
                "ranking_projection": projection,
                "ranking_projection_sha256": _sha(projection_bytes),
            }
            card_rows.append(card)
            projection_rows.append(projection)
            native_order.append(card_id)
            source_id_by_card_id[card_id] = source_id
            capture_rows.append(
                {
                    "task_id": operation.operation_id,
                    "source_id": source_id,
                    "result_index": index + 1,
                    "url": result.url,
                    "title": result.title,
                    "engine": result.engine,
                }
            )
            all_bindings.append(
                {
                    "task_id": operation.operation_id,
                    "card_id": card_id,
                    "source_id": source_id,
                    "native_rank": index + 1,
                    "full_metadata_sha256": card["full_metadata_sha256"],
                    "ranking_projection_sha256": card["ranking_projection_sha256"],
                }
            )
        total_cards += len(card_rows)
        if total_cards > MAX_CARDS_PER_STAGE:
            raise PipelineInputError("stage-card-cap")
        task_outputs.append(
            {
                **task,
                "cards": card_rows,
                "native_order": native_order,
                "ranking_candidates": projection_rows,
                "source_id_by_card_id": source_id_by_card_id,
            }
        )

    capture_manifest = {
        "schema": CAPTURE_SCHEMA,
        "stage": snapshot_index["stage"],
        "stage_uuid": stage_uuid,
        "source_revision": source_revision,
        "protocol_sha256": pins["protocol_sha256"],
        "cohorts_sha256": pins["cohorts_sha256"],
        "candidate_identity_sha256": pins["candidate_identity_sha256"],
        "candidate_endpoint_sha256": pins["candidate_endpoint_sha256"],
        "sources": capture_rows,
    }
    manifest_bytes = _canonical(capture_manifest)
    return {
        "schema": "coverage-pipeline-inputs/1",
        "stage_uuid": stage_uuid,
        "stage": snapshot_index["stage"],
        "capture_manifest": capture_manifest,
        "capture_manifest_bytes": manifest_bytes,
        "capture_manifest_sha256": _sha(manifest_bytes),
        "candidate_identity_sha256": pins["candidate_identity_sha256"],
        "candidate_endpoint_sha256": pins["candidate_endpoint_sha256"],
        "tasks": task_outputs,
        "card_bindings": all_bindings,
        "card_bindings_sha256": _sha(_canonical(all_bindings)),
        "source_count": len(source_by_canonical_url),
        "card_count": total_cards,
        "quality_credit": False,
        "admission_created": False,
    }
