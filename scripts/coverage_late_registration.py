"""Verify an externally sealed post-acquisition registration bundle.

Only pool-derived task-input and source-capture material pins may change from
the pre-acquisition registration. All frozen study identity, task assignment,
source closure, budgets (inside the pinned protocol), and plans stay fixed.
The helper verifies an external late registration; it never creates one.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from typing import Mapping

from scripts import coverage_live_acquire
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_study_acquire as acquisition
from scripts import coverage_study_core as core
from scripts.intent_ranking_coverage import compile_request

_SHA = re.compile(r"[0-9a-f]{64}\Z")
_LATE_MATERIALS = frozenset({"task_input_manifest", "source_capture_manifest"})
_SAVED_POOL_MODE = "saved-corpus/1"


class LateRegistrationError(ValueError):
    """The post-acquisition registration changed frozen study identity."""


def _bind_development_receipt(plan: object, bindings: dict[str, object], artifacts: dict[str, bytes]) -> None:
    """Bind the separately sealed development decision without masking collisions."""
    names = ("development_receipt_sha256", "development_stage_uuid")
    if plan.stage_kind != "confirmation":
        if any(name in bindings for name in names) or "development-pass-receipt" in artifacts:
            raise LateRegistrationError("late-registration-development-receipt-unexpected")
        return
    raw = plan.development_receipt_bytes
    pin = plan.development_receipt_sha256
    stage_uuid = plan.development_stage_uuid
    if type(raw) is not bytes or type(pin) is not str or _sha(raw) != pin or type(stage_uuid) is not str:
        raise LateRegistrationError("confirmation-development-receipt-required")
    expected = {names[0]: pin, names[1]: stage_uuid}
    if any(name in bindings and bindings[name] != value for name, value in expected.items()):
        raise LateRegistrationError("late-registration-development-receipt-binding-collision")
    if "development-pass-receipt" in artifacts and artifacts["development-pass-receipt"] != raw:
        raise LateRegistrationError("late-registration-development-receipt-artifact-collision")
    bindings.update(expected)
    artifacts.setdefault("development-pass-receipt", raw)


@dataclass(frozen=True)
class VerifiedLateRegistration:
    prepared: core.PreparedStage
    registration_sha256: str
    input_manifest_bytes: bytes
    source_capture_manifest_bytes: bytes


def build_postacquisition_materials(
    *,
    plan: object,
    acquisition_evidence: object,
    pipeline_inputs: Mapping[str, object],
    static_materials: Mapping[str, bytes],
    verified_pool_source: object | None = None,
    saved_corpus_import: object | None = None,
) -> dict[str, bytes]:
    """Rebuild the exact late core materials from verified pool snapshots.

    This returns data for an external late registration request. It never
    chooses or changes cohort tasks, candidate pools, source closure, or gates.
    """
    from scripts import coverage_stage_orchestration as orchestration

    frozen_source = verified_pool_source is not None
    saved_source = saved_corpus_import is not None
    if (
        type(plan) is not orchestration.StagePlan
        or sum((acquisition_evidence is not None, frozen_source, saved_source)) != 1
        or (acquisition_evidence is not None and type(acquisition_evidence) is not orchestration.AcquisitionEvidence)
        or (frozen_source and acquisition_evidence is not None)
        or (saved_source and (acquisition_evidence is not None or frozen_source))
        or type(pipeline_inputs) is not dict
        or type(static_materials) is not dict
        or set(static_materials) != set(core.PIN_NAMES) - _LATE_MATERIALS
        or pipeline_inputs.get("stage_uuid") != plan.stage_uuid
        or type(pipeline_inputs.get("capture_manifest_bytes")) is not bytes
    ):
        raise LateRegistrationError("postacquisition-material-input-invalid")
    if saved_source:
        from scripts import coverage_frozen_pool_inputs as frozen
        from scripts import coverage_saved_corpus as saved

        if (
            plan.pool_input_mode != _SAVED_POOL_MODE
            or type(plan.saved_origin) is not saved.SavedCorpusOrigin
            or type(saved_corpus_import) is not saved.SavedCorpusImportResult
            or not frozen.is_verified_pool_snapshot_source(saved_corpus_import.origin_pool_verified)
            or saved_corpus_import.origin_package_sha256 != plan.saved_origin.expected_package_sha256
            or saved_corpus_import.pipeline_inputs != pipeline_inputs
            or saved_corpus_import.capture_manifest_bytes != pipeline_inputs.get("capture_manifest_bytes")
            or saved_corpus_import.pipeline_inputs.get("stage_uuid") != plan.stage_uuid
            or saved_corpus_import.origin_pool_verified.pool_index.get("stage_uuid")
            != plan.saved_origin.registration.stage_uuid
        ):
            raise LateRegistrationError("saved-corpus-import-context-invalid")
    elif plan.pool_input_mode == _SAVED_POOL_MODE:
        raise LateRegistrationError("saved-corpus-import-required")
    try:
        protocol = core._strict_json(plan.protocol_bytes, "protocol")
        manifest = core._strict_json(plan.acquisition_manifest_bytes, "acquisition-manifest")
        if type(protocol) is not dict or type(manifest) is not dict:
            raise ValueError("protocol or manifest is not an object")
        protocol_pool_mode = acquisition.pool_input_mode_for_stage(protocol, plan.stage_kind)
        profile = acquisition.execution_profile_from_protocol(
            protocol.get("acquisition"), pool_input_mode=protocol_pool_mode
        )
        if saved_source:
            if protocol_pool_mode != _SAVED_POOL_MODE:
                raise ValueError("saved inputs are not registered for this stage")
            preacquisition_input = {
                "schema": "coverage-preacquisition-input-manifest/1",
                "stage_uuid": plan.stage_uuid,
                "protocol_sha256": _sha(plan.protocol_bytes),
                "cohorts_sha256": _sha(plan.cohorts_bytes),
                "task_ids": [
                    *(row["task_id"] for row in plan.research_cases),
                    *(row["target_id"] for row in plan.navigation_targets),
                ],
            }
            if plan.band_companion_sha256 is not None:
                preacquisition_input.update(
                    {
                        "band_companion_sha256": plan.band_companion_sha256,
                        "initial_registration_sha256": plan.initial_registration_sha256,
                    }
                )
            expected_manifest = {
                "schema": "coverage-saved-import-plan/1",
                "stage": plan.stage_kind,
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "source_closure_sha256": plan.source_closure_sha256,
                "protocol_sha256": _sha(plan.protocol_bytes),
                "cohorts_sha256": _sha(plan.cohorts_bytes),
                "input_manifest_sha256": _sha(core._canonical(preacquisition_input)),
                "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
                "research_cases": list(plan.research_cases),
                "navigation_targets": list(plan.navigation_targets),
                "packet_stage_uuid": plan.packet_stage_uuid,
                "origin_package_sha256": plan.saved_origin.expected_package_sha256,
            }
            if profile is acquisition.CONTINUE_ACQUISITION_PROFILE:
                expected_manifest["execution_profile"] = profile.identifier
            if manifest != expected_manifest:
                raise ValueError("saved import plan does not match the pinned stage and origin")
        else:
            manifest_profile = acquisition.execution_profile_from_manifest(manifest)
            if profile != manifest_profile:
                raise ValueError("acquisition profile does not match the pinned protocol")
    except Exception as exc:
        raise LateRegistrationError("postacquisition-profile-binding-invalid") from exc
    try:
        if saved_source:
            verified_source = saved_corpus_import.origin_pool_verified
            registration = plan.saved_origin.registration
            if (
                verified_source.pool_index.get("source_revision") != registration.source_revision
                or verified_source.pool_index.get("source_closure_sha256") != registration.source_closure_sha256
                or verified_source.pool_index.get("protocol_sha256") != registration.protocol_sha256
                or verified_source.pool_index.get("cohorts_sha256") != registration.cohorts_sha256
                or verified_source.pool_index.get("candidate_identity_sha256") != registration.candidate_identity_sha256
                or verified_source.pool_index.get("candidate_endpoint_sha256") != registration.candidate_endpoint_sha256
            ):
                raise ValueError("saved origin pool binding mismatch")
            verified = {"operations": verified_source.operations}
        elif frozen_source:
            from scripts.coverage_frozen_pool_inputs import is_verified_pool_snapshot_source

            if not is_verified_pool_snapshot_source(verified_pool_source):
                raise ValueError("unverified frozen pool source")
            if (
                verified_pool_source.pool_manifest_sha256 != plan.expected_acquisition_manifest_sha256
                or verified_pool_source.pool_index.get("stage_uuid") != plan.stage_uuid
                or verified_pool_source.pool_index.get("source_revision") != plan.source_revision
                or verified_pool_source.pool_index.get("source_closure_sha256") != plan.source_closure_sha256
                or verified_pool_source.pool_index.get("protocol_sha256") != _sha(plan.protocol_bytes)
                or verified_pool_source.pool_index.get("cohorts_sha256") != _sha(plan.cohorts_bytes)
            ):
                raise ValueError("frozen pool source binding mismatch")
            verified = {"operations": verified_pool_source.operations}
        else:
            index = core._sha256(acquisition_evidence.snapshot_index_sha256, "acquisition-index")
            verified = coverage_live_acquire.verify_pool_snapshot_index(
                acquisition_evidence.snapshots_directory,
                expected_index_sha256=index,
                expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
                expected_stage_uuid=plan.stage_uuid,
                expected_source_revision=plan.source_revision,
                expected_execution_profile=profile.identifier,
            )
    except Exception as exc:
        raise LateRegistrationError("postacquisition-snapshot-invalid") from exc
    tasks = pipeline_inputs.get("tasks")
    if type(tasks) is not list or len(tasks) != 8:
        raise LateRegistrationError("postacquisition-task-inventory")
    research_tasks = []
    for task in tasks:
        if type(task) is not dict:
            raise LateRegistrationError("postacquisition-task-row")
        candidates = task.get("ranking_candidates")
        native_order = task.get("native_order")
        try:
            compiled = compile_request(
                query=task["query"],
                purpose=task["purpose"],
                facets=task["facets"],
                candidates=candidates,
                incumbent_order=native_order,
            )
        except Exception as exc:
            raise LateRegistrationError("postacquisition-task-input-invalid") from exc
        research_tasks.append(
            {
                "task_id": task["task_id"],
                "query": task["query"],
                "purpose": task["purpose"],
                "facets": task["facets"],
                "candidates": candidates,
                "incumbent_order": native_order,
                "compiled_request_sha256": _sha(compiled.body),
            }
        )
    target_rows = list(plan.navigation_targets)
    if len(target_rows) != 5:
        raise LateRegistrationError("postacquisition-navigation-count")
    operations = verified["operations"]
    navigation_tasks = []
    for target in target_rows:
        target_id = target["target_id"]
        artifact = operations.get(target_id)
        operation = artifact.get("operation") if type(artifact) is dict else None
        response = operation.get("canonical_response") if type(operation) is dict else None
        rows = response.get("results") if type(response) is dict else None
        if type(rows) is not list or not rows:
            raise LateRegistrationError("postacquisition-navigation-pool-invalid")
        pool_ids = [f"n{index}" for index in range(len(rows))]
        target_url = target.get("target_url")
        try:
            target_canonical = pipeline._canonical_public_url(target_url)[1]
            matching_ids = [
                pool_ids[index]
                for index, row in enumerate(rows)
                if type(row) is dict and pipeline._canonical_public_url(row.get("url"))[1] == target_canonical
            ]
        except Exception as exc:
            raise LateRegistrationError("postacquisition-navigation-target-invalid") from exc
        if len(matching_ids) != 1 or response.get("query") != target.get("search_query"):
            raise LateRegistrationError("postacquisition-navigation-target-not-acquired")
        navigation_tasks.append(
            {
                "task_id": target_id,
                "query": target["search_query"],
                "target_candidate_id": matching_ids[0],
                "pool_ids": pool_ids,
                "w0_order": pool_ids,
            }
        )
    input_bytes = core._canonical(
        {
            "schema": core.INPUT_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "research_tasks": research_tasks,
            "navigation_tasks": navigation_tasks,
        }
    )
    if (
        static_materials["protocol"] != plan.protocol_bytes
        or static_materials["acquisition_plan"] != plan.acquisition_plan_bytes
        or _sha(static_materials["qualified_source_closure"]) != plan.source_closure_sha256
    ):
        raise LateRegistrationError("postacquisition-static-binding-mismatch")
    materials = dict(static_materials)
    materials["task_input_manifest"] = input_bytes
    capture_bytes = pipeline_inputs.get("capture_manifest_bytes")
    if type(capture_bytes) is not bytes:
        raise LateRegistrationError("postacquisition-capture-manifest-invalid")
    materials["source_capture_manifest"] = capture_bytes
    return materials


def saved_corpus_import_reference(result: object, *, plan: object) -> bytes:
    """Return a small, source-bound reference to a verified saved import."""
    from scripts import coverage_saved_corpus as saved
    from scripts import coverage_stage_orchestration as orchestration

    if (
        type(plan) is not orchestration.StagePlan
        or plan.pool_input_mode != _SAVED_POOL_MODE
        or type(result) is not saved.SavedCorpusImportResult
        or result.pipeline_inputs.get("stage_uuid") != plan.stage_uuid
        or result.origin_package_sha256 != plan.saved_origin.expected_package_sha256
        or result.current_search_calls != 0
        or result.current_capture_calls != 0
    ):
        raise LateRegistrationError("saved-corpus-import-reference-invalid")
    receipt = saved._strict(result.import_receipt_bytes, "saved-import-receipt")
    expected = {
        "schema": saved.SCHEMA,
        "origin_stage_uuid": plan.saved_origin.registration.stage_uuid,
        "origin_package_sha256": result.origin_package_sha256,
        "stage_uuid": plan.stage_uuid,
        "packet_stage_uuid": plan.packet_stage_uuid,
        "current_search_calls": 0,
        "current_capture_calls": 0,
        "historical_search_calls": result.historical_search_calls,
        "historical_capture_calls": result.historical_capture_calls,
        "historical_health_calls": result.historical_health_calls,
        "historical_scrape_calls": result.historical_scrape_calls,
        "source_rows": result.historical_scrape_calls,
        "captured_contexts": result.historical_captured_sources,
        "source_failures": result.historical_failed_sources,
        "import_authority_receipt_sha256": receipt.get("import_authority_receipt_sha256"),
        "derived_capture_manifest_sha256": result.capture_manifest_sha256,
    }
    if type(receipt) is not dict or any(receipt.get(key) != value for key, value in expected.items()):
        raise LateRegistrationError("saved-corpus-import-receipt-mismatch")
    inventory = saved._strict(result.capture_inventory_bytes, "saved-capture-inventory")
    if (
        inventory.get("schema") != saved.CAPTURE_IMPORT_SCHEMA
        or inventory.get("stage_uuid") != plan.stage_uuid
        or inventory.get("origin_stage_uuid") != plan.saved_origin.registration.stage_uuid
        or inventory.get("origin_inventory_sha256") != receipt.get("origin_capture_inventory_sha256")
        or inventory.get("import_receipt_sha256") != result.import_receipt_sha256
    ):
        raise LateRegistrationError("saved-corpus-capture-inventory-mismatch")
    return core._canonical(
        {
            "schema": "coverage-saved-corpus-import-reference/1",
            "stage_uuid": plan.stage_uuid,
            "origin_stage_uuid": plan.saved_origin.registration.stage_uuid,
            "origin_package_sha256": result.origin_package_sha256,
            "import_receipt_sha256": result.import_receipt_sha256,
            "capture_manifest_sha256": result.capture_manifest_sha256,
            "capture_inventory_sha256": _sha(result.capture_inventory_bytes),
            "source_rows": result.historical_scrape_calls,
            "captured_contexts": result.historical_captured_sources,
            "failed_contexts": result.historical_failed_sources,
            "current_search_calls": 0,
            "current_capture_calls": 0,
        }
    )


async def request_saved_corpus_import(
    *,
    exchange: object,
    plan: object,
    expected_initial_registration_sha256: str,
    stage_clock_bytes: bytes,
    phase_bindings: Mapping[str, object] | None = None,
    phase_artifacts: Mapping[str, bytes] | None = None,
) -> object:
    """Obtain a fresh early permit and derive current-stage inputs from the pinned archive."""
    from scripts import coverage_operator_handoff
    from scripts import coverage_saved_corpus as saved
    from scripts import coverage_stage_orchestration as orchestration
    from scripts.coverage_local_authority import PinnedReceiptAuthorities

    if (
        type(exchange) is not coverage_operator_handoff.OperatorReceiptHandoff
        or type(plan) is not orchestration.StagePlan
        or plan.pool_input_mode != _SAVED_POOL_MODE
        or plan.stage_kind != "development"
        or type(plan.saved_origin) is not saved.SavedCorpusOrigin
    ):
        raise LateRegistrationError("saved-corpus-import-context-invalid")
    if type(stage_clock_bytes) is not bytes or not stage_clock_bytes:
        raise LateRegistrationError("saved-corpus-import-stage-clock-required")
    origin = plan.saved_origin
    try:
        package_sha = saved.package_sha256(origin.collection_directory, origin.capture_stage_directory)
        collection_index_sha = saved.resource_evidence.receipt_inventory_sha256(origin.collection_directory)
        capture_inventory_sha = _sha(
            saved._private_file(origin.capture_stage_directory / "inventory.json", max_bytes=4_000_000)
        )
        capture_manifest_sha = _sha(
            saved._private_file(origin.capture_stage_directory / "source-manifest.json", max_bytes=4_000_000)
        )
    except Exception as exc:
        raise LateRegistrationError("saved-corpus-origin-unavailable") from exc
    if package_sha != origin.expected_package_sha256:
        raise LateRegistrationError("saved-corpus-origin-package-mismatch")
    bindings = {
        "stage_uuid": plan.stage_uuid,
        "packet_stage_uuid": plan.packet_stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
        "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        "origin_package_sha256": package_sha,
    }
    origin_reference = core._canonical(
        {
            "schema": "coverage-saved-corpus-origin-reference/1",
            "origin_stage_uuid": origin.registration.stage_uuid,
            "origin_source_revision": origin.registration.source_revision,
            "origin_package_sha256": package_sha,
            "collection_receipt_inventory_sha256": collection_index_sha,
            "capture_inventory_sha256": capture_inventory_sha,
            "capture_manifest_sha256": capture_manifest_sha,
        }
    )
    if type(phase_bindings) is not dict or type(phase_artifacts) is not dict:
        raise LateRegistrationError("saved-corpus-import-phase-context-required")
    if any(key in bindings and bindings[key] != value for key, value in phase_bindings.items()):
        raise LateRegistrationError("saved-corpus-import-phase-binding-collision")
    request_bindings = {**bindings, **phase_bindings}
    if any(
        name == "saved-corpus-origin-reference" and raw != origin_reference for name, raw in phase_artifacts.items()
    ):
        raise LateRegistrationError("saved-corpus-import-phase-artifact-collision")
    artifacts = {**phase_artifacts, "saved-corpus-origin-reference": origin_reference}
    artifacts["stage-clock"] = stage_clock_bytes
    try:
        receipt, receipt_sha = await exchange.request_async(
            stage_uuid=plan.stage_uuid,
            scope="saved-corpus-import",
            request_id="permit",
            bindings=request_bindings,
            deadline_monotonic=plan.stage_deadline_monotonic,
            artifacts=artifacts,
        )
        permit = PinnedReceiptAuthorities.saved_corpus_import(plan, receipt, receipt_sha)
        result = saved.verify_and_import_saved_corpus(
            origin,
            saved.SavedCorpusStageBinding(
                stage_uuid=plan.stage_uuid,
                packet_stage_uuid=plan.packet_stage_uuid,
                stage_kind=plan.stage_kind,
                source_revision=plan.source_revision,
                source_closure_sha256=plan.source_closure_sha256,
                protocol_sha256=_sha(plan.protocol_bytes),
                cohorts_sha256=_sha(plan.cohorts_bytes),
                candidate_identity_sha256=_sha(plan.candidate_identity_bytes),
                candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
                origin_package_sha256=permit.origin_package_sha256,
                import_authority_receipt_sha256=permit.receipt_sha256,
            ),
            plan.research_cases,
            plan.navigation_targets,
        )
    except Exception as exc:
        raise LateRegistrationError("saved-corpus-import-failed") from exc
    return result


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _registration(raw: bytes, expected_sha256: str) -> dict[str, object]:
    if type(raw) is not bytes or not raw or type(expected_sha256) is not str or not _SHA.fullmatch(expected_sha256):
        raise LateRegistrationError("late-registration-input-invalid")
    if _sha(raw) != expected_sha256:
        raise LateRegistrationError("late-registration-pin-mismatch")
    value = core._strict_json(raw, "late-registration")
    if type(value) is not dict or core._canonical(value) != raw:
        raise LateRegistrationError("late-registration-noncanonical")
    return value


def verify_late_registration(
    *,
    initial_registration_bytes: bytes,
    expected_initial_registration_sha256: str,
    late_registration_bytes: bytes,
    expected_late_registration_sha256: str,
    materials: Mapping[str, bytes],
    expected_source_revision: str,
    expected_stage_uuid: str,
    forbidden_stage_uuids: tuple[str, ...],
    task_ids: tuple[str, ...],
    navigation_skip_reasons: tuple[str | None, ...],
    band_companion_bytes: bytes | None = None,
    expected_band_companion_sha256: str | None = None,
    expected_source_pins: Mapping[str, str] | None = None,
    development_receipt_bytes: bytes | None = None,
    expected_development_receipt_sha256: str | None = None,
    expected_development_stage_uuid: str | None = None,
) -> VerifiedLateRegistration:
    """Validate lineage and run the original pure registration preflight."""
    initial = _registration(initial_registration_bytes, expected_initial_registration_sha256)
    late = _registration(late_registration_bytes, expected_late_registration_sha256)
    expected_initial_fields = {
        "schema",
        "stage_uuid",
        "stage_kind",
        "source_revision",
        "pins",
        "operation_ids",
        "navigation_skip_reasons",
        "task_ids",
    }
    if set(initial) not in (expected_initial_fields, expected_initial_fields | {"band_companion_sha256"}):
        raise LateRegistrationError("initial-registration-shape")
    if set(late) != set(initial):
        raise LateRegistrationError("late-registration-shape")
    if (band_companion_bytes is None) != (expected_band_companion_sha256 is None):
        raise LateRegistrationError("late-registration-companion-binding-incomplete")
    if initial.get("band_companion_sha256") != expected_band_companion_sha256 or (
        band_companion_bytes is not None and _sha(band_companion_bytes) != expected_band_companion_sha256
    ):
        raise LateRegistrationError("late-registration-companion-binding-mismatch")
    if type(initial.get("pins")) is not dict or type(late.get("pins")) is not dict:
        raise LateRegistrationError("late-registration-pins-shape")
    if set(initial["pins"]) != set(core.PIN_NAMES) or set(late["pins"]) != set(core.PIN_NAMES):
        raise LateRegistrationError("late-registration-pins-inventory")
    immutable_fields = {key for key in initial if key != "pins"}
    if any(initial[key] != late[key] for key in immutable_fields):
        raise LateRegistrationError("late-registration-lineage-drift")
    for name in core.PIN_NAMES:
        if name not in _LATE_MATERIALS and initial["pins"][name] != late["pins"][name]:
            raise LateRegistrationError(f"late-registration-frozen-pin-drift:{name}")
    if type(materials) is not dict or set(materials) != set(core.PIN_NAMES):
        raise LateRegistrationError("late-registration-material-inventory")
    pins = late["pins"]
    for name in core.PIN_NAMES:
        raw = materials[name]
        if type(raw) is not bytes or _sha(raw) != pins[name]:
            raise LateRegistrationError(f"late-registration-material-pin-mismatch:{name}")
    try:
        prepared = core.preflight_stage(
            registration_bytes=late_registration_bytes,
            expected_registration_sha256=expected_late_registration_sha256,
            expected_stage_uuid=expected_stage_uuid,
            forbidden_stage_uuids=forbidden_stage_uuids,
            expected_source_revision=expected_source_revision,
            observed_source_revision=expected_source_revision,
            materials=materials,
            expected_pins=pins,
            acquisition_plan_bytes=materials["acquisition_plan"],
            capture_plan_bytes=materials["capture_plan"],
            task_ids=task_ids,
            navigation_skip_reasons=navigation_skip_reasons,
            development_receipt_bytes=development_receipt_bytes,
            expected_development_receipt_sha256=expected_development_receipt_sha256,
            expected_development_stage_uuid=expected_development_stage_uuid,
            band_companion_bytes=band_companion_bytes,
            expected_band_companion_sha256=expected_band_companion_sha256,
            expected_source_pins=expected_source_pins,
        )
    except Exception as exc:
        raise LateRegistrationError("late-registration-core-preflight-failed") from exc
    return VerifiedLateRegistration(
        prepared=prepared,
        registration_sha256=expected_late_registration_sha256,
        input_manifest_bytes=materials["task_input_manifest"],
        source_capture_manifest_bytes=materials["source_capture_manifest"],
    )


async def request_late_registration(
    *,
    exchange: object,
    plan: object,
    acquisition_evidence: object | None,
    pipeline_inputs: Mapping[str, object],
    static_materials: Mapping[str, bytes],
    initial_registration_bytes: bytes,
    expected_initial_registration_sha256: str,
    forbidden_stage_uuids: tuple[str, ...],
    task_ids: tuple[str, ...],
    navigation_skip_reasons: tuple[str | None, ...],
    verified_pool_source: object | None = None,
    saved_corpus_import: object | None = None,
    stage_clock_bytes: bytes | None = None,
    phase_bindings: Mapping[str, object] | None = None,
    phase_artifacts: Mapping[str, bytes] | None = None,
) -> object:
    """Wait for an external late registration, then run core preflight.

    The exchange expects a separate pin file in its private operator root.
    This function never writes a registration and never turns preflight into
    execution authorization.
    """
    from scripts import coverage_operator_handoff
    from scripts import coverage_stage_orchestration as orchestration

    if type(exchange) is not coverage_operator_handoff.OperatorReceiptHandoff:
        raise LateRegistrationError("late-registration-operator-exchange-required")
    if (
        type(plan) is not orchestration.StagePlan
        or sum(
            (
                type(acquisition_evidence) is orchestration.AcquisitionEvidence,
                verified_pool_source is not None,
                saved_corpus_import is not None,
            )
        )
        != 1
    ):
        raise LateRegistrationError("late-registration-stage-context-invalid")
    materials = build_postacquisition_materials(
        plan=plan,
        acquisition_evidence=acquisition_evidence,
        pipeline_inputs=pipeline_inputs,
        static_materials=static_materials,
        verified_pool_source=verified_pool_source,
        saved_corpus_import=saved_corpus_import,
    )
    immutable = _registration(initial_registration_bytes, expected_initial_registration_sha256)
    saved_source = saved_corpus_import is not None
    source_index_sha = (
        saved_corpus_import.origin_pool_verified.snapshot_index_sha256
        if saved_source
        else verified_pool_source.snapshot_index_sha256
        if verified_pool_source is not None
        else acquisition_evidence.snapshot_index_sha256
    )
    source_manifest_sha = (
        saved_corpus_import.origin_pool_verified.pool_manifest_sha256
        if saved_source
        else verified_pool_source.pool_manifest_sha256
        if verified_pool_source is not None
        else acquisition_evidence.acquisition_manifest_sha256
    )
    request_bindings = {
        "initial_registration_sha256": expected_initial_registration_sha256,
        "stage_uuid": plan.stage_uuid,
        "source_revision": plan.source_revision,
        "source_closure_sha256": plan.source_closure_sha256,
        "protocol_sha256": _sha(plan.protocol_bytes),
        "cohorts_sha256": _sha(plan.cohorts_bytes),
        "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
        "acquisition_manifest_sha256": source_manifest_sha,
        "snapshot_index_sha256": source_index_sha,
        "task_ids": list(task_ids),
        "operation_ids": immutable.get("operation_ids"),
        "navigation_skip_reasons": list(navigation_skip_reasons),
        "late_material_sha256": {name: _sha(materials[name]) for name in sorted(_LATE_MATERIALS)},
    }
    if phase_bindings is not None:
        if type(phase_bindings) is not dict or any(
            key in request_bindings and request_bindings[key] != value for key, value in phase_bindings.items()
        ):
            raise LateRegistrationError("late-registration-phase-binding-collision")
        request_bindings.update(phase_bindings)
    artifact_bytes = {f"material-{name.replace('_', '-')}": raw for name, raw in materials.items()}
    artifact_bytes["capture-manifest"] = pipeline_inputs["capture_manifest_bytes"]
    if saved_source:
        saved_reference = saved_corpus_import_reference(saved_corpus_import, plan=plan)
        request_bindings["saved_corpus_import_receipt_sha256"] = saved_corpus_import.import_receipt_sha256
        request_bindings["origin_package_sha256"] = saved_corpus_import.origin_package_sha256
        artifact_bytes["saved-corpus-import-reference"] = saved_reference
        artifact_bytes["saved-corpus-import-receipt-sha256"] = saved_corpus_import.import_receipt_sha256.encode("ascii")
    if stage_clock_bytes is not None:
        artifact_bytes["stage-clock"] = stage_clock_bytes
    _bind_development_receipt(plan, request_bindings, artifact_bytes)
    if verified_pool_source is not None:
        # This same-host, source-closed handle points only to the fixed
        # stage_inventory/<uuid>/collection location. The issuer must reopen
        # that sealed inventory and run verify_frozen_pool_evidence; no caller
        # supplied filesystem path or copied multi-megabyte response is trusted.
        collection_root = verified_pool_source.snapshots_directory
        if collection_root.name != "collection" or collection_root.parent.name != plan.stage_uuid:
            raise LateRegistrationError("frozen-pool-inventory-location-invalid")
        artifact_bytes["frozen-pool-reference"] = core._canonical(
            {
                "schema": "coverage-frozen-pool-reference/1",
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "source_closure_sha256": plan.source_closure_sha256,
                "protocol_sha256": _sha(plan.protocol_bytes),
                "cohorts_sha256": _sha(plan.cohorts_bytes),
                "pool_manifest_sha256": verified_pool_source.pool_manifest_sha256,
                "snapshot_index_sha256": verified_pool_source.snapshot_index_sha256,
                "evidence_sha256": verified_pool_source.evidence_sha256,
                "availability_sha256": verified_pool_source.availability_sha256,
                "collector_receipt_sha256": verified_pool_source.collector_receipt_sha256,
                "proof_sha256": verified_pool_source.proof_sha256,
                "relative_inventory": "collection",
            }
        )
    if phase_artifacts is not None:
        if type(phase_artifacts) is not dict or any(
            type(name) is not str or type(raw) is not bytes or (name in artifact_bytes and artifact_bytes[name] != raw)
            for name, raw in phase_artifacts.items()
        ):
            raise LateRegistrationError("late-registration-phase-artifact-collision")
        artifact_bytes.update(phase_artifacts)
    try:
        registration_bytes, registration_sha256 = await exchange.request_async(
            stage_uuid=plan.stage_uuid,
            scope="late-registration",
            request_id="registration-1",
            bindings=request_bindings,
            deadline_monotonic=plan.stage_deadline_monotonic,
            artifacts=artifact_bytes,
        )
    except Exception as exc:
        raise LateRegistrationError("late-registration-handoff-failed") from exc
    verified = verify_late_registration(
        initial_registration_bytes=initial_registration_bytes,
        expected_initial_registration_sha256=expected_initial_registration_sha256,
        late_registration_bytes=registration_bytes,
        expected_late_registration_sha256=registration_sha256,
        materials=materials,
        expected_source_revision=plan.source_revision,
        expected_stage_uuid=plan.stage_uuid,
        forbidden_stage_uuids=forbidden_stage_uuids,
        task_ids=task_ids,
        navigation_skip_reasons=navigation_skip_reasons,
        band_companion_bytes=plan.band_companion_bytes,
        expected_band_companion_sha256=plan.band_companion_sha256,
        expected_source_pins=plan.band_companion_source_pins,
        development_receipt_bytes=plan.development_receipt_bytes,
        expected_development_receipt_sha256=plan.development_receipt_sha256,
        expected_development_stage_uuid=plan.development_stage_uuid,
    )
    return orchestration.LatePreparationEvidence(
        prepared=verified.prepared,
        input_manifest_bytes=verified.input_manifest_bytes,
        preflight_receipt_sha256=verified.registration_sha256,
    )
