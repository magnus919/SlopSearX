from __future__ import annotations

import hashlib
import uuid
from dataclasses import asdict
from pathlib import Path

import pytest

from scripts import coverage_frozen_pool_inputs as frozen
from scripts import coverage_operator_runner as runner
from scripts import coverage_saved_corpus as saved


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _private_dir(path: Path) -> Path:
    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def _private_file(path: Path, raw: bytes, mode: int = 0o600) -> Path:
    path.write_bytes(raw)
    path.chmod(mode)
    return path


def _origin(tmp_path: Path, plan: bytes, *, sidecar: Path | None) -> saved.SavedCorpusOrigin:
    collection = _private_dir(tmp_path / "collection")
    capture = _private_dir(tmp_path / "capture")
    if sidecar is None:
        _private_file(collection / "acquisition-plan.json", plan)
    registration = frozen.FrozenPoolRegistrationContext(
        stage_uuid=str(uuid.UUID(int=731)),
        stage_kind="development",
        source_revision="a" * 40,
        source_closure_sha256="b" * 64,
        protocol_sha256="c" * 64,
        cohorts_sha256="d" * 64,
        candidate_identity_sha256="e" * 64,
        candidate_endpoint_sha256="f" * 64,
        acquisition_plan_sha256=_sha(plan),
        task_ids=("D26-R01",),
        navigation_ids=("D26-N01",),
        evidence_sha256="1" * 64,
        collector_receipt_sha256="2" * 64,
        availability_sha256="3" * 64,
    )
    historical_dir = _private_dir(tmp_path / "historical-source")
    historical_bytes = b"# verified historical operation runner source\n"
    historical_sidecar = _private_file(historical_dir / "coverage_study_acquire.py", historical_bytes)
    return saved.SavedCorpusOrigin(
        registration=registration,
        research_cases=(),
        navigation_targets=(),
        collection_directory=collection,
        expected_collection_receipt_inventory_sha256="4" * 64,
        capture_stage_directory=capture,
        expected_capture_inventory_sha256="5" * 64,
        expected_capture_manifest_sha256="6" * 64,
        expected_package_sha256=saved.package_sha256(collection, capture),
        acquisition_plan_sidecar=sidecar,
        operation_runner_source_sidecar=historical_sidecar,
        operation_runner_source_sha256=_sha(historical_bytes),
    )


def _config(origin: saved.SavedCorpusOrigin, *, include_sidecar: bool = True) -> dict[str, object]:
    value = asdict(origin)
    value["registration"] = asdict(origin.registration)
    value["registration"]["task_ids"] = list(origin.registration.task_ids)
    value["registration"]["navigation_ids"] = list(origin.registration.navigation_ids)
    value["research_cases"] = []
    value["navigation_targets"] = []
    value["collection_directory"] = str(origin.collection_directory)
    value["capture_stage_directory"] = str(origin.capture_stage_directory)
    value["operation_runner_source_sidecar"] = str(origin.operation_runner_source_sidecar)
    if origin.acquisition_plan_sidecar is not None:
        value["acquisition_plan_sidecar"] = str(origin.acquisition_plan_sidecar)
    elif not include_sidecar:
        value.pop("acquisition_plan_sidecar")
    return value


def test_registered_sidecar_is_verified_before_import_without_changing_package(tmp_path):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    handoff = _private_dir(tmp_path / "handoff")
    sidecar = _private_file(handoff / "acquisition-plan.bin", plan)
    origin = _origin(tmp_path, plan, sidecar=sidecar)
    package_before = saved.package_sha256(origin.collection_directory, origin.capture_stage_directory)

    parsed = runner._saved_origin_from_config(_config(origin))

    assert parsed.acquisition_plan_sidecar == sidecar
    assert saved.origin_acquisition_plan_bytes(parsed) == plan
    verified_historical = saved.verify_historical_operation_runner_module(parsed)
    assert verified_historical.origin_stage_uuid == parsed.registration.stage_uuid
    assert verified_historical.origin_source_revision == parsed.registration.source_revision
    assert verified_historical.origin_package_sha256 == parsed.expected_package_sha256
    assert verified_historical.module_bytes == parsed.operation_runner_source_sidecar.read_bytes()
    assert saved.package_sha256(parsed.collection_directory, parsed.capture_stage_directory) == package_before


def test_legacy_complete_package_config_uses_only_fixed_in_package_plan_path(tmp_path):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    origin = _origin(tmp_path, plan, sidecar=None)

    parsed = runner._saved_origin_from_config(_config(origin, include_sidecar=False))

    assert parsed.acquisition_plan_sidecar is None
    assert saved.origin_acquisition_plan_bytes(parsed) == plan


def test_missing_legacy_package_plan_is_rejected_during_static_config_validation(tmp_path):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    origin = _origin(tmp_path, plan, sidecar=None)
    (origin.collection_directory / "acquisition-plan.json").unlink()
    origin = saved.SavedCorpusOrigin(
        **{
            **origin.__dict__,
            "expected_package_sha256": saved.package_sha256(
                origin.collection_directory, origin.capture_stage_directory
            ),
        }
    )

    with pytest.raises(runner.OperatorRunnerError, match="saved-origin-acquisition-plan-invalid"):
        runner._saved_origin_from_config(_config(origin))


def test_sidecar_parent_symlink_into_package_is_rejected(tmp_path):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    origin = _origin(tmp_path, plan, sidecar=None)
    alias = tmp_path / "package-alias"
    alias.symlink_to(origin.collection_directory, target_is_directory=True)
    origin = saved.SavedCorpusOrigin(**{**origin.__dict__, "acquisition_plan_sidecar": alias / "acquisition-plan.json"})

    with pytest.raises(saved.SavedCorpusError, match="origin-acquisition-plan-sidecar-inside-package"):
        saved.origin_acquisition_plan_bytes(origin)


@pytest.mark.parametrize("mutation", ["missing", "wrong-pin", "permissions", "symlink"])
def test_historical_source_module_sidecar_fails_closed(tmp_path, mutation):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    origin = _origin(tmp_path, plan, sidecar=None)
    source_path = origin.operation_runner_source_sidecar
    config = _config(origin)
    if mutation == "missing":
        source_path.unlink()
    elif mutation == "wrong-pin":
        config["operation_runner_source_sha256"] = "9" * 64
    elif mutation == "permissions":
        source_path.chmod(0o640)
    elif mutation == "symlink":
        actual = source_path.with_name("actual.py")
        source_path.rename(actual)
        source_path.symlink_to(actual)

    with pytest.raises(runner.OperatorRunnerError, match="saved-origin-control-source-invalid"):
        runner._saved_origin_from_config(config)


@pytest.mark.parametrize(
    ("mutate", "message"),
    [
        ("missing", "saved-origin-acquisition-plan-invalid"),
        ("wrong-pin", "saved-origin-acquisition-plan-invalid"),
        ("permissions", "saved-origin-acquisition-plan-invalid"),
        ("symlink", "saved-origin-acquisition-plan-invalid"),
        ("inside-package", "saved-origin-acquisition-plan-invalid"),
    ],
)
def test_static_origin_config_rejects_unreadable_or_unbound_sidecars(tmp_path, mutate, message):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    handoff = _private_dir(tmp_path / "handoff")
    sidecar = _private_file(handoff / "acquisition-plan.bin", plan)
    origin = _origin(tmp_path, plan, sidecar=sidecar)
    config = _config(origin)

    if mutate == "missing":
        sidecar.unlink()
    elif mutate == "wrong-pin":
        config["registration"]["acquisition_plan_sha256"] = "9" * 64
    elif mutate == "permissions":
        sidecar.chmod(0o640)
    elif mutate == "symlink":
        sidecar.unlink()
        sidecar.symlink_to(_private_file(handoff / "actual-plan.bin", plan))
    elif mutate == "inside-package":
        config["acquisition_plan_sidecar"] = str(origin.collection_directory / "acquisition-plan.json")

    with pytest.raises(runner.OperatorRunnerError, match=message):
        runner._saved_origin_from_config(config)


def test_sidecar_and_legacy_package_copy_must_be_identical(tmp_path):
    plan = b'{"schema":"synthetic-plan/1","tasks":[]}\n'
    other = b'{"schema":"synthetic-plan/1","tasks":[{}]}\n'
    handoff = _private_dir(tmp_path / "handoff")
    sidecar = _private_file(handoff / "acquisition-plan.bin", plan)
    origin = _origin(tmp_path, plan, sidecar=sidecar)
    _private_file(origin.collection_directory / "acquisition-plan.json", other)
    updated = saved.SavedCorpusOrigin(
        **{
            **origin.__dict__,
            "expected_package_sha256": saved.package_sha256(
                origin.collection_directory, origin.capture_stage_directory
            ),
        }
    )

    with pytest.raises(saved.SavedCorpusError, match="origin-acquisition-plan-sidecar-conflicts-with-package"):
        saved.origin_acquisition_plan_bytes(updated)
