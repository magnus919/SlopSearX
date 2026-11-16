"""One-shot external receipt issuer. No search, capture, Jev or model calls."""

import argparse
import asyncio
import dataclasses
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import stat
import subprocess
import time


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _require_confirmation_receipt(plan):
    if (
        type(getattr(plan, "development_receipt_bytes", None)) is not bytes
        or type(getattr(plan, "development_receipt_sha256", None)) is not str
        or type(getattr(plan, "development_stage_uuid", None)) is not str
        or sha(plan.development_receipt_bytes) != plan.development_receipt_sha256
    ):
        raise ValueError("confirmation-development-receipt-required")


def read(path, cap=8_000_000):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_size > cap
        ):
            raise ValueError("private-file-invalid")
        raw = os.read(fd, cap + 1)
        if len(raw) > cap:
            raise ValueError("private-file-over-cap")
        return raw
    finally:
        os.close(fd)


def write(path, raw):
    from scripts.coverage_atomic_io import write_new as atomic_write_new

    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = path.parent.lstat()
    if not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700 or info.st_uid != os.geteuid():
        raise ValueError("private-parent-invalid")
    atomic_write_new(path, raw)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=pathlib.Path, required=True)
    p.add_argument("--source-revision", required=True)
    p.add_argument("--config", type=pathlib.Path, required=True)
    p.add_argument("--config-sha256", required=True)
    p.add_argument(
        "--scope",
        choices=[
            "saved-corpus-import",
            "frozen-pool-collection",
            "late-registration",
            "protected-source-capture",
            "source-capture",
            "selector-map-admission",
            "selector-operation-permits",
            "answer-execution",
        ],
        required=True,
    )
    p.add_argument("--request-id", default="permit")
    p.add_argument("--active-registration", type=pathlib.Path)
    p.add_argument("--active-registration-sha256")
    p.add_argument("--qualification-evidence", type=pathlib.Path)
    p.add_argument("--qualification-evidence-sha256")
    p.add_argument("--preflight-only", action="store_true")
    a = p.parse_args()
    source = a.source.resolve(strict=True)
    if subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip() != a.source_revision:
        raise ValueError("source-revision-mismatch")
    if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True).strip():
        raise ValueError("source-not-clean")
    spec = importlib.util.spec_from_file_location("fresh_launcher", source / "scripts/coverage_operator_launcher.py")
    launcher = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(launcher)
    cache = launcher.configure_source_import()
    try:
        from scripts import coverage_frozen_pool_collect as collector
        from scripts import coverage_late_registration as late
        from scripts import coverage_local_authority as authority
        from scripts import coverage_operator_handoff as handoff
        from scripts import coverage_operator_runner as runner_mod
        from scripts import coverage_selector_input_map as selector_map
        from scripts import coverage_source_capture as capture

        runner = runner_mod.OperatorStageRunner.preflight_from_file(a.config, a.config_sha256)
        if runner.plan.source_revision != a.source_revision:
            raise ValueError("config-source-mismatch")
        if a.preflight_only:
            print(
                json.dumps({"status": "issuer-static-preflight-passed", "scientific_calls": 0, "receipt_issued": False})
            )
            return
        if runner.plan.stage_kind == "confirmation":
            _require_confirmation_receipt(runner.plan)
        initial_pin = runner.initial_registration_sha256
        active_pin = initial_pin
        if a.active_registration is not None:
            raw = read(a.active_registration)
            if not a.active_registration_sha256 or sha(raw) != a.active_registration_sha256:
                raise ValueError("active-pin-mismatch")
            active_pin = a.active_registration_sha256
        elif a.scope not in {"saved-corpus-import", "frozen-pool-collection", "late-registration"}:
            raise ValueError("active-registration-required")
        root = pathlib.Path(runner.directories["operator_handoff"])
        stage = runner.plan.stage_uuid
        request_path = root / stage / a.scope / "requests" / (a.request_id + ".json")
        base = request_path.parent.parent
        if (base / "responses" / (a.request_id + ".receipt")).exists() or (
            base / "pins" / (a.request_id + ".sha256")
        ).exists():
            raise ValueError("receipt-already-issued")
        request = read(request_path)
        envelope = authority._strict_json(request)
        if envelope.get("bindings", {}).get("active_python_environment_sha256") != sha(
            runner.active_environment_receipt_bytes
        ) or envelope.get("bindings", {}).get("active_python_environment") != json.loads(
            runner.active_environment_receipt_bytes
        ):
            raise ValueError("active-environment-mismatch")
        common = dict(
            root=root,
            stage_uuid=stage,
            request_id=a.request_id,
            request_bytes=request,
            expected_source_closure_sha256=runner.plan.source_closure_sha256,
            expected_initial_registration_sha256=initial_pin,
            expected_active_registration_sha256=active_pin,
        )
        artifacts = handoff.load_request_artifacts(
            scope=a.scope,
            **{
                k: v
                for k, v in common.items()
                if k not in {"expected_initial_registration_sha256", "expected_active_registration_sha256"}
            },
        )
        request_bindings = envelope.get("bindings")
        if runner.plan.stage_kind == "confirmation":
            if (
                type(request_bindings) is not dict
                or artifacts.get("development-pass-receipt") != runner.plan.development_receipt_bytes
                or request_bindings.get("development_receipt_sha256") != runner.plan.development_receipt_sha256
                or request_bindings.get("development_stage_uuid") != runner.plan.development_stage_uuid
            ):
                raise ValueError("development-pass-receipt-request-mismatch")
        elif "development-pass-receipt" in artifacts or (
            type(request_bindings) is dict
            and any(key in request_bindings for key in ("development_receipt_sha256", "development_stage_uuid"))
        ):
            raise ValueError("development-pass-receipt-unexpected")
        clock_doc = authority._strict_json(artifacts["stage-clock"])
        clock = collector.FrozenCollectionStageClock(**{k: v for k, v in clock_doc.items() if k != "schema"})
        collector._validate_clock(clock, test_mode=False, test_pacer=None)
        plan = dataclasses.replace(
            runner.plan,
            stage_started_monotonic=clock.stage_started_monotonic,
            stage_deadline_monotonic=clock.stage_deadline_monotonic,
        )
        if time.monotonic() >= plan.stage_deadline_monotonic:
            raise ValueError("stage-expired")
        if a.scope == "saved-corpus-import":
            if active_pin != initial_pin or plan.pool_input_mode != "saved-corpus/1":
                raise ValueError("saved-import-requires-initial-registration")
            receipt = authority.PhaseArtifactReceiptIssuer.saved_corpus_import(**common, plan=plan)
            base = root / stage / a.scope
            write(base / "responses" / (a.request_id + ".receipt"), receipt)
            write(base / "pins" / (a.request_id + ".sha256"), sha(receipt).encode())
            print(
                json.dumps(
                    {"scope": a.scope, "stage_uuid": stage, "receipt_sha256": sha(receipt), "scientific_calls": 0}
                )
            )
            return
        forbidden = tuple(runner.config["forbidden_stage_uuids"])
        rebuild = dict(
            plan=plan,
            static_materials=runner.static_materials,
            inventory_root=runner.directories["stage_inventory"],
            now_monotonic=time.monotonic(),
        )
        issuer = authority.PhaseArtifactReceiptIssuer
        if a.scope == "frozen-pool-collection":
            receipt = issuer.frozen_pool_collection(**common, plan=plan)
        elif a.scope == "late-registration":
            receipt = issuer.late_registration(**common, **rebuild, forbidden_stage_uuids=forbidden)
        elif a.scope == "selector-operation-permits":
            receipt = issuer.selector_operation_permits(**common, **rebuild, forbidden_stage_uuids=forbidden)
        elif a.scope in {"source-capture", "protected-source-capture", "selector-map-admission"}:
            saved_import_out = {}
            materials, pipeline, verified = issuer.rebuild_frozen_registration_materials(
                **common, **rebuild, scope=a.scope, saved_import_out=saved_import_out
            )
            if a.scope == "source-capture":
                receipt = issuer.source_capture(
                    **common,
                    expected_capture_manifest_bytes=pipeline["capture_manifest_bytes"],
                    plan=plan,
                )
            elif a.scope == "protected-source-capture":
                if a.qualification_evidence is None or not a.qualification_evidence_sha256:
                    raise ValueError("qualified-proof-required")
                proof = read(a.qualification_evidence, 16384)
                if sha(proof) != a.qualification_evidence_sha256:
                    raise ValueError("qualification-evidence-pin-mismatch")
                evidence = authority._strict_json(proof).get("evidence")
                if type(evidence) is not dict:
                    raise ValueError("qualification-evidence-invalid")
                endpoint = runner.config["candidate_base_url"]
                _, _, endpoint_sha = capture._candidate_endpoints(endpoint)
                ca_path = runner.private_paths.get("capture_ca_bundle")
                ca_sha = sha(read(pathlib.Path(ca_path), 262144)) if ca_path else None
                expected = dataclasses.asdict(
                    capture.ProtectedCaptureQualificationBindings(
                        stage_uuid=stage,
                        source_revision=plan.source_revision,
                        protocol_sha256=sha(plan.protocol_bytes),
                        candidate_identity_sha256=sha(plan.candidate_identity_bytes),
                        candidate_endpoint_sha256=endpoint_sha,
                        candidate_endpoint_scheme="https",
                        candidate_runtime_revision=runner.candidate_identity["runtime"]["revision"],
                        capture_module_sha256=sha(pathlib.Path(capture.__file__).read_bytes()),
                        ca_bundle_sha256=ca_sha,
                    )
                )
                qualified = authority._canonical(
                    {
                        "schema": authority.SCHEMA,
                        "decision": "allow",
                        "scope": a.scope,
                        "bindings": expected,
                        "evidence": evidence,
                    }
                )
                receipt = issuer.protected_capture_qualification(
                    **common,
                    candidate_base_url=endpoint,
                    ca_bundle_sha256=ca_sha,
                    qualification_receipt_bytes=qualified,
                    expected_qualification_receipt_sha256=sha(qualified),
                    plan=plan,
                )
            else:
                active_raw = read(a.active_registration)
                prepared = late.verify_late_registration(
                    initial_registration_bytes=runner.initial_registration,
                    expected_initial_registration_sha256=initial_pin,
                    late_registration_bytes=active_raw,
                    expected_late_registration_sha256=active_pin,
                    materials=materials,
                    expected_source_revision=plan.source_revision,
                    expected_stage_uuid=stage,
                    forbidden_stage_uuids=forbidden,
                    task_ids=tuple(row["task_id"] for row in plan.research_cases)
                    + tuple(row["target_id"] for row in plan.navigation_targets),
                    navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
                    band_companion_bytes=plan.band_companion_bytes,
                    expected_band_companion_sha256=plan.band_companion_sha256,
                    expected_source_pins=plan.band_companion_source_pins,
                    development_receipt_bytes=getattr(plan, "development_receipt_bytes", None),
                    expected_development_receipt_sha256=getattr(plan, "development_receipt_sha256", None),
                    expected_development_stage_uuid=getattr(plan, "development_stage_uuid", None),
                ).prepared
                map_bytes = selector_map.build_selector_input_map(
                    prepared=prepared,
                    task_input_manifest_bytes=materials["task_input_manifest"],
                    pipeline_tasks=pipeline["tasks"],
                    navigation_targets=plan.navigation_targets,
                    snapshots_directory=verified.snapshots_directory,
                    acquisition_snapshot_index_sha256=verified.snapshot_index_sha256,
                    acquisition_manifest_bytes=plan.acquisition_manifest_bytes,
                    acquisition_manifest_sha256=(
                        sha(plan.acquisition_manifest_bytes)
                        if plan.pool_input_mode == "saved-corpus/1"
                        else verified.pool_manifest_sha256
                    ),
                    verified_pool_source=verified,
                    saved_corpus_import=saved_import_out.get("imported"),
                    initial_registration_sha256=plan.initial_registration_sha256,
                    band_companion_sha256=plan.band_companion_sha256,
                    neutral_fixture_bytes=(
                        source
                        / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
                    ).read_bytes(),
                    frozen_v1_source_bytes=(
                        source / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
                    ).read_bytes(),
                    current_service_source_bytes=(source / "slopsearx/service.py").read_bytes(),
                    expected_builder_source_sha256=sha(pathlib.Path(selector_map.__file__).read_bytes()),
                )
                receipt = issuer.selector_map_admission(
                    **common, prepared=prepared, expected_selector_map_bytes=map_bytes, plan=plan
                )
        else:
            receipt = asyncio.run(
                issuer.answer_execution(
                    **common,
                    **rebuild,
                    endpoint=runner.config["answer_endpoint"],
                    allow_trusted_private_http=runner.config.get("allow_trusted_private_http", False),
                    expected_task_material_bytes=artifacts["answer-task-material"],
                    forbidden_stage_uuids=forbidden,
                    capture_receipts_root=runner.directories["capture_receipts"],
                    selector_results_root=runner.directories["selector_results"],
                    selector_archive_root=runner.directories["selector_archive"],
                )
            )
        write(base / "responses" / (a.request_id + ".receipt"), receipt)
        write(base / "pins" / (a.request_id + ".sha256"), (sha(receipt) + "\n").encode())
        print(
            json.dumps({"scope": a.scope, "stage_uuid": stage, "receipt_sha256": sha(receipt), "scientific_calls": 0})
        )
    finally:
        shutil.rmtree(cache, ignore_errors=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(json.dumps({"status": "issuer-refused", "error_class": type(exc).__name__}))
        raise SystemExit(1)
