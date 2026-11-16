"""Strict adapters for operator-pinned receipts and local one-shot claims.

Receipt digests are supplied separately from the receipt bytes. This module
checks exact schemas and execution bindings; it does not issue permits,
qualification, selector admission, or a cryptographic signature claim.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import stat
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Mapping

from scripts import coverage_answer_execution as answer
from scripts import coverage_live_acquire as acquisition
from scripts import coverage_operator_handoff, coverage_study_acquire
from scripts import coverage_source_capture as capture

SCHEMA = "coverage-operator-authority-receipt/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")


class LocalAuthorityError(ValueError):
    """An operator-pinned receipt or local lease failed closed validation."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _phase_request_context_bindings(
    request_bindings: object,
    *,
    expected_source_closure_sha256: str,
    expected_scope_bindings: Mapping[str, object],
    expected_context_bindings: Mapping[str, object] | None = None,
) -> dict[str, object]:
    """Validate the runner's common context without loosening scope bindings."""
    common_keys = {
        "source_closure_sha256",
        "active_python_environment",
        "active_python_environment_sha256",
    }
    context_bindings = dict(expected_context_bindings or {})
    expected_all = {**expected_scope_bindings, **context_bindings}
    if (
        type(request_bindings) is not dict
        or set(request_bindings) != set(expected_all) | common_keys
        or request_bindings.get("source_closure_sha256") != expected_source_closure_sha256
    ):
        raise LocalAuthorityError("phase-artifact-request-context-binding-mismatch")
    environment = request_bindings.get("active_python_environment")
    environment_sha256 = request_bindings.get("active_python_environment_sha256")
    if (
        type(environment) is not dict
        or environment.get("schema") != "coverage-active-python-environment/1"
        or type(environment_sha256) is not str
        or not _SHA.fullmatch(environment_sha256)
        or _sha(_canonical(environment)) != environment_sha256
    ):
        raise LocalAuthorityError("phase-artifact-active-environment-binding-invalid")
    scope_bindings = {key: value for key, value in request_bindings.items() if key not in common_keys}
    if scope_bindings != expected_all:
        raise LocalAuthorityError("phase-artifact-request-scope-binding-mismatch")
    return {key: value for key, value in scope_bindings.items() if key not in context_bindings}


_DEVELOPMENT_RECEIPT_BINDINGS = frozenset({"development_receipt_sha256", "development_stage_uuid"})


def _verified_development_receipt_context(
    plan: object | None,
    registration: Mapping[str, object],
    request_bindings: object,
    artifacts: Mapping[str, bytes],
) -> dict[str, str]:
    """Validate only the known confirmation proof fields against the source-bound plan."""
    registered_kind = registration.get("stage_kind")
    if registered_kind not in {"development", "confirmation"}:
        raise LocalAuthorityError("phase-artifact-registered-stage-kind-invalid")
    if plan is None:
        if registered_kind != "development":
            raise LocalAuthorityError("phase-artifact-confirmation-plan-required")
    elif getattr(plan, "stage_kind", None) != registered_kind:
        raise LocalAuthorityError("phase-artifact-plan-stage-kind-mismatch")
    if type(request_bindings) is not dict:
        raise LocalAuthorityError("phase-artifact-development-receipt-request-invalid")
    if registered_kind != "confirmation":
        if "development-pass-receipt" in artifacts or _DEVELOPMENT_RECEIPT_BINDINGS.intersection(request_bindings):
            raise LocalAuthorityError("phase-artifact-development-receipt-unexpected")
        return {}
    raw = getattr(plan, "development_receipt_bytes", None)
    pin = getattr(plan, "development_receipt_sha256", None)
    dev_uuid = getattr(plan, "development_stage_uuid", None)
    if (
        type(raw) is not bytes
        or not raw
        or type(pin) is not str
        or not _SHA.fullmatch(pin)
        or _sha(raw) != pin
        or type(dev_uuid) is not str
        or artifacts.get("development-pass-receipt") != raw
        or request_bindings.get("development_receipt_sha256") != pin
        or request_bindings.get("development_stage_uuid") != dev_uuid
    ):
        raise LocalAuthorityError("phase-artifact-development-receipt-mismatch")
    try:
        from scripts import coverage_study_core as core

        core._verify_development_receipt(
            raw,
            expected_sha256=pin,
            current_stage_uuid=plan.stage_uuid,
            expected_development_stage_uuid=dev_uuid,
            forbidden_stage_uuids=(),
            protocol_sha256=_sha(plan.protocol_bytes),
            source_revision=plan.source_revision,
            source_closure_sha256=plan.source_closure_sha256,
        )
    except Exception as exc:
        raise LocalAuthorityError("phase-artifact-development-receipt-invalid") from exc
    return {"development_receipt_sha256": pin, "development_stage_uuid": dev_uuid}


def _strict_json(raw: bytes) -> object:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise LocalAuthorityError("receipt-duplicate-key")
            result[key] = value
        return result

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
        )
    except LocalAuthorityError:
        raise
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError, RecursionError) as exc:
        raise LocalAuthorityError("receipt-json-invalid") from exc


def _pinned_document(
    raw: bytes,
    expected_sha256: str,
    *,
    scope: str,
    bindings: Mapping[str, object],
    extra: Mapping[str, object] | None = None,
    allow_evidence: bool = False,
) -> dict[str, object]:
    if (
        type(raw) is not bytes
        or not raw
        or len(raw) > 16_384
        or type(expected_sha256) is not str
        or not _SHA.fullmatch(expected_sha256)
        or _sha(raw) != expected_sha256
    ):
        raise LocalAuthorityError("receipt-pin-or-binding-invalid")
    document = _strict_json(raw)
    expected = {
        "schema": SCHEMA,
        "decision": "allow",
        "scope": scope,
        "bindings": _strict_json(_canonical(dict(bindings))),
    }
    if extra is not None:
        expected["evidence"] = _strict_json(_canonical(dict(extra)))
    expected_keys = set(expected)
    if allow_evidence:
        expected_keys.add("evidence")
    if (
        type(document) is not dict
        or set(document) != expected_keys
        or any(document.get(key) != value for key, value in expected.items())
        or (allow_evidence and type(document.get("evidence")) is not dict)
        or _canonical(document) != raw
    ):
        raise LocalAuthorityError("receipt-claims-mismatch")
    return document


class PinnedReceiptAuthorities:
    """Construct existing typed permits only from exact externally pinned claims."""

    @staticmethod
    def acquisition(
        manifest: Mapping[str, object],
        manifest_sha256: str,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
    ) -> acquisition.VerifiedAcquisitionPermit:
        bindings = {
            "stage_uuid": manifest["stage_uuid"],
            "source_revision": manifest["source_revision"],
            "source_closure_sha256": manifest["source_closure_sha256"],
            "protocol_sha256": manifest["protocol_sha256"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "input_manifest_sha256": manifest["input_manifest_sha256"],
            "acquisition_plan_sha256": manifest["acquisition_plan_sha256"],
            "stage_manifest_sha256": manifest_sha256,
        }
        profile = coverage_study_acquire.execution_profile_from_manifest(manifest)
        if profile is coverage_study_acquire.CONTINUE_ACQUISITION_PROFILE:
            bindings["execution_profile"] = profile.identifier
        _pinned_document(receipt_bytes, expected_receipt_sha256, scope="source-acquisition", bindings=bindings)
        return acquisition.VerifiedAcquisitionPermit(
            status="verified",
            scope="source-acquisition",
            **bindings,
            receipt_sha256=expected_receipt_sha256,
            **(
                {}
                if profile is coverage_study_acquire.CONTINUE_ACQUISITION_PROFILE
                else {"execution_profile": profile.identifier}
            ),
        )

    @staticmethod
    def source_capture(
        manifest_bytes: bytes,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
        ca_bundle_sha256: str | None = None,
    ) -> capture.VerifiedSourceCapturePermit:
        manifest = _strict_json(manifest_bytes)
        if type(manifest) is not dict or type(manifest.get("sources")) is not list:
            raise LocalAuthorityError("capture-manifest-invalid")
        bindings = {
            "stage_uuid": manifest["stage_uuid"],
            "manifest_sha256": _sha(manifest_bytes),
            "protocol_sha256": manifest["protocol_sha256"],
            "source_revision": manifest["source_revision"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": manifest["candidate_identity_sha256"],
            "candidate_endpoint_sha256": manifest["candidate_endpoint_sha256"],
            "source_count": len(manifest["sources"]),
            "max_owned_calls": capture.MAX_OWNED_CALLS,
            "max_health_calls": capture.MAX_HEALTH_CALLS,
            "max_scrape_calls": capture.MAX_SCRAPE_CALLS,
            "timeout_seconds": capture.REQUEST_TIMEOUT_SECONDS,
            "response_bytes": capture.MAX_RESPONSE_BYTES,
            "source_context_characters": capture.MAX_SOURCE_CHARS,
            "ca_bundle_sha256": ca_bundle_sha256,
        }
        _pinned_document(receipt_bytes, expected_receipt_sha256, scope="source-capture", bindings=bindings)
        return capture.VerifiedSourceCapturePermit(
            status="verified-admitted",
            scope="source-capture",
            **bindings,
            receipt_sha256=expected_receipt_sha256,
        )

    @staticmethod
    def protected_capture_qualification(
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
        bindings: capture.ProtectedCaptureQualificationBindings,
    ) -> capture.VerifiedProtectedCaptureQualification:
        expected_bindings = asdict(bindings)
        document = _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="protected-source-capture",
            bindings=expected_bindings,
            allow_evidence=True,
        )
        evidence = document.get("evidence")
        if type(evidence) is not dict:
            raise LocalAuthorityError("capture-qualification-evidence-missing")
        # Qualification evidence is an exact operator-pinned record. Its digest
        # is not independently treated as proof by this adapter.
        required = {
            "grok_image_digest",
            "grok_config_sha256",
            "protected_profile_sha256",
            "full_boundary_evidence_sha256",
        }
        if (
            set(evidence) != required
            or type(evidence.get("grok_image_digest")) is not str
            or not re.fullmatch(r"sha256:[0-9a-f]{64}", evidence["grok_image_digest"])
            or any(
                type(evidence.get(name)) is not str or not _SHA.fullmatch(evidence[name])
                for name in required - {"grok_image_digest"}
            )
        ):
            raise LocalAuthorityError("capture-qualification-evidence-invalid")
        return capture.VerifiedProtectedCaptureQualification(
            status="verified-qualified",
            scope="protected-source-capture",
            bindings=bindings,
            receipt_sha256=expected_receipt_sha256,
            **evidence,
        )

    @staticmethod
    def answer(
        receipt_bytes: bytes, expected_receipt_sha256: str, bindings: Mapping[str, object]
    ) -> answer.VerifiedAnswerPermit:
        document = _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="answer-execution",
            bindings=bindings,
            extra={
                "max_calls": answer.MAX_CALLS,
                "request_bytes_maximum": answer.MAX_REQUEST_BYTES,
                "response_bytes_maximum": answer.MAX_RESPONSE_BYTES,
            },
        )
        evidence = document["evidence"]
        answer_bindings = dict(bindings)
        answer_bindings["operation_ids"] = tuple(bindings["operation_ids"])
        return answer.VerifiedAnswerPermit(
            status="verified-admitted",
            **answer_bindings,
            **evidence,
            receipt_sha256=expected_receipt_sha256,
        )

    @staticmethod
    def frozen_pool_collection(
        manifest: Mapping[str, object],
        *,
        legacy_manifest_sha256: str,
        pool_input_mode: str,
        stage_clock_sha256: str,
        candidate_identity_sha256: str,
        candidate_endpoint_sha256: str,
        receipt_bytes: bytes,
        expected_receipt_sha256: str,
    ):
        stage = manifest.get("stage")
        bindings = {
            "stage_uuid": manifest.get("stage_uuid"),
            "stage_kind": stage,
            "source_revision": manifest.get("source_revision"),
            "source_closure_sha256": manifest.get("source_closure_sha256"),
            "protocol_sha256": manifest.get("protocol_sha256"),
            "cohorts_sha256": manifest.get("cohorts_sha256"),
            "input_manifest_sha256": manifest.get("input_manifest_sha256"),
            "acquisition_plan_sha256": manifest.get("acquisition_plan_sha256"),
            "legacy_manifest_sha256": legacy_manifest_sha256,
            "pool_input_mode": pool_input_mode,
            "stage_clock_sha256": stage_clock_sha256,
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": candidate_endpoint_sha256,
        }
        _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="frozen-pool-collection",
            bindings=bindings,
        )
        from scripts import coverage_frozen_pool_collect

        return coverage_frozen_pool_collect.VerifiedFrozenCollectionPermit(
            status="verified",
            scope="frozen-pool-collection",
            stage_uuid=bindings["stage_uuid"],
            stage_kind=stage,
            source_revision=bindings["source_revision"],
            source_closure_sha256=bindings["source_closure_sha256"],
            protocol_sha256=bindings["protocol_sha256"],
            cohorts_sha256=bindings["cohorts_sha256"],
            input_manifest_sha256=bindings["input_manifest_sha256"],
            acquisition_plan_sha256=bindings["acquisition_plan_sha256"],
            legacy_manifest_sha256=legacy_manifest_sha256,
            pool_input_mode=pool_input_mode,
            stage_clock_sha256=stage_clock_sha256,
            candidate_identity_sha256=candidate_identity_sha256,
            candidate_endpoint_sha256=candidate_endpoint_sha256,
            receipt_sha256=expected_receipt_sha256,
        )

    @staticmethod
    def saved_corpus_import(plan, receipt_bytes: bytes, expected_receipt_sha256: str):
        """Verify an early permit bound to this fresh stage and immutable origin package."""
        from scripts import coverage_saved_corpus as saved
        from scripts import coverage_stage_orchestration as orchestration

        origin = getattr(plan, "saved_origin", None)
        if (
            type(plan) is not orchestration.StagePlan
            or plan.pool_input_mode != "saved-corpus/1"
            or type(origin) is not saved.SavedCorpusOrigin
            or plan.stage_kind != "development"
            or type(plan.source_revision) is not str
            or saved._REVISION.fullmatch(plan.source_revision) is None
            or plan.stage_uuid in {origin.registration.stage_uuid, plan.packet_stage_uuid}
            or plan.packet_stage_uuid == origin.registration.stage_uuid
        ):
            raise LocalAuthorityError("saved-corpus-import-plan-invalid")
        try:
            protocol = _strict_json(plan.protocol_bytes)
        except Exception as exc:
            raise LocalAuthorityError("saved-corpus-import-protocol-invalid") from exc
        try:
            if type(protocol) is not dict:
                raise ValueError("protocol must be an object")
            stage_pool_input_mode = coverage_study_acquire.pool_input_mode_for_stage(protocol, plan.stage_kind)
        except (TypeError, ValueError) as exc:
            raise LocalAuthorityError("saved-corpus-import-protocol-invalid") from exc
        if (
            type(protocol) is not dict
            or stage_pool_input_mode != "saved-corpus/1"
            or _sha(plan.cohorts_bytes) != origin.registration.cohorts_sha256
            or _sha(plan.candidate_identity_bytes) != origin.registration.candidate_identity_sha256
            or plan.candidate_endpoint_sha256 != origin.registration.candidate_endpoint_sha256
        ):
            raise LocalAuthorityError("saved-corpus-import-plan-binding-mismatch")
        try:
            actual_package_sha256 = saved.package_sha256(origin.collection_directory, origin.capture_stage_directory)
        except Exception as exc:
            raise LocalAuthorityError("saved-corpus-import-origin-unavailable") from exc
        if actual_package_sha256 != origin.expected_package_sha256:
            raise LocalAuthorityError("saved-corpus-import-origin-package-mismatch")
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "packet_stage_uuid": plan.packet_stage_uuid,
            "source_revision": plan.source_revision,
            "source_closure_sha256": plan.source_closure_sha256,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
            "origin_package_sha256": actual_package_sha256,
        }
        _pinned_document(
            receipt_bytes,
            expected_receipt_sha256,
            scope="saved-corpus-import",
            bindings=bindings,
        )
        return saved.VerifiedSavedCorpusImportPermit(
            status="verified-admitted",
            scope="saved-corpus-import",
            **bindings,
            receipt_sha256=expected_receipt_sha256,
        )


class PhaseArtifactReceiptIssuer:
    """Recompute phase receipts from fixed artifacts and pinned registration context."""

    @staticmethod
    def saved_corpus_import(
        *,
        root,
        stage_uuid,
        request_id,
        request_bytes,
        expected_source_closure_sha256,
        expected_initial_registration_sha256,
        expected_active_registration_sha256,
        plan,
    ) -> bytes:
        """Issue a fresh import permit only after reopening and validating its immutable origin."""
        from scripts import coverage_saved_corpus as saved
        from scripts import coverage_stage_orchestration as orchestration

        envelope = _strict_json(request_bytes)
        origin = getattr(plan, "saved_origin", None)
        if type(plan) is not orchestration.StagePlan:
            raise LocalAuthorityError("saved-corpus-import-context-invalid")
        try:
            protocol = _strict_json(plan.protocol_bytes)
            if type(protocol) is not dict:
                raise ValueError("protocol must be an object")
            stage_pool_input_mode = coverage_study_acquire.pool_input_mode_for_stage(protocol, plan.stage_kind)
        except (TypeError, ValueError) as exc:
            raise LocalAuthorityError("saved-corpus-import-context-invalid") from exc
        if (
            type(plan) is not orchestration.StagePlan
            or plan.pool_input_mode != "saved-corpus/1"
            or stage_pool_input_mode != "saved-corpus/1"
            or type(origin) is not saved.SavedCorpusOrigin
            or plan.stage_kind != "development"
            or type(plan.source_revision) is not str
            or saved._REVISION.fullmatch(plan.source_revision) is None
            or envelope.get("scope") != "saved-corpus-import"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
            or stage_uuid != plan.stage_uuid
        ):
            raise LocalAuthorityError("saved-corpus-import-context-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="saved-corpus-import",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        clock_raw = artifacts.get("stage-clock")
        try:
            from scripts import coverage_frozen_pool_collect as collector

            clock_doc = _strict_json(clock_raw) if type(clock_raw) is bytes else None
            clock = collector.FrozenCollectionStageClock(
                **{key: value for key, value in clock_doc.items() if key != "schema"}
            )
            validated_clock = collector._validate_clock(clock, test_mode=False, test_pacer=None)
        except Exception as exc:
            raise LocalAuthorityError("saved-corpus-import-stage-clock-invalid") from exc
        if (
            validated_clock != clock_doc
            or clock.stage_started_monotonic != plan.stage_started_monotonic
            or clock.stage_deadline_monotonic != plan.stage_deadline_monotonic
            or clock.clock_origin_sha256 != collector._local_clock_origin_sha256()
        ):
            raise LocalAuthorityError("saved-corpus-import-stage-clock-mismatch")
        initial_raw = artifacts.get("initial-registration")
        initial = _strict_json(initial_raw) if type(initial_raw) is bytes else None
        active_raw = artifacts.get("active-registration")
        active = _strict_json(active_raw) if type(active_raw) is bytes else initial
        if (
            type(initial) is not dict
            or _sha(initial_raw) != expected_initial_registration_sha256
            or type(active) is not dict
            or active != initial
            or initial.get("stage_uuid") != stage_uuid
            or initial.get("source_revision") != plan.source_revision
            or initial.get("pins", {}).get("protocol") != _sha(plan.protocol_bytes)
            or initial.get("pins", {}).get("qualified_source_closure") != expected_source_closure_sha256
            or artifacts.get("protocol") != plan.protocol_bytes
            or artifacts.get("cohorts") != plan.cohorts_bytes
            or artifacts.get("acquisition-plan") != plan.acquisition_plan_bytes
            or protocol.get("cohorts_sha256") != _sha(plan.cohorts_bytes)
        ):
            raise LocalAuthorityError("saved-corpus-import-registration-mismatch")
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
            raise LocalAuthorityError("saved-corpus-import-origin-invalid") from exc
        if (
            package_sha != origin.expected_package_sha256
            or type(plan.source_revision) is not str
            or saved._REVISION.fullmatch(plan.source_revision) is None
            or _sha(plan.cohorts_bytes) != origin.registration.cohorts_sha256
            or _sha(plan.candidate_identity_bytes) != origin.registration.candidate_identity_sha256
            or plan.candidate_endpoint_sha256 != origin.registration.candidate_endpoint_sha256
        ):
            raise LocalAuthorityError("saved-corpus-import-origin-plan-mismatch")
        expected = {
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
        public = {
            key: value
            for key, value in envelope["bindings"].items()
            if key not in {"active_python_environment", "active_python_environment_sha256"}
        }
        if public != expected or artifacts.get("saved-corpus-origin-reference") != _canonical(
            {
                "schema": "coverage-saved-corpus-origin-reference/1",
                "origin_stage_uuid": origin.registration.stage_uuid,
                "origin_source_revision": origin.registration.source_revision,
                "origin_package_sha256": package_sha,
                "collection_receipt_inventory_sha256": collection_index_sha,
                "capture_inventory_sha256": capture_inventory_sha,
                "capture_manifest_sha256": capture_manifest_sha,
            }
        ):
            raise LocalAuthorityError("saved-corpus-import-request-mismatch")
        return _canonical({"schema": SCHEMA, "decision": "allow", "scope": "saved-corpus-import", "bindings": expected})

    @staticmethod
    async def _rebuild_frozen_answer_tasks(
        *,
        root,
        stage_uuid,
        request_id,
        request_bytes,
        expected_source_closure_sha256,
        expected_initial_registration_sha256,
        expected_active_registration_sha256,
        plan,
        static_materials,
        inventory_root,
        capture_receipts_root,
        selector_results_root,
        selector_archive_root,
        forbidden_stage_uuids,
        now_monotonic,
    ) -> bytes:
        """Recreate answer catalogs from the pinned corpus, captures, and selector evidence."""
        import stat
        from types import SimpleNamespace

        from scripts import coverage_assessment_packets as packets
        from scripts import coverage_frozen_pool_inputs as frozen
        from scripts import coverage_jev_execution as jev
        from scripts import coverage_late_registration as late
        from scripts import coverage_operator_handoff as handoff
        from scripts import coverage_selector_input_map as selector_map
        from scripts import coverage_stage_orchestration as orchestration
        from scripts import intent_ranking_receipts as receipts
        from scripts.coverage_operator_runner import _answer_task_artifact_bytes

        if type(plan) is not orchestration.StagePlan or plan.stage_uuid != stage_uuid:
            raise LocalAuthorityError("phase-artifact-answer-plan-invalid")
        artifacts = handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="answer-execution",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        active, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        saved_import_out: dict[str, object] = {}
        materials, pipeline, verified = PhaseArtifactReceiptIssuer.rebuild_frozen_registration_materials(
            root=root,
            stage_uuid=stage_uuid,
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
            plan=plan,
            static_materials=static_materials,
            inventory_root=inventory_root,
            now_monotonic=now_monotonic,
            scope="answer-execution",
            saved_import_out=saved_import_out,
        )

        def private_dir(path_value, *parts):
            path = Path(path_value)
            for part in parts:
                path = path / part
                info = path.lstat()
                if (
                    stat.S_ISLNK(info.st_mode)
                    or not stat.S_ISDIR(info.st_mode)
                    or stat.S_IMODE(info.st_mode) != 0o700
                    or info.st_uid != os.geteuid()
                ):
                    raise LocalAuthorityError("phase-artifact-answer-private-directory-invalid")
            return path

        capture_dir = private_dir(capture_receipts_root, stage_uuid)
        inventory_raw = frozen._read_private_regular(capture_dir, "inventory.json", "capture-inventory")
        source_manifest_raw = frozen._read_private_regular(capture_dir, "source-manifest.json", "source-manifest")
        reference = _strict_json(artifacts.get("answer-source-reference", b""))
        ref = {
            "schema": "coverage-answer-source-reference/1",
            "stage_uuid": stage_uuid,
            "capture_inventory_sha256": _sha(inventory_raw),
        }
        if (
            type(reference) is not dict
            or set(reference)
            != {"schema", "stage_uuid", "capture_inventory_sha256", "selector_terminal_inventory_sha256"}
            or any(reference.get(key) != value for key, value in ref.items())
        ):
            raise LocalAuthorityError("phase-artifact-answer-capture-reference-mismatch")
        if _sha(source_manifest_raw) != active.get("pins", {}).get("source_capture_manifest"):
            raise LocalAuthorityError("phase-artifact-answer-capture-source-pin-mismatch")
        inventory_doc = _strict_json(inventory_raw)
        if (
            type(inventory_doc) is not dict
            or inventory_doc.get("stage_uuid") != stage_uuid
            or inventory_doc.get("status") not in {"complete", "complete-with-source-failures"}
            or inventory_doc.get("source_manifest_sha256") != _sha(source_manifest_raw)
        ):
            raise LocalAuthorityError("phase-artifact-answer-capture-inventory-invalid")
        context_files = {}
        for row in inventory_doc.get("sources", []):
            if type(row) is not dict or row.get("status") != "captured":
                continue
            name = row.get("context_artifact")
            if type(name) is not str or not re.fullmatch(r"context-[0-9]{4}\.json", name):
                raise LocalAuthorityError("phase-artifact-answer-context-name-invalid")
            raw = frozen._read_private_regular(capture_dir, name, "capture-context")
            if len(raw) > 64_000:
                raise LocalAuthorityError("phase-artifact-answer-context-over-cap")
            context_files[name] = raw

        prepared_refs = packets.prepare_assessment_packets(
            packet_stage_uuid=plan.packet_stage_uuid,
            pipeline_inputs=pipeline,
            research_cases=list(plan.research_cases),
            capture_inventory_bytes=inventory_raw,
            expected_capture_inventory_sha256=_sha(inventory_raw),
            context_artifacts=context_files,
        )

        results_dir = private_dir(selector_results_root)
        archive_dir = private_dir(selector_archive_root)
        terminal_raw = receipts._read_private(
            results_dir / f"{stage_uuid}.terminal-inventory.json", max_bytes=4_000_000
        )
        if reference.get("selector_terminal_inventory_sha256") != _sha(terminal_raw):
            raise LocalAuthorityError("phase-artifact-answer-selector-reference-mismatch")
        terminal = _strict_json(terminal_raw)
        prepared = late.verify_late_registration(
            initial_registration_bytes=artifacts["initial-registration"],
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            late_registration_bytes=artifacts["active-registration"],
            expected_late_registration_sha256=expected_active_registration_sha256,
            materials=materials,
            expected_source_revision=plan.source_revision,
            expected_stage_uuid=stage_uuid,
            forbidden_stage_uuids=forbidden_stage_uuids,
            task_ids=tuple(row["task_id"] for row in plan.research_cases)
            + tuple(row["target_id"] for row in plan.navigation_targets),
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            band_companion_bytes=plan.band_companion_bytes,
            expected_band_companion_sha256=plan.band_companion_sha256,
            expected_source_pins=plan.band_companion_source_pins,
            development_receipt_bytes=plan.development_receipt_bytes,
            expected_development_receipt_sha256=plan.development_receipt_sha256,
            expected_development_stage_uuid=plan.development_stage_uuid,
        ).prepared
        operation_materials = {}
        rebuilt_map = selector_map.build_selector_input_map(
            prepared=prepared,
            task_input_manifest_bytes=materials["task_input_manifest"],
            pipeline_tasks=pipeline["tasks"],
            navigation_targets=plan.navigation_targets,
            snapshots_directory=verified.snapshots_directory,
            acquisition_snapshot_index_sha256=verified.snapshot_index_sha256,
            acquisition_manifest_bytes=plan.acquisition_manifest_bytes,
            acquisition_manifest_sha256=(
                _sha(plan.acquisition_manifest_bytes)
                if plan.pool_input_mode == "saved-corpus/1"
                else verified.pool_manifest_sha256
            ),
            neutral_fixture_bytes=(
                Path(__file__).parents[1]
                / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
            ).read_bytes(),
            frozen_v1_source_bytes=(
                Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
            ).read_bytes(),
            current_service_source_bytes=(Path(__file__).parents[1] / "slopsearx/service.py").read_bytes(),
            expected_builder_source_sha256=_sha(Path(selector_map.__file__).read_bytes()),
            operation_materials=operation_materials,
            verified_pool_source=verified,
            saved_corpus_import=saved_import_out.get("imported"),
            initial_registration_sha256=plan.initial_registration_sha256,
            band_companion_sha256=plan.band_companion_sha256,
        )
        map_doc = _strict_json(rebuilt_map)
        map_operations = map_doc.get("operations") if type(map_doc) is dict else None
        if (
            terminal.get("schema") != jev.INVENTORY_SCHEMA
            or terminal.get("stage_uuid") != stage_uuid
            or terminal.get("registration_sha256") != prepared.registration_sha256
            or terminal.get("source_revision") != plan.source_revision
            or terminal.get("source_closure_sha256") != expected_source_closure_sha256
            or terminal.get("protocol_sha256") != _sha(plan.protocol_bytes)
            or terminal.get("status") != "all-registered-calls-complete"
            or terminal.get("all_calls_complete") is not True
            or type(map_operations) is not list
        ):
            raise LocalAuthorityError("phase-artifact-answer-selector-terminal-invalid")
        map_by_id = (
            {row.get("operation_id"): row for row in map_operations if type(row) is dict}
            if type(map_operations) is list
            else {}
        )
        if len(map_by_id) != len(map_operations) or set(map_by_id) != set(prepared.operation_ids):
            raise LocalAuthorityError("phase-artifact-answer-selector-map-inventory-invalid")
        terminal_operations = terminal.get("operations")
        if (
            type(terminal_operations) is not list
            or tuple(row.get("operation_id") for row in terminal_operations) != prepared.operation_ids
            or any(row.get("state") != "complete-success" for row in terminal_operations)
        ):
            raise LocalAuthorityError("phase-artifact-answer-selector-terminal-invalid")
        observations = terminal.get("resource_observations")
        if type(observations) is not list or len(observations) != len(prepared.operation_ids):
            raise LocalAuthorityError("phase-artifact-answer-selector-observations-invalid")
        obs_by_id = {row.get("operation_id"): row for row in observations if type(row) is dict}
        if len(obs_by_id) != len(observations) or set(obs_by_id) != set(prepared.operation_ids):
            raise LocalAuthorityError("phase-artifact-answer-selector-observation-inventory-invalid")
        result_docs = {}
        for operation_id in prepared.operation_ids:
            result_raw = receipts._read_private(
                results_dir / f"{stage_uuid}.{operation_id}.result.json", max_bytes=65_536
            )
            result = _strict_json(result_raw)
            obs = obs_by_id[operation_id]
            material = operation_materials.get(operation_id)
            if type(material) is not dict:
                raise LocalAuthorityError("phase-artifact-answer-selector-input-missing")
            request_body, _compiled = jev.build_selector_request(
                operation_id=operation_id,
                operation_input_bytes=material["operation_input_bytes"],
                legacy_control=material.get("legacy_control"),
            )
            if type(result) is not dict:
                raise LocalAuthorityError("phase-artifact-answer-selector-result-invalid")
            result_checks = {
                "canonical": jev._canonical(result) == result_raw,
                "schema": result.get("schema") == jev.FINAL_RECEIPT_SCHEMA,
                "stage": result.get("stage_uuid") == stage_uuid,
                "operation": result.get("operation_id") == operation_id,
                "source": result.get("source_revision") == plan.source_revision,
                "closure": result.get("source_closure_sha256") == expected_source_closure_sha256,
                "protocol": result.get("protocol_sha256") == _sha(plan.protocol_bytes),
                "status": result.get("status") in {"selected", "fallback", "complete-original-v1"},
                "input": result.get("operation_input_sha256") == _sha(material["operation_input_bytes"]),
                "request": result.get("request_body_sha256") == _sha(request_body),
                "map_input": (
                    map_by_id[operation_id].get("operation_input_sha256") == _sha(material["operation_input_bytes"])
                ),
                "map_request": map_by_id[operation_id].get("request_body_sha256") == _sha(request_body),
                "observation_status": obs.get("status") == "observed",
                "observation_result": obs.get("result_receipt_sha256") == _sha(result_raw),
                "observation_request_bytes": obs.get("request_bytes") == len(request_body),
                "observation_response_bytes": obs.get("response_bytes") == result.get("response_bytes"),
                "observation_dispatches": obs.get("provider_dispatch_count") == result.get("provider_dispatch_count"),
                "observation_archive": obs.get("archive_receipt_sha256") == result.get("archive_receipt_sha256"),
            }
            if not all(result_checks.values()):
                failed_checks = ",".join(name for name, valid in result_checks.items() if not valid)
                raise LocalAuthorityError(f"phase-artifact-answer-selector-result-invalid:{failed_checks}")
            archive_sha = result.get("archive_receipt_sha256")
            if type(archive_sha) is not str:
                raise LocalAuthorityError("phase-artifact-answer-selector-archive-pin-missing")
            archive_bindings = {
                "stage_uuid": stage_uuid,
                "operation_id": operation_id,
                "request_body_sha256": _sha(request_body),
                "source_revision": plan.source_revision,
            }
            try:
                receipts.replay_response(
                    archive_dir, expected_bindings=archive_bindings, expected_receipt_sha256=archive_sha
                )
            except Exception as exc:
                raise LocalAuthorityError("phase-artifact-answer-selector-archive-invalid") from exc
            result_docs[operation_id] = (result, request_body)
        orders = {}
        for index, task in enumerate(pipeline["tasks"], start=1):
            task_id = task["task_id"]
            prefix = f"research-{index:02d}-base-"
            selected = {}
            for arm in ("w0", "candidate"):
                operation_id = prefix + arm
                result, request_body = result_docs[operation_id]
                result_raw = jev._canonical(result)
                obs = obs_by_id[operation_id]
                map_entry = next((row for row in map_operations if row.get("operation_id") == operation_id), None)
                material = operation_materials[operation_id]
                request_body, compiled = jev.build_selector_request(
                    operation_id=operation_id,
                    operation_input_bytes=material["operation_input_bytes"],
                    legacy_control=material.get("legacy_control"),
                )
                if (
                    result.get("operation_input_sha256") != _sha(material["operation_input_bytes"])
                    or type(map_entry) is not dict
                    or map_entry.get("operation_input_sha256") != _sha(material["operation_input_bytes"])
                    or map_entry.get("request_body_sha256") != _sha(request_body)
                ):
                    raise LocalAuthorityError("phase-artifact-answer-selector-result-invalid")
                archive_bindings = {
                    "stage_uuid": stage_uuid,
                    "operation_id": operation_id,
                    "request_body_sha256": _sha(request_body),
                    "source_revision": plan.source_revision,
                }
                archive_sha = result["archive_receipt_sha256"]
                if arm == "candidate":
                    permit = jev._canonical(
                        {
                            "schema": jev.PERMIT_SCHEMA,
                            "status": "verified-admitted",
                            "stage_uuid": stage_uuid,
                            "operation_id": operation_id,
                            "parser_mode": "coverage",
                            "source_revision": plan.source_revision,
                            "protocol_sha256": _sha(plan.protocol_bytes),
                            "source_closure_sha256": expected_source_closure_sha256,
                            "operation_input_sha256": _sha(material["operation_input_bytes"]),
                            "request_body_sha256": _sha(request_body),
                        }
                    )
                    replayed = jev.replay_candidate_response(
                        archive_root=archive_dir,
                        prepared=prepared,
                        operation_input_bytes=material["operation_input_bytes"],
                        permit_bytes=permit,
                        expected_permit_sha256=_sha(permit),
                        operation_id=operation_id,
                        bindings=archive_bindings,
                        receipt_sha256=archive_sha,
                        compiled_request=compiled,
                    )
                    order = tuple(replayed.ordered_ids)
                    if list(order) != result.get("ordered_ids"):
                        raise LocalAuthorityError("phase-artifact-answer-candidate-order-replay-mismatch")
                else:
                    legacy = material.get("legacy_control")
                    if type(legacy) is not dict:
                        raise LocalAuthorityError("phase-artifact-answer-w0-control-missing")
                    permit = jev._canonical(
                        {
                            "schema": jev.PERMIT_SCHEMA,
                            "status": "verified-admitted",
                            "stage_uuid": stage_uuid,
                            "operation_id": operation_id,
                            "parser_mode": "original-v1",
                            "source_revision": plan.source_revision,
                            "protocol_sha256": _sha(plan.protocol_bytes),
                            "source_closure_sha256": expected_source_closure_sha256,
                            "operation_input_sha256": _sha(material["operation_input_bytes"]),
                            "request_body_sha256": _sha(request_body),
                        }
                    )
                    pool = legacy["canonical_pool"]
                    _replayed, _status, legacy_receipt = await jev.replay_original_v1_response(
                        archive_root=archive_dir,
                        prepared=prepared,
                        operation_input_bytes=material["operation_input_bytes"],
                        permit_bytes=permit,
                        expected_permit_sha256=_sha(permit),
                        operation_id=operation_id,
                        bindings=archive_bindings,
                        receipt_sha256=archive_sha,
                        service=legacy["service"],
                        query=legacy["query"],
                        canonical_pool=pool,
                        frozen_v1_source_bytes=legacy["frozen_v1_source_bytes"],
                        current_service_source_bytes=legacy["current_service_source_bytes"],
                        expected_v1_request_sha256=_sha(request_body),
                    )
                    order = tuple(legacy_receipt.ordered_ids or ())
                    if list(order) != result.get("native_ordered_ids"):
                        raise LocalAuthorityError("phase-artifact-answer-w0-order-replay-mismatch")
                    legacy_control = material.get("legacy_control")
                    canonical_pool = legacy_control.get("canonical_pool") if type(legacy_control) is dict else None
                    if type(canonical_pool) not in {list, tuple}:
                        raise LocalAuthorityError("phase-artifact-answer-w0-canonical-pool-missing")
                    expected_prefix = tuple(f"c{index}" for index in range(len(canonical_pool)))
                    if (
                        len(order) != len(expected_prefix)
                        or len(set(order)) != len(order)
                        or set(order) != set(expected_prefix)
                        or not set(order).issubset(set(task["native_order"]))
                    ):
                        raise LocalAuthorityError("phase-artifact-answer-w0-prefix-invalid")
                    order += tuple(card_id for card_id in task["native_order"] if card_id not in order)
                if len(order) != len(task["native_order"]) or set(order) != set(task["native_order"]):
                    raise LocalAuthorityError("phase-artifact-answer-selector-order-membership")
                selected[arm] = order
            orders[task_id] = selected
        selector = SimpleNamespace(research_orders=orders)
        rebuilt_tasks, _bodies = orchestration._prepare_answer_tasks(
            plan,
            pipeline,
            prepared_refs,
            selector,
            inventory_doc,
        )
        rebuilt = _answer_task_artifact_bytes(stage_uuid, rebuilt_tasks)
        if type(rebuilt) is not bytes:
            raise LocalAuthorityError("phase-artifact-answer-rebuild-invalid")
        return rebuilt

    @staticmethod
    def _registered_context(
        artifacts: Mapping[str, bytes],
        *,
        stage_uuid: str,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
    ) -> tuple[dict[str, object], dict[str, object]]:
        """Require request context to match independently pinned registrations."""
        names = ("protocol", "cohorts", "source-closure", "initial-registration", "active-registration")
        raw = {name: artifacts.get(name) for name in names}
        if any(type(value) is not bytes for value in raw.values()):
            raise LocalAuthorityError("phase-artifact-registered-context-missing")
        if (
            _sha(raw["source-closure"]) != expected_source_closure_sha256
            or _sha(raw["initial-registration"]) != expected_initial_registration_sha256
            or _sha(raw["active-registration"]) != expected_active_registration_sha256
        ):
            raise LocalAuthorityError("phase-artifact-registered-context-pin-mismatch")
        protocol, cohorts, initial, active = (
            _strict_json(raw[name]) for name in ("protocol", "cohorts", "initial-registration", "active-registration")
        )
        try:
            from scripts import coverage_study_core as core

            initial = core._parse_registration(raw["initial-registration"])
            active = core._parse_registration(raw["active-registration"])
        except Exception as exc:
            raise LocalAuthorityError("phase-artifact-registration-schema-invalid") from exc
        if (
            any(type(value) is not dict for value in (protocol, cohorts, initial, active))
            or type(initial.get("pins")) is not dict
            or type(active.get("pins")) is not dict
            or initial.get("stage_uuid") != stage_uuid
            or active.get("stage_uuid") != stage_uuid
            or initial.get("source_revision") != active.get("source_revision")
            or initial["pins"].get("protocol") != _sha(raw["protocol"])
            or active["pins"].get("protocol") != _sha(raw["protocol"])
            or protocol.get("cohorts_sha256") != _sha(raw["cohorts"])
            or initial["pins"].get("qualified_source_closure") != expected_source_closure_sha256
            or active["pins"].get("qualified_source_closure") != expected_source_closure_sha256
        ):
            raise LocalAuthorityError("phase-artifact-registered-context-invalid")
        return active, protocol

    @staticmethod
    def rebuild_frozen_registration_materials(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        plan: object,
        static_materials: Mapping[str, bytes],
        inventory_root: str | os.PathLike[str],
        now_monotonic: float,
        scope: str = "late-registration",
        saved_import_out: dict[str, object] | None = None,
    ) -> tuple[dict[str, bytes], dict[str, object], object]:
        """Reopen and verify the registered frozen corpus, then rebuild late materials.

        The caller supplies an independently reconstructed, source-pinned plan
        and static materials. The only collection path is the fixed
        ``<inventory-root>/<stage_uuid>/collection`` directory; the inventory
        root is independently supplied from the validated operator config.
        """
        import stat

        from scripts import coverage_frozen_pool_collect as collector
        from scripts import coverage_frozen_pool_inputs as frozen
        from scripts import coverage_late_registration as late
        from scripts import coverage_stage_orchestration as orchestration

        if type(plan) is not orchestration.StagePlan or type(static_materials) is not dict:
            raise LocalAuthorityError("phase-artifact-frozen-plan-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope=scope,
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        active, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        if plan.pool_input_mode == "saved-corpus/1":
            from scripts import coverage_late_registration as late
            from scripts import coverage_saved_corpus as saved

            if (
                type(plan.saved_origin) is not saved.SavedCorpusOrigin
                or plan.stage_uuid != stage_uuid
                or plan.source_revision != active.get("source_revision")
                or _sha(plan.protocol_bytes) != _sha(artifacts["protocol"])
                or _sha(plan.cohorts_bytes) != _sha(artifacts["cohorts"])
                or _sha(plan.acquisition_plan_bytes) != _sha(artifacts["acquisition-plan"])
            ):
                raise LocalAuthorityError("saved-corpus-late-plan-binding-mismatch")
            for name, raw in static_materials.items():
                if artifacts.get(f"static-{name.replace('_', '-')}") != raw:
                    raise LocalAuthorityError("phase-artifact-static-material-mismatch")
            reference_raw = artifacts.get("saved-corpus-import-reference")
            reference = _strict_json(reference_raw) if type(reference_raw) is bytes else None
            request_envelope = _strict_json(request_bytes)
            request_bindings = request_envelope.get("bindings") if type(request_envelope) is dict else None
            if type(reference) is not dict or reference.get("schema") != "coverage-saved-corpus-import-reference/1":
                raise LocalAuthorityError("saved-corpus-import-reference-missing")
            if (
                type(request_bindings) is not dict
                or request_bindings.get("saved_corpus_import_receipt_sha256") != reference.get("import_receipt_sha256")
                or request_bindings.get("origin_package_sha256") != reference.get("origin_package_sha256")
                or reference.get("stage_uuid") != stage_uuid
            ):
                raise LocalAuthorityError("saved-corpus-import-binding-mismatch")
            permit_base = Path(root) / stage_uuid / "saved-corpus-import"
            try:
                receipt_raw = coverage_operator_handoff._read_private(
                    permit_base / "responses" / "permit.receipt", 16_384
                )
                pin_raw = coverage_operator_handoff._read_private(permit_base / "pins" / "permit.sha256", 256)
                permit_sha = pin_raw.decode("ascii").strip()
            except Exception as exc:
                raise LocalAuthorityError("saved-corpus-import-permit-missing") from exc
            permit = PinnedReceiptAuthorities.saved_corpus_import(plan, receipt_raw, permit_sha)
            binding = saved.SavedCorpusStageBinding(
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
            )
            imported = saved.verify_and_import_saved_corpus(
                plan.saved_origin, binding, plan.research_cases, plan.navigation_targets
            )
            if saved_import_out is not None:
                if type(saved_import_out) is not dict:
                    raise LocalAuthorityError("saved-corpus-import-output-invalid")
                saved_import_out["imported"] = imported
            if (
                late.saved_corpus_import_reference(imported, plan=plan) != reference_raw
                or reference.get("import_receipt_sha256")
                != artifacts.get("saved-corpus-import-receipt-sha256", b"").decode()
                or artifacts.get("saved-corpus-import-receipt-sha256") != imported.import_receipt_sha256.encode()
                or artifacts.get("capture-manifest") != imported.capture_manifest_bytes
            ):
                raise LocalAuthorityError("saved-corpus-import-reconstruction-mismatch")
            materials = late.build_postacquisition_materials(
                plan=plan,
                acquisition_evidence=None,
                pipeline_inputs=imported.pipeline_inputs,
                static_materials=static_materials,
                saved_corpus_import=imported,
            )
            if scope == "late-registration":
                for name, raw in materials.items():
                    if artifacts.get(f"material-{name.replace('_', '-')}") != raw:
                        raise LocalAuthorityError(f"phase-artifact-late-material-recomputation-mismatch:{name}")
            return materials, imported.pipeline_inputs, imported.origin_pool_verified
        if (
            plan.stage_uuid != stage_uuid
            or plan.source_closure_sha256 != expected_source_closure_sha256
            or plan.pool_input_mode != "frozen-pool/1"
            or plan.source_revision != active.get("source_revision")
            or _sha(plan.protocol_bytes) != _sha(artifacts["protocol"])
            or _sha(plan.cohorts_bytes) != _sha(artifacts["cohorts"])
            or _sha(plan.acquisition_plan_bytes) != _sha(artifacts["acquisition-plan"])
        ):
            raise LocalAuthorityError("phase-artifact-frozen-plan-binding-mismatch")
        for name, raw in static_materials.items():
            artifact_name = f"static-{name.replace('_', '-')}"
            if type(raw) is not bytes or artifacts.get(artifact_name) != raw:
                raise LocalAuthorityError("phase-artifact-static-material-mismatch")

        clock_raw = artifacts.get("stage-clock")
        clock_doc = _strict_json(clock_raw) if type(clock_raw) is bytes else None
        if (
            type(clock_doc) is not dict
            or set(clock_doc)
            != {
                "schema",
                "stage_started_monotonic",
                "stage_deadline_monotonic",
                "stage_started_utc",
                "stage_deadline_utc",
                "clock_origin_sha256",
            }
            or clock_doc.get("schema") != "coverage-frozen-collection-stage-clock/1"
        ):
            raise LocalAuthorityError("phase-artifact-stage-clock-invalid")
        clock = collector.FrozenCollectionStageClock(
            **{key: value for key, value in clock_doc.items() if key != "schema"}
        )
        if (
            clock.stage_started_monotonic != plan.stage_started_monotonic
            or clock.stage_deadline_monotonic != plan.stage_deadline_monotonic
            or clock.clock_origin_sha256 != collector._local_clock_origin_sha256()
        ):
            raise LocalAuthorityError("phase-artifact-stage-clock-plan-mismatch")
        inventory = Path(inventory_root)
        collection = inventory / stage_uuid / "collection"
        try:
            inventory_stat = inventory.lstat()
            stage_stat = (inventory / stage_uuid).lstat()
            collection_stat = collection.lstat()
        except OSError as exc:
            raise LocalAuthorityError("phase-artifact-frozen-inventory-missing") from exc
        if (
            any(
                stat.S_ISLNK(value.st_mode) or not stat.S_ISDIR(value.st_mode)
                for value in (inventory_stat, stage_stat, collection_stat)
            )
            or any(
                value.st_uid != getattr(os, "geteuid", lambda: value.st_uid)()
                for value in (inventory_stat, stage_stat, collection_stat)
            )
            or stat.S_IMODE(inventory_stat.st_mode) != 0o700
            or stat.S_IMODE(stage_stat.st_mode) != 0o700
            or stat.S_IMODE(collection_stat.st_mode) != 0o700
        ):
            raise LocalAuthorityError("phase-artifact-frozen-inventory-not-private")
        evidence_raw = frozen._read_private_regular(collection, "frozen-pool-evidence.json", "pool-evidence")
        receipt_raw = frozen._read_private_regular(collection, "collector-receipt.json", "collector-receipt")
        availability_raw = frozen._read_private_regular(collection, "engine-availability.json", "availability")
        reference_raw = artifacts.get("frozen-pool-reference")
        reference = _strict_json(reference_raw) if type(reference_raw) is bytes else None
        evidence = frozen.FrozenPoolEvidence(
            snapshots_directory=collection,
            evidence_bytes=evidence_raw,
            expected_evidence_sha256=_sha(evidence_raw),
            collector_receipt_bytes=receipt_raw,
            expected_collector_receipt_sha256=_sha(receipt_raw),
            availability_bytes=availability_raw,
            expected_availability_sha256=_sha(availability_raw),
        )
        verified = frozen.verify_frozen_pool_evidence(
            evidence,
            expected_stage_uuid=stage_uuid,
            expected_stage_kind=plan.stage_kind,
            expected_source_revision=plan.source_revision,
            expected_source_closure_sha256=plan.source_closure_sha256,
            expected_protocol_sha256=_sha(plan.protocol_bytes),
            expected_cohorts_sha256=_sha(plan.cohorts_bytes),
            expected_candidate_identity_sha256=_sha(plan.candidate_identity_bytes),
            expected_candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
            expected_acquisition_plan_sha256=_sha(plan.acquisition_plan_bytes),
            expected_task_ids=tuple(row["task_id"] for row in plan.research_cases),
            expected_navigation_ids=tuple(row["target_id"] for row in plan.navigation_targets),
            stage_started_monotonic=clock.stage_started_monotonic,
            stage_deadline_monotonic=clock.stage_deadline_monotonic,
            now_monotonic=now_monotonic,
        )
        if type(reference) is not dict or reference != {
            "schema": "coverage-frozen-pool-reference/1",
            "stage_uuid": stage_uuid,
            "source_revision": plan.source_revision,
            "source_closure_sha256": plan.source_closure_sha256,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "pool_manifest_sha256": verified.pool_manifest_sha256,
            "snapshot_index_sha256": verified.snapshot_index_sha256,
            "evidence_sha256": verified.evidence_sha256,
            "availability_sha256": verified.availability_sha256,
            "collector_receipt_sha256": verified.collector_receipt_sha256,
            "proof_sha256": verified.proof_sha256,
            "relative_inventory": "collection",
        }:
            raise LocalAuthorityError("phase-artifact-frozen-reference-mismatch")
        pipeline_inputs = frozen.build_pipeline_inputs_from_frozen(
            list(plan.research_cases),
            evidence=evidence,
            verified=verified,
            expected_stage_uuid=stage_uuid,
            expected_source_revision=plan.source_revision,
            expected_protocol_sha256=_sha(plan.protocol_bytes),
            expected_cohorts_sha256=_sha(plan.cohorts_bytes),
            expected_candidate_identity_sha256=_sha(plan.candidate_identity_bytes),
            expected_candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
        )
        materials = late.build_postacquisition_materials(
            plan=plan,
            acquisition_evidence=None,
            pipeline_inputs=pipeline_inputs,
            static_materials=static_materials,
            verified_pool_source=verified,
        )
        if scope == "late-registration":
            for name, raw in materials.items():
                if artifacts.get(f"material-{name.replace('_', '-')}") != raw:
                    raise LocalAuthorityError(f"phase-artifact-late-material-recomputation-mismatch:{name}")
        return materials, pipeline_inputs, verified

    @staticmethod
    def late_registration(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        plan: object,
        static_materials: Mapping[str, bytes],
        inventory_root: str | os.PathLike[str],
        forbidden_stage_uuids: tuple[str, ...],
        now_monotonic: float,
    ) -> bytes:
        """Create the same late-registration bytes only after frozen-source rebuild and preflight."""
        from scripts import coverage_late_registration as late

        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="late-registration",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        initial_raw = artifacts.get("initial-registration")
        if type(initial_raw) is not bytes or _sha(initial_raw) != expected_initial_registration_sha256:
            raise LocalAuthorityError("phase-artifact-initial-registration-pin-mismatch")
        initial = _strict_json(initial_raw)
        if type(initial) is not dict or type(initial.get("pins")) is not dict:
            raise LocalAuthorityError("phase-artifact-initial-registration-invalid")
        materials, _pipeline, _verified = PhaseArtifactReceiptIssuer.rebuild_frozen_registration_materials(
            root=root,
            stage_uuid=stage_uuid,
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
            plan=plan,
            static_materials=static_materials,
            inventory_root=inventory_root,
            now_monotonic=now_monotonic,
        )
        bindings = _strict_json(request_bytes)["bindings"]
        expected_material_hashes = {name: _sha(materials[name]) for name in sorted(late._LATE_MATERIALS)}
        development_receipt = artifacts.get("development-pass-receipt")
        if plan.stage_kind == "confirmation":
            if (
                type(plan.development_receipt_bytes) is not bytes
                or development_receipt != plan.development_receipt_bytes
                or bindings.get("development_receipt_sha256") != plan.development_receipt_sha256
                or bindings.get("development_stage_uuid") != plan.development_stage_uuid
            ):
                raise LocalAuthorityError("phase-artifact-development-receipt-mismatch")
        elif development_receipt is not None or any(
            key in bindings for key in ("development_receipt_sha256", "development_stage_uuid")
        ):
            raise LocalAuthorityError("phase-artifact-development-receipt-unexpected")
        if (
            bindings.get("initial_registration_sha256") != expected_initial_registration_sha256
            or bindings.get("stage_uuid") != stage_uuid
            or bindings.get("source_revision") != plan.source_revision
            or bindings.get("source_closure_sha256") != expected_source_closure_sha256
            or bindings.get("protocol_sha256") != _sha(plan.protocol_bytes)
            or bindings.get("cohorts_sha256") != _sha(plan.cohorts_bytes)
            or bindings.get("acquisition_plan_sha256") != _sha(plan.acquisition_plan_bytes)
            or bindings.get("late_material_sha256") != expected_material_hashes
        ):
            raise LocalAuthorityError("phase-artifact-late-registration-binding-mismatch")
        late_doc = dict(initial)
        pins = dict(initial["pins"])
        for name in late._LATE_MATERIALS:
            pins[name] = _sha(materials[name])
        late_doc["pins"] = pins
        receipt = _canonical(late_doc)
        task_ids = tuple(row["task_id"] for row in plan.research_cases) + tuple(
            row["target_id"] for row in plan.navigation_targets
        )
        late.verify_late_registration(
            initial_registration_bytes=initial_raw,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            late_registration_bytes=receipt,
            expected_late_registration_sha256=_sha(receipt),
            materials=materials,
            expected_source_revision=plan.source_revision,
            expected_stage_uuid=stage_uuid,
            forbidden_stage_uuids=forbidden_stage_uuids,
            task_ids=task_ids,
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            band_companion_bytes=plan.band_companion_bytes,
            expected_band_companion_sha256=plan.band_companion_sha256,
            expected_source_pins=plan.band_companion_source_pins,
            development_receipt_bytes=plan.development_receipt_bytes,
            expected_development_receipt_sha256=plan.development_receipt_sha256,
            expected_development_stage_uuid=plan.development_stage_uuid,
        )
        return receipt

    @staticmethod
    def selector_map_admission(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        prepared: object,
        expected_selector_map_bytes: bytes,
        plan: object | None = None,
    ) -> bytes:
        """Admit only the independently rebuilt, source-bound registered selector map."""
        from scripts import coverage_selector_admission as admission
        from scripts import coverage_study_core as core

        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "selector-map-admission"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
            or type(prepared) is not core.PreparedStage
            or prepared.fresh_execution_authorized is not False
            or prepared.selector_input_map_schema != "coverage-selector-input-map/2-registered"
            or type(expected_selector_map_bytes) is not bytes
        ):
            raise LocalAuthorityError("phase-artifact-selector-map-context-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="selector-map-admission",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        registration, protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        proof_context = _verified_development_receipt_context(plan, registration, envelope.get("bindings"), artifacts)
        map_bytes = artifacts.get("selector-map")
        if (
            map_bytes != expected_selector_map_bytes
            or prepared.stage_uuid != stage_uuid
            or prepared.registration_sha256 != expected_active_registration_sha256
            or prepared.source_revision != registration.get("source_revision")
            or prepared.pins.get("protocol") != _sha(artifacts["protocol"])
            or prepared.pins.get("qualified_source_closure") != expected_source_closure_sha256
            or protocol.get("selector_input_map_schema") != prepared.selector_input_map_schema
            or protocol.get("selector_input_map_status") != "registered"
        ):
            raise LocalAuthorityError("phase-artifact-selector-map-source-mismatch")
        bindings = envelope.get("bindings")
        public = (
            {
                key: value
                for key, value in bindings.items()
                if key not in {"active_python_environment", "active_python_environment_sha256", *proof_context.keys()}
            }
            if type(bindings) is dict
            else None
        )
        expected = {
            "stage_uuid": stage_uuid,
            "source_revision": registration["source_revision"],
            "protocol_sha256": _sha(artifacts["protocol"]),
            "registration_sha256": expected_active_registration_sha256,
            "source_closure_sha256": expected_source_closure_sha256,
            "selector_input_map_sha256": _sha(map_bytes),
        }
        receipt_bindings = dict(expected)
        if plan is not None and getattr(plan, "pool_input_mode", None) == "saved-corpus/1":
            # The coordinator adds these two public bindings only for a saved
            # development import. Recompute them from the pinned import permit
            # and immutable origin package; do not trust matching request and
            # artifact hashes by themselves.
            from scripts import coverage_late_registration as late
            from scripts import coverage_saved_corpus as saved
            from scripts import coverage_stage_orchestration as orchestration

            origin = getattr(plan, "saved_origin", None)
            if (
                type(plan) is not orchestration.StagePlan
                or plan.stage_kind != "development"
                or plan.stage_uuid != stage_uuid
                or type(origin) is not saved.SavedCorpusOrigin
            ):
                raise LocalAuthorityError("saved-corpus-selector-map-context-invalid")
            try:
                permit_root = Path(root) / stage_uuid / "saved-corpus-import"
                handoff = coverage_operator_handoff
                permit_raw = handoff._read_private(permit_root / "responses" / "permit.receipt", 16_384)
                permit_pin_raw = handoff._read_private(permit_root / "pins" / "permit.sha256", 256)
                permit_pin = permit_pin_raw.decode("ascii").strip()
                permit = PinnedReceiptAuthorities.saved_corpus_import(plan, permit_raw, permit_pin)
                binding = saved.SavedCorpusStageBinding(
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
                )
                imported = saved.verify_and_import_saved_corpus(
                    origin, binding, plan.research_cases, plan.navigation_targets
                )
                expected_reference = late.saved_corpus_import_reference(imported, plan=plan)
            except Exception as exc:
                raise LocalAuthorityError("saved-corpus-selector-map-import-invalid") from exc
            reference = artifacts.get("saved-corpus-import-reference")
            import_receipt_pin = artifacts.get("saved-corpus-import-receipt-sha256")
            if (
                reference != expected_reference
                or import_receipt_pin != imported.import_receipt_sha256.encode("ascii")
                or artifacts.get("capture-manifest") != imported.capture_manifest_bytes
            ):
                raise LocalAuthorityError("saved-corpus-selector-map-import-binding-mismatch")
            expected.update(
                {
                    "origin_package_sha256": imported.origin_package_sha256,
                    "saved_corpus_import_receipt_sha256": imported.import_receipt_sha256,
                }
            )
        if public != expected:
            raise LocalAuthorityError("phase-artifact-selector-map-request-binding-mismatch")
        receipt = _canonical(
            {
                "schema": admission.SCHEMA,
                "status": "admitted",
                **receipt_bindings,
            }
        )
        admission.verify_selector_map_admission(
            receipt_bytes=receipt,
            expected_receipt_sha256=_sha(receipt),
            protocol_bytes=artifacts["protocol"],
            prepared=prepared,
            selector_input_map_sha256=_sha(map_bytes),
        )
        return receipt

    @staticmethod
    def selector_operation_permits(
        *,
        root: str | os.PathLike[str],
        inventory_root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        plan: object,
        static_materials: Mapping[str, bytes],
        forbidden_stage_uuids: tuple[str, ...],
        now_monotonic: float,
    ) -> bytes:
        """Rebuild the registered map from the sealed pool before issuing its one-shot permit batch."""
        from scripts import coverage_jev_execution as jev
        from scripts import coverage_late_registration as late
        from scripts import coverage_selector_input_map as selector_map

        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "selector-operation-permits"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
            or plan.stage_uuid != stage_uuid
            or plan.source_closure_sha256 != expected_source_closure_sha256
        ):
            raise LocalAuthorityError("phase-artifact-selector-batch-context-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="selector-operation-permits",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        active, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        saved_import_out: dict[str, object] = {}
        materials, pipeline_inputs, verified = PhaseArtifactReceiptIssuer.rebuild_frozen_registration_materials(
            root=root,
            stage_uuid=stage_uuid,
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
            plan=plan,
            static_materials=static_materials,
            inventory_root=inventory_root,
            now_monotonic=now_monotonic,
            scope="selector-operation-permits",
            saved_import_out=saved_import_out,
        )
        initial_raw = artifacts.get("initial-registration")
        active_raw = artifacts.get("active-registration")
        if type(initial_raw) is not bytes or type(active_raw) is not bytes:
            raise LocalAuthorityError("phase-artifact-selector-registration-missing")
        late_verified = late.verify_late_registration(
            initial_registration_bytes=initial_raw,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            late_registration_bytes=active_raw,
            expected_late_registration_sha256=expected_active_registration_sha256,
            materials=materials,
            expected_source_revision=plan.source_revision,
            expected_stage_uuid=stage_uuid,
            forbidden_stage_uuids=forbidden_stage_uuids,
            task_ids=tuple(row["task_id"] for row in plan.research_cases)
            + tuple(row["target_id"] for row in plan.navigation_targets),
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            band_companion_bytes=plan.band_companion_bytes,
            expected_band_companion_sha256=plan.band_companion_sha256,
            expected_source_pins=plan.band_companion_source_pins,
            development_receipt_bytes=plan.development_receipt_bytes,
            expected_development_receipt_sha256=plan.development_receipt_sha256,
            expected_development_stage_uuid=plan.development_stage_uuid,
        )
        prepared = late_verified.prepared
        task_manifest = artifacts.get("task-input-manifest")
        acquisition_manifest = artifacts.get("acquisition-manifest")
        stage_clock_raw = artifacts.get("stage-clock")
        if (
            type(task_manifest) is not bytes
            or _sha(task_manifest) != active.get("pins", {}).get("task_input_manifest")
            or acquisition_manifest != plan.acquisition_manifest_bytes
            or type(stage_clock_raw) is not bytes
        ):
            raise LocalAuthorityError("phase-artifact-selector-dynamic-material-mismatch")
        repository_root = Path(__file__).parents[1]
        neutral_fixture = (
            repository_root / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
        ).read_bytes()
        # The selector's W0 replay uses the frozen historical source artifact,
        # not the current production-rerank module pinned by the stage config.
        # Its path is fixed by the checked-out source revision.
        frozen_source = (
            repository_root / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
        ).read_bytes()
        current_service_source = (repository_root / "slopsearx/service.py").read_bytes()
        operation_materials: dict[str, dict[str, object]] = {}
        rebuilt_map = selector_map.build_selector_input_map(
            prepared=prepared,
            task_input_manifest_bytes=task_manifest,
            pipeline_tasks=pipeline_inputs["tasks"],
            navigation_targets=plan.navigation_targets,
            snapshots_directory=verified.snapshots_directory,
            acquisition_snapshot_index_sha256=verified.snapshot_index_sha256,
            acquisition_manifest_bytes=plan.acquisition_manifest_bytes,
            acquisition_manifest_sha256=(
                _sha(plan.acquisition_manifest_bytes)
                if plan.pool_input_mode == "saved-corpus/1"
                else verified.pool_manifest_sha256
            ),
            neutral_fixture_bytes=neutral_fixture,
            frozen_v1_source_bytes=frozen_source,
            current_service_source_bytes=current_service_source,
            expected_builder_source_sha256=_sha(Path(selector_map.__file__).read_bytes()),
            operation_materials=operation_materials,
            verified_pool_source=verified,
            saved_corpus_import=saved_import_out.get("imported"),
            initial_registration_sha256=plan.initial_registration_sha256,
            band_companion_sha256=plan.band_companion_sha256,
        )
        map_raw = artifacts.get("selector-map")
        map_doc = _strict_json(map_raw) if type(map_raw) is bytes else None
        artifact_index_raw = artifacts.get("selector-artifact-index")
        artifact_index = _strict_json(artifact_index_raw) if type(artifact_index_raw) is bytes else None
        if (
            rebuilt_map != map_raw
            or _sha(rebuilt_map) != envelope.get("bindings", {}).get("selector_input_map_sha256")
            or type(map_doc) is not dict
            or type(map_doc.get("operations")) is not list
            or type(artifact_index) is not dict
            or set(artifact_index) != {"schema", "operations", "legacy_controls"}
            or artifact_index.get("schema") != "coverage-selector-artifact-index/1"
        ):
            raise LocalAuthorityError("phase-artifact-selector-map-rebuild-mismatch")
        input_rows = artifact_index.get("operations")
        legacy_rows = artifact_index.get("legacy_controls")
        if type(input_rows) is not list or type(legacy_rows) is not list:
            raise LocalAuthorityError("phase-artifact-selector-artifact-index-invalid")
        expected_inputs = []
        expected_legacy = []
        for operation_id in prepared.operation_ids:
            material = operation_materials.get(operation_id)
            if type(material) is not dict or type(material.get("operation_input_bytes")) is not bytes:
                raise LocalAuthorityError("phase-artifact-selector-operation-material-missing")
            suffix = hashlib.sha256(operation_id.encode()).hexdigest()[:16]
            input_name = f"input-{suffix}"
            if artifacts.get(input_name) != material["operation_input_bytes"]:
                raise LocalAuthorityError("phase-artifact-selector-operation-input-mismatch")
            expected_inputs.append({"operation_id": operation_id, "input_artifact": input_name})
            legacy = material.get("legacy_control")
            if legacy is not None:
                if (
                    artifacts.get("frozen-v1-source") != frozen_source
                    or artifacts.get("current-service-source") != current_service_source
                ):
                    raise LocalAuthorityError("phase-artifact-selector-legacy-source-mismatch")
                expected_legacy.append(
                    {
                        "operation_id": operation_id,
                        "query": legacy["query"],
                        "pool_reference": "frozen-pool-index",
                    }
                )

        def exact_rows(actual, expected, key):
            if any(type(row) is not dict for row in actual + expected):
                return False
            actual_map = {row.get(key): row for row in actual}
            expected_map = {row.get(key): row for row in expected}
            return len(actual_map) == len(actual) and len(expected_map) == len(expected) and actual_map == expected_map

        if not exact_rows(input_rows, expected_inputs, "operation_id"):
            raise LocalAuthorityError("phase-artifact-selector-operation-input-inventory-mismatch")
        if not exact_rows(legacy_rows, expected_legacy, "operation_id"):
            raise LocalAuthorityError("phase-artifact-selector-legacy-inventory-mismatch")
        bindings = envelope.get("bindings")
        if (
            type(bindings) is not dict
            or bindings.get("stage_uuid") != stage_uuid
            or bindings.get("registration_sha256") != expected_active_registration_sha256
            or bindings.get("selector_input_map_sha256") != _sha(rebuilt_map)
            or bindings.get("operations") != map_doc["operations"]
            or active.get("stage_uuid") != stage_uuid
        ):
            raise LocalAuthorityError("phase-artifact-selector-batch-request-mismatch")
        permit_rows = []
        map_entries = {row["operation_id"]: row for row in map_doc["operations"]}
        if set(map_entries) != set(prepared.operation_ids):
            raise LocalAuthorityError("phase-artifact-selector-operation-inventory-mismatch")
        for operation_id in prepared.operation_ids:
            entry = map_entries[operation_id]
            parser_mode = entry.get("parser_mode")
            permit = jev._canonical(
                {
                    "schema": jev.PERMIT_SCHEMA,
                    "status": "verified-admitted",
                    "stage_uuid": stage_uuid,
                    "operation_id": operation_id,
                    "parser_mode": parser_mode,
                    "source_revision": plan.source_revision,
                    "protocol_sha256": active["pins"]["protocol"],
                    "source_closure_sha256": expected_source_closure_sha256,
                    "operation_input_sha256": entry["operation_input_sha256"],
                    "request_body_sha256": entry["request_body_sha256"],
                }
            )
            jev._permit(
                permit,
                _sha(permit),
                prepared=prepared,
                operation_id=operation_id,
                operation_input_sha256=entry["operation_input_sha256"],
                request_sha256=entry["request_body_sha256"],
                parser_mode=parser_mode,
            )
            permit_rows.append(
                {
                    "operation_id": operation_id,
                    "permit_base64": base64.b64encode(permit).decode("ascii"),
                    "permit_sha256": _sha(permit),
                }
            )
        receipt = _canonical(
            {
                "schema": "coverage-selector-permit-batch/1",
                "stage_uuid": stage_uuid,
                "selector_input_map_sha256": _sha(rebuilt_map),
                "operations": permit_rows,
            }
        )
        if len(receipt) > 64_000:
            raise LocalAuthorityError("phase-artifact-selector-permit-batch-over-cap")
        return receipt

    @staticmethod
    def frozen_pool_collection(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        plan: object,
    ) -> bytes:
        """Issue only a frozen-collection receipt rebuilt from the registered plan and same-host clock."""
        from scripts import coverage_frozen_pool_collect as collector

        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "frozen-pool-collection"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
            or plan.stage_uuid != stage_uuid
            or plan.pool_input_mode != "frozen-pool/1"
        ):
            raise LocalAuthorityError("phase-artifact-frozen-permit-context-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="frozen-pool-collection",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        registration, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        manifest_raw = artifacts.get("acquisition-manifest")
        clock_raw = artifacts.get("stage-clock")
        if (
            type(manifest_raw) is not bytes
            or manifest_raw != plan.acquisition_manifest_bytes
            or type(clock_raw) is not bytes
        ):
            raise LocalAuthorityError("phase-artifact-frozen-permit-material-missing")
        manifest = _strict_json(manifest_raw)
        clock_doc = _strict_json(clock_raw)
        if type(manifest) is not dict or type(clock_doc) is not dict:
            raise LocalAuthorityError("phase-artifact-frozen-permit-document-invalid")
        try:
            from scripts import coverage_live_acquire as live

            if live._canonical(manifest) != manifest_raw:
                raise ValueError
            clock = collector.FrozenCollectionStageClock(
                **{key: value for key, value in clock_doc.items() if key != "schema"}
            )
            checked_clock = collector._validate_clock(clock, test_mode=False, test_pacer=None)
        except Exception as exc:
            raise LocalAuthorityError("phase-artifact-frozen-permit-validation-failed") from exc
        if (
            clock.stage_started_monotonic != plan.stage_started_monotonic
            or clock.stage_deadline_monotonic != plan.stage_deadline_monotonic
            or clock.clock_origin_sha256 != collector._local_clock_origin_sha256()
            or checked_clock != clock_doc
            or manifest.get("stage_uuid") != stage_uuid
            or manifest.get("stage") != plan.stage_kind
            or manifest.get("source_revision") != plan.source_revision
            or manifest.get("source_closure_sha256") != expected_source_closure_sha256
            or manifest.get("protocol_sha256") != _sha(plan.protocol_bytes)
            or manifest.get("cohorts_sha256") != _sha(plan.cohorts_bytes)
            or manifest.get("acquisition_plan_sha256") != _sha(plan.acquisition_plan_bytes)
            or registration.get("source_revision") != plan.source_revision
        ):
            raise LocalAuthorityError("phase-artifact-frozen-permit-plan-mismatch")
        proof_context = _verified_development_receipt_context(plan, registration, envelope.get("bindings"), artifacts)
        public = {
            key: value
            for key, value in envelope["bindings"].items()
            if key not in {"active_python_environment", "active_python_environment_sha256", *proof_context.keys()}
        }
        expected = {
            "stage_uuid": stage_uuid,
            "stage_kind": plan.stage_kind,
            "source_revision": plan.source_revision,
            "source_closure_sha256": expected_source_closure_sha256,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "input_manifest_sha256": manifest.get("input_manifest_sha256"),
            "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
            "legacy_manifest_sha256": plan.expected_acquisition_manifest_sha256,
            "pool_input_mode": "frozen-pool/1",
            "stage_clock_sha256": _sha(clock_raw),
            "candidate_identity_sha256": _sha(plan.candidate_identity_bytes),
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        }
        if public != expected:
            raise LocalAuthorityError("phase-artifact-frozen-permit-request-mismatch")
        receipt = _canonical(
            {"schema": SCHEMA, "decision": "allow", "scope": "frozen-pool-collection", "bindings": expected}
        )
        PinnedReceiptAuthorities.frozen_pool_collection(
            manifest,
            legacy_manifest_sha256=expected["legacy_manifest_sha256"],
            pool_input_mode=expected["pool_input_mode"],
            stage_clock_sha256=expected["stage_clock_sha256"],
            candidate_identity_sha256=expected["candidate_identity_sha256"],
            candidate_endpoint_sha256=expected["candidate_endpoint_sha256"],
            receipt_bytes=receipt,
            expected_receipt_sha256=_sha(receipt),
        )
        return receipt

    @staticmethod
    def source_capture(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        expected_capture_manifest_bytes: bytes,
        plan: object | None = None,
    ) -> bytes:
        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "source-capture"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
        ):
            raise LocalAuthorityError("phase-artifact-request-binding-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="source-capture",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        registration, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        manifest_bytes = artifacts.get("capture-manifest")
        if type(manifest_bytes) is not bytes or manifest_bytes != expected_capture_manifest_bytes:
            raise LocalAuthorityError("phase-artifact-capture-manifest-missing")
        active_pins = registration.get("pins") if type(registration) is dict else None
        if type(active_pins) is not dict or active_pins.get("source_capture_manifest") != _sha(manifest_bytes):
            raise LocalAuthorityError("phase-artifact-capture-manifest-registration-pin-mismatch")
        bindings = envelope["bindings"]
        proof_context = _verified_development_receipt_context(plan, registration, bindings, artifacts)
        manifest = _strict_json(manifest_bytes)
        if type(manifest) is not dict or _sha(manifest_bytes) != bindings.get("manifest_sha256"):
            raise LocalAuthorityError("phase-artifact-capture-manifest-mismatch")
        sources = manifest.get("sources")
        expected = {
            "stage_uuid": manifest.get("stage_uuid"),
            "manifest_sha256": _sha(manifest_bytes),
            "protocol_sha256": manifest.get("protocol_sha256"),
            "source_revision": manifest.get("source_revision"),
            "cohorts_sha256": manifest.get("cohorts_sha256"),
            "candidate_identity_sha256": manifest.get("candidate_identity_sha256"),
            "candidate_endpoint_sha256": manifest.get("candidate_endpoint_sha256"),
            "source_count": len(sources) if type(sources) is list else None,
            "max_owned_calls": capture.MAX_OWNED_CALLS,
            "max_health_calls": capture.MAX_HEALTH_CALLS,
            "max_scrape_calls": capture.MAX_SCRAPE_CALLS,
            "timeout_seconds": capture.REQUEST_TIMEOUT_SECONDS,
            "response_bytes": capture.MAX_RESPONSE_BYTES,
            "source_context_characters": capture.MAX_SOURCE_CHARS,
            "ca_bundle_sha256": bindings.get("ca_bundle_sha256"),
        }
        public = {
            key: value
            for key, value in bindings.items()
            if key
            not in {
                "active_python_environment",
                "active_python_environment_sha256",
                "source_closure_sha256",
                *proof_context.keys(),
            }
        }
        protocol_raw = artifacts.get("protocol", b"")
        cohorts_raw = artifacts.get("cohorts", b"")
        closure_raw = artifacts.get("source-closure", b"")
        if (
            public != expected
            or bindings.get("stage_uuid") != stage_uuid
            or _sha(protocol_raw) != manifest.get("protocol_sha256")
            or _sha(cohorts_raw) != manifest.get("cohorts_sha256")
            or _sha(closure_raw) != expected_source_closure_sha256
            or registration.get("source_revision") != manifest.get("source_revision")
        ):
            raise LocalAuthorityError("phase-artifact-source-capture-binding-mismatch")
        receipt = _canonical({"schema": SCHEMA, "decision": "allow", "scope": "source-capture", "bindings": public})
        PinnedReceiptAuthorities.source_capture(
            manifest_bytes, receipt, _sha(receipt), ca_bundle_sha256=public["ca_bundle_sha256"]
        )
        return receipt

    @staticmethod
    def protected_capture_qualification(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        candidate_base_url: str,
        ca_bundle_sha256: str | None,
        qualification_receipt_bytes: bytes,
        expected_qualification_receipt_sha256: str,
        plan: object | None = None,
    ) -> bytes:
        """Validate the protected-profile receipt against the registered capture request."""
        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "protected-source-capture"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
        ):
            raise LocalAuthorityError("phase-artifact-request-binding-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="protected-source-capture",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        registration, _protocol = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        proof_context = _verified_development_receipt_context(plan, registration, envelope.get("bindings"), artifacts)
        manifest_bytes = artifacts.get("capture-manifest")
        identity_bytes = artifacts.get("candidate-identity")
        if type(manifest_bytes) is not bytes or type(identity_bytes) is not bytes:
            raise LocalAuthorityError("phase-artifact-capture-material-missing")
        manifest = _strict_json(manifest_bytes)
        identity = capture._validate_candidate_identity(identity_bytes, _sha(identity_bytes))
        try:
            _health_url, _scrape_url, endpoint_sha = capture._candidate_endpoints(candidate_base_url)
        except capture.SourceCaptureError as exc:
            raise LocalAuthorityError("phase-artifact-candidate-endpoint-invalid") from exc
        protocol_bytes = artifacts.get("protocol")
        cohorts_bytes = artifacts.get("cohorts")
        closure_bytes = artifacts.get("source-closure")
        pins = registration.get("pins") if type(registration) is dict else None
        if (
            type(manifest) is not dict
            or type(pins) is not dict
            or pins.get("source_capture_manifest") != _sha(manifest_bytes)
            or manifest.get("stage_uuid") != stage_uuid
            or manifest.get("source_revision") != registration.get("source_revision")
            or manifest.get("protocol_sha256") != _sha(protocol_bytes)
            or manifest.get("cohorts_sha256") != _sha(cohorts_bytes)
            or manifest.get("candidate_identity_sha256") != _sha(identity_bytes)
            or manifest.get("candidate_endpoint_sha256") != endpoint_sha
            or type(manifest.get("sources")) is not list
            or _sha(closure_bytes) != expected_source_closure_sha256
        ):
            raise LocalAuthorityError("phase-artifact-capture-registration-mismatch")
        bindings = capture.ProtectedCaptureQualificationBindings(
            stage_uuid=stage_uuid,
            source_revision=registration["source_revision"],
            protocol_sha256=_sha(protocol_bytes),
            candidate_identity_sha256=_sha(identity_bytes),
            candidate_endpoint_sha256=endpoint_sha,
            candidate_endpoint_scheme="https",
            candidate_runtime_revision=identity["runtime"]["revision"],
            capture_module_sha256=_sha(Path(capture.__file__).read_bytes()),
            ca_bundle_sha256=ca_bundle_sha256,
        )
        _phase_request_context_bindings(
            envelope.get("bindings"),
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_scope_bindings=asdict(bindings),
            expected_context_bindings=proof_context,
        )
        PinnedReceiptAuthorities.protected_capture_qualification(
            qualification_receipt_bytes,
            expected_qualification_receipt_sha256,
            bindings,
        )
        return qualification_receipt_bytes

    @staticmethod
    async def answer_execution(
        *,
        root: str | os.PathLike[str],
        stage_uuid: str,
        request_id: str,
        request_bytes: bytes,
        expected_source_closure_sha256: str,
        expected_initial_registration_sha256: str,
        expected_active_registration_sha256: str,
        endpoint: str,
        allow_trusted_private_http: bool,
        expected_task_material_bytes: bytes,
        forbidden_stage_uuids: tuple[str, ...] | None = None,
        plan: object | None = None,
        static_materials: Mapping[str, bytes] | None = None,
        inventory_root: str | os.PathLike[str] | None = None,
        capture_receipts_root: str | os.PathLike[str] | None = None,
        selector_results_root: str | os.PathLike[str] | None = None,
        selector_archive_root: str | os.PathLike[str] | None = None,
        now_monotonic: float | None = None,
    ) -> bytes:
        envelope = _strict_json(request_bytes)
        if (
            type(envelope) is not dict
            or envelope.get("scope") != "answer-execution"
            or envelope.get("stage_uuid") != stage_uuid
            or envelope.get("request_id") != request_id
        ):
            raise LocalAuthorityError("phase-artifact-request-binding-invalid")
        artifacts = coverage_operator_handoff.load_request_artifacts(
            root=root,
            stage_uuid=stage_uuid,
            scope="answer-execution",
            request_id=request_id,
            request_bytes=request_bytes,
            expected_source_closure_sha256=expected_source_closure_sha256,
        )
        registration_context, _protocol_context = PhaseArtifactReceiptIssuer._registered_context(
            artifacts,
            stage_uuid=stage_uuid,
            expected_source_closure_sha256=expected_source_closure_sha256,
            expected_initial_registration_sha256=expected_initial_registration_sha256,
            expected_active_registration_sha256=expected_active_registration_sha256,
        )
        proof_context = _verified_development_receipt_context(
            plan, registration_context, envelope.get("bindings"), artifacts
        )
        protocol_doc = _strict_json(artifacts.get("protocol", b""))
        frozen_pipeline = None
        if type(protocol_doc) is dict and protocol_doc.get("pool_input_mode") == "frozen-pool/1":
            from scripts import coverage_assessment_packets as packets

            required = (
                plan,
                static_materials,
                inventory_root,
                capture_receipts_root,
                selector_results_root,
                selector_archive_root,
                forbidden_stage_uuids,
                now_monotonic,
            )
            if any(value is None for value in required):
                raise LocalAuthorityError("phase-artifact-answer-source-context-required")
            _materials, frozen_pipeline, _verified = PhaseArtifactReceiptIssuer.rebuild_frozen_registration_materials(
                root=root,
                stage_uuid=stage_uuid,
                request_id=request_id,
                request_bytes=request_bytes,
                expected_source_closure_sha256=expected_source_closure_sha256,
                expected_initial_registration_sha256=expected_initial_registration_sha256,
                expected_active_registration_sha256=expected_active_registration_sha256,
                plan=plan,
                static_materials=static_materials,
                inventory_root=inventory_root,
                now_monotonic=now_monotonic,
                scope="answer-execution",
            )
            rebuilt_task_bytes = await PhaseArtifactReceiptIssuer._rebuild_frozen_answer_tasks(
                root=root,
                stage_uuid=stage_uuid,
                request_id=request_id,
                request_bytes=request_bytes,
                expected_source_closure_sha256=expected_source_closure_sha256,
                expected_initial_registration_sha256=expected_initial_registration_sha256,
                expected_active_registration_sha256=expected_active_registration_sha256,
                plan=plan,
                static_materials=static_materials,
                inventory_root=inventory_root,
                capture_receipts_root=capture_receipts_root,
                selector_results_root=selector_results_root,
                selector_archive_root=selector_archive_root,
                forbidden_stage_uuids=forbidden_stage_uuids,
                now_monotonic=now_monotonic,
            )
            if (
                artifacts.get("answer-task-material") != rebuilt_task_bytes
                or expected_task_material_bytes != rebuilt_task_bytes
            ):
                raise LocalAuthorityError("phase-artifact-answer-source-rebuild-mismatch")
        task_raw = artifacts.get("answer-task-material")
        manifest_raw = artifacts.get("answer-request-manifest")
        if type(task_raw) is not bytes or type(manifest_raw) is not bytes or task_raw != expected_task_material_bytes:
            raise LocalAuthorityError("phase-artifact-answer-material-missing")
        task_doc = _strict_json(task_raw)
        if (
            type(task_doc) is not dict
            or set(task_doc) != {"schema", "stage_uuid", "tasks"}
            or task_doc.get("schema") != "coverage-answer-task-material/1"
            or task_doc.get("stage_uuid") != stage_uuid
            or type(task_doc.get("tasks")) is not list
            or not task_doc["tasks"]
        ):
            raise LocalAuthorityError("phase-artifact-answer-material-invalid")
        if frozen_pipeline is not None:
            input_manifest_raw = artifacts.get("task-input-manifest")
            if type(input_manifest_raw) is not bytes or _sha(input_manifest_raw) != registration_context.get(
                "pins", {}
            ).get("task_input_manifest"):
                raise LocalAuthorityError("phase-artifact-answer-input-manifest-pin-mismatch")
            input_manifest = _strict_json(input_manifest_raw)
            research_rows = input_manifest.get("research_tasks") if type(input_manifest) is dict else None
            task_rows = task_doc["tasks"]
            if type(research_rows) is not list or len(research_rows) != 8 or len(task_rows) != 8:
                raise LocalAuthorityError("phase-artifact-answer-task-inventory-mismatch")
            cohort_doc = _strict_json(artifacts.get("cohorts", b""))
            stage_kind = registration_context.get("stage_kind")
            stage_rows = cohort_doc.get("stages") if type(cohort_doc) is dict else None
            matching_stages = (
                [row for row in stage_rows if type(row) is dict and row.get("stage") == stage_kind]
                if type(stage_rows) is list
                else []
            )
            if len(matching_stages) != 1 or type(matching_stages[0].get("research_cases")) is not list:
                raise LocalAuthorityError("phase-artifact-answer-cohort-stage-invalid")
            cohort_cases = matching_stages[0]["research_cases"]
            cases_by_id = {row.get("task_id"): row for row in cohort_cases if type(row) is dict}
            if len(cases_by_id) != 8 or len(cohort_cases) != 8:
                raise LocalAuthorityError("phase-artifact-answer-cohort-inventory-invalid")
            pipeline_tasks_by_id = {row.get("task_id"): row for row in frozen_pipeline["tasks"] if type(row) is dict}
            for registered_row, task_row in zip(research_rows, task_rows, strict=True):
                if (
                    type(registered_row) is not dict
                    or type(task_row) is not dict
                    or registered_row.get("task_id") != task_row.get("task_id")
                    or registered_row.get("query") != task_row.get("query")
                    or registered_row.get("purpose") != task_row.get("purpose")
                ):
                    raise LocalAuthorityError("phase-artifact-answer-task-source-mismatch")
                case = cases_by_id.get(task_row.get("task_id"))
                pipeline_task = pipeline_tasks_by_id.get(task_row.get("task_id"))
                try:
                    _facets, expected_checks = packets._facet_and_check_contract(case, pipeline_task)
                except Exception as exc:
                    raise LocalAuthorityError("phase-artifact-answer-registered-checks-invalid") from exc
                checks_match = {
                    "case": type(case) is dict,
                    "kind": task_row.get("kind") == "research",
                    "query": type(case) is dict and task_row.get("query") == case.get("search_query"),
                    "purpose": type(pipeline_task) is dict and task_row.get("purpose") == pipeline_task.get("purpose"),
                    "check_count": len(expected_checks) == 4,
                    "checks": task_row.get("critical_checks") == list(expected_checks),
                }
                if not all(checks_match.values()):
                    raise LocalAuthorityError(
                        "phase-artifact-answer-registered-checks-mismatch:"
                        + ",".join(name for name, valid in checks_match.items() if not valid)
                    )
        tasks = []
        try:
            for row in task_doc["tasks"]:
                if type(row) is not dict or set(row) != {
                    "task_id",
                    "kind",
                    "query",
                    "purpose",
                    "critical_checks",
                    "answers",
                    "catalog_base64",
                    "catalog_sha256",
                }:
                    raise ValueError
                answers = []
                for item in row["answers"]:
                    if type(item) is not dict or set(item) != {
                        "arm",
                        "body_base64",
                        "body_sha256",
                        "delivered_source_ids",
                        "delivered_evidence_ids",
                    }:
                        raise ValueError
                    body = base64.b64decode(item["body_base64"], validate=True)
                    if _sha(body) != item["body_sha256"]:
                        raise ValueError
                    answers.append(
                        answer.consumer.AnswerInput(
                            arm=item["arm"],
                            body=body,
                            body_sha256=item["body_sha256"],
                            delivered_source_ids=tuple(item["delivered_source_ids"]),
                            delivered_evidence_ids=tuple(item["delivered_evidence_ids"]),
                        )
                    )
                catalog = base64.b64decode(row["catalog_base64"], validate=True)
                if _sha(catalog) != row["catalog_sha256"]:
                    raise ValueError
                tasks.append(
                    answer.AnswerTask(
                        task_id=row["task_id"],
                        kind=row["kind"],
                        query=row["query"],
                        purpose=row["purpose"],
                        critical_checks=tuple(row["critical_checks"]),
                        answers=tuple(answers),
                        catalog_bytes=catalog,
                        catalog_sha256=row["catalog_sha256"],
                    )
                )
        except Exception as exc:
            raise LocalAuthorityError("phase-artifact-answer-task-invalid") from exc
        requests = answer._make_requests(tasks)
        manifest, manifest_sha = answer._request_manifest(
            requests, endpoint_security_mode=envelope["bindings"].get("endpoint_security_mode")
        )
        if manifest != manifest_raw:
            raise LocalAuthorityError("phase-artifact-answer-manifest-recomputation-mismatch")
        bindings = envelope["bindings"]
        registration = _strict_json(artifacts.get("active-registration", b""))
        source_closure = _strict_json(artifacts.get("source-closure", b""))
        protocol = _strict_json(artifacts.get("protocol", b""))
        cohorts = _strict_json(artifacts.get("cohorts", b""))
        if (
            type(registration) is not dict
            or type(registration.get("pins")) is not dict
            or type(source_closure) is not dict
            or type(protocol) is not dict
            or type(cohorts) is not dict
            or registration.get("stage_uuid") != stage_uuid
            or registration != registration_context
            or registration.get("pins", {}).get("protocol") != _sha(artifacts["protocol"])
            or protocol.get("cohorts_sha256") != _sha(artifacts["cohorts"])
            or registration.get("pins", {}).get("qualified_source_closure") != expected_source_closure_sha256
            or _sha(artifacts["source-closure"]) != expected_source_closure_sha256
            or source_closure.get("source_revision") != registration.get("source_revision")
        ):
            raise LocalAuthorityError("phase-artifact-answer-registration-mismatch")
        if manifest_sha != bindings.get("operation_manifest_sha256"):
            raise LocalAuthorityError("phase-artifact-answer-manifest-pin-mismatch")
        endpoint_url, mode = answer._chat_url(endpoint, allow_trusted_private_http=allow_trusted_private_http)
        if mode == "trusted-private-http":
            _dial_url, _host_header, resolved_sha = await answer._resolve_private_http_destination(endpoint_url)
        else:
            resolved_sha = answer._sha(answer._canonical({"mode": "https-required"}))
        endpoint_sha = answer._sha(
            answer._canonical(
                {
                    "endpoint": endpoint,
                    "security_mode": mode,
                    "resolved_destination_sha256": resolved_sha,
                }
            )
        )
        proof_context = _verified_development_receipt_context(plan, registration, bindings, artifacts)
        public = {
            key: value
            for key, value in bindings.items()
            if key
            not in {
                "active_python_environment",
                "active_python_environment_sha256",
                "source_closure_sha256",
                *proof_context.keys(),
            }
        }
        expected = {
            "stage_uuid": stage_uuid,
            "source_revision": registration["source_revision"],
            "protocol_sha256": _sha(artifacts["protocol"]),
            "cohorts_sha256": _sha(artifacts["cohorts"]),
            "operation_manifest_sha256": manifest_sha,
            "endpoint_sha256": endpoint_sha,
            "resolved_destination_sha256": resolved_sha,
            "endpoint_security_mode": mode,
            "operation_ids": [row.operation_id for row in requests],
        }
        if public != expected:
            raise LocalAuthorityError("phase-artifact-answer-binding-mismatch")
        evidence = {
            "max_calls": answer.MAX_CALLS,
            "request_bytes_maximum": answer.MAX_REQUEST_BYTES,
            "response_bytes_maximum": answer.MAX_RESPONSE_BYTES,
        }
        receipt = _canonical(
            {
                "schema": SCHEMA,
                "decision": "allow",
                "scope": "answer-execution",
                "bindings": public,
                "evidence": evidence,
            }
        )
        PinnedReceiptAuthorities.answer(receipt, _sha(receipt), public)
        return receipt


class AcquisitionReceiptVerifier:
    """Fixed verifier adapter accepted by the acquisition executor."""

    def verify(self, manifest, manifest_sha256, receipt_bytes, expected_receipt_sha256):
        return PinnedReceiptAuthorities.acquisition(manifest, manifest_sha256, receipt_bytes, expected_receipt_sha256)


class SourceCaptureReceiptVerifier:
    """Fixed verifier adapter accepted by the source-capture executor."""

    def verify(self, manifest_bytes, receipt_bytes, expected_receipt_sha256, ca_bundle_sha256=None):
        return PinnedReceiptAuthorities.source_capture(
            manifest_bytes, receipt_bytes, expected_receipt_sha256, ca_bundle_sha256
        )


class ProtectedCaptureReceiptVerifier:
    """Fixed verifier adapter for externally pinned capture qualification."""

    def verify(self, receipt_bytes, expected_receipt_sha256, bindings):
        if type(bindings) is not capture.ProtectedCaptureQualificationBindings:
            raise LocalAuthorityError("capture-qualification-bindings-invalid")
        return PinnedReceiptAuthorities.protected_capture_qualification(
            receipt_bytes, expected_receipt_sha256, bindings
        )


class AnswerReceiptVerifier:
    """Fixed verifier adapter accepted by the paired-answer executor."""

    def verify(self, receipt_bytes, expected_receipt_sha256, bindings):
        return PinnedReceiptAuthorities.answer(receipt_bytes, expected_receipt_sha256, bindings)


class FrozenPoolCollectionReceiptVerifier:
    """Fixed adapter for a separately pinned, one-shot frozen-pool permit."""

    def __init__(
        self,
        *,
        pool_input_mode: str,
        stage_clock_sha256: str,
        candidate_identity_sha256: str,
        candidate_endpoint_sha256: str,
    ) -> None:
        self.pool_input_mode = pool_input_mode
        self.stage_clock_sha256 = stage_clock_sha256
        self.candidate_identity_sha256 = candidate_identity_sha256
        self.candidate_endpoint_sha256 = candidate_endpoint_sha256

    def verify_collection(
        self,
        manifest,
        manifest_sha256,
        pool_input_mode,
        stage_clock_sha256,
        candidate_identity_sha256,
        candidate_endpoint_sha256,
        receipt_bytes,
        expected_receipt_sha256,
    ):
        expected = (
            self.pool_input_mode,
            self.stage_clock_sha256,
            self.candidate_identity_sha256,
            self.candidate_endpoint_sha256,
        )
        actual = (
            pool_input_mode,
            stage_clock_sha256,
            candidate_identity_sha256,
            candidate_endpoint_sha256,
        )
        if actual != expected or pool_input_mode != "frozen-pool/1":
            raise LocalAuthorityError("frozen-collection-context-mismatch")
        return PinnedReceiptAuthorities.frozen_pool_collection(
            manifest,
            legacy_manifest_sha256=manifest_sha256,
            pool_input_mode=pool_input_mode,
            stage_clock_sha256=stage_clock_sha256,
            candidate_identity_sha256=candidate_identity_sha256,
            candidate_endpoint_sha256=candidate_endpoint_sha256,
            receipt_bytes=receipt_bytes,
            expected_receipt_sha256=expected_receipt_sha256,
        )


class SavedCorpusImportReceiptVerifier:
    """Fixed verifier adapter for a fresh, source-bound saved-corpus permit."""

    def verify(self, plan, receipt_bytes, expected_receipt_sha256):
        return PinnedReceiptAuthorities.saved_corpus_import(plan, receipt_bytes, expected_receipt_sha256)


class FileOneShotLease:
    """Durable O_EXCL local consumption of an already pinned permit."""

    def __init__(self, root: str | os.PathLike[str], *, scope: str) -> None:
        if type(scope) is not str or not re.fullmatch(r"[a-z][a-z0-9-]{0,47}", scope):
            raise LocalAuthorityError("lease-scope-invalid")
        self.root = Path(root)
        self.scope = scope
        try:
            self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
            info = self.root.lstat()
        except OSError as exc:
            raise LocalAuthorityError("lease-root-unavailable") from exc
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o700
            or info.st_uid != getattr(os, "geteuid", lambda: info.st_uid)()
        ):
            raise LocalAuthorityError("lease-root-not-private")
        self._root_identity = (info.st_dev, info.st_ino)

    def consume_once(
        self,
        permit: object,
        *,
        operation_id: str | None = None,
        request_sha256: str | None = None,
    ) -> dict[str, str]:
        stage_uuid = getattr(permit, "stage_uuid", None)
        permit_sha = getattr(permit, "receipt_sha256", None)
        if type(permit) is dict:
            stage_uuid = permit.get("stage_uuid", stage_uuid)
            permit_sha = permit.get("receipt_sha256", permit_sha)
        try:
            if type(stage_uuid) is not str or str(uuid.UUID(stage_uuid)) != stage_uuid:
                raise ValueError
        except (TypeError, ValueError) as exc:
            raise LocalAuthorityError("lease-stage-uuid-invalid") from exc
        if type(permit_sha) is not str or not _SHA.fullmatch(permit_sha):
            raise LocalAuthorityError("lease-permit-digest-invalid")
        if operation_id is not None and (
            type(operation_id) is not str or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}", operation_id)
        ):
            raise LocalAuthorityError("lease-operation-id-invalid")
        if request_sha256 is not None and (type(request_sha256) is not str or not _SHA.fullmatch(request_sha256)):
            raise LocalAuthorityError("lease-request-digest-invalid")
        claims = {
            "schema": "coverage-local-one-shot-lease/1",
            "scope": self.scope,
            "status": "consumed",
            "stage_uuid": stage_uuid,
            "operation_id": operation_id,
            "request_sha256": request_sha256,
            "permit_sha256": permit_sha,
        }
        raw = _canonical(claims)
        # The logical slot is the identity. Binding the filename to the request
        # or permit digest would let a changed request claim a fresh lease.
        name_key = _sha(_canonical([self.scope, stage_uuid, operation_id]))
        filename = f"{name_key}.lease.json"
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        try:
            directory_fd = os.open(
                self.root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
            )
        except OSError as exc:
            raise LocalAuthorityError("lease-root-changed") from exc
        try:
            metadata = os.fstat(directory_fd)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or (metadata.st_dev, metadata.st_ino) != self._root_identity
                or stat.S_IMODE(metadata.st_mode) != 0o700
                or metadata.st_uid != getattr(os, "geteuid", lambda: metadata.st_uid)()
            ):
                raise LocalAuthorityError("lease-root-changed")
            try:
                descriptor = os.open(filename, flags, 0o600, dir_fd=directory_fd)
            except FileExistsError as exc:
                raise LocalAuthorityError("lease-already-consumed") from exc
            try:
                with os.fdopen(descriptor, "wb", closefd=False) as stream:
                    stream.write(raw)
                    stream.flush()
                    os.fsync(stream.fileno())
            finally:
                os.close(descriptor)
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
        return {"status": "consumed", "stage_uuid": stage_uuid, "receipt_sha256": _sha(raw)}
