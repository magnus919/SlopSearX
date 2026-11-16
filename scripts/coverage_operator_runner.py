"""Fixed-config operator launcher for a registered coverage-study stage.

The config contains paths and immutable identities, never Python import names or
callbacks. Source, admission, permit, qualification, and grading receipts arrive
through the private operator handoff and are still checked by their existing
source-bound verifiers. This module does not create authority.
"""

from __future__ import annotations

import base64
import hashlib
import importlib.machinery
import json
import math
import os
import re
import stat
import subprocess
import sys
import time
import types
import uuid
from dataclasses import asdict, fields
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from scripts import coverage_answer_execution as answer
from scripts import (
    coverage_grade_closure,
    coverage_jev_execution,
    coverage_legacy_control,
    coverage_live_acquire,
    coverage_selector_input_map,
    coverage_study_acquire,
)
from scripts import coverage_local_authority as local_authority
from scripts import coverage_native_grader_handoff as native_handoff
from scripts import coverage_native_host_dispatch as native_host_dispatch
from scripts import coverage_operator_handoff as operator_handoff
from scripts import coverage_runtime_environment as runtime_environment
from scripts import coverage_saved_corpus as saved_corpus
from scripts import coverage_selector_admission as selector_admission
from scripts import coverage_source_capture as capture
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_stage_runtime as runtime
from scripts import coverage_study_core as core
from scripts import intent_ranking_coverage as coverage_impl
from scripts.coverage_late_registration import request_late_registration
from slopsearx import rerank as production_rerank_impl

CONFIG_SCHEMA = "coverage-operator-stage-config/1"
_BATCH_SCHEMA = "coverage-selector-permit-batch/1"
_SHA = re.compile(r"[0-9a-f]{64}\Z")
_GIT = re.compile(r"[0-9a-f]{40}\Z")
_CONFIG_KEYS = {
    "schema",
    "stage_uuid",
    "packet_stage_uuid",
    "stage_kind",
    "source_revision",
    "candidate_base_url",
    "answer_endpoint",
    "initial_registration_sha256",
    "forbidden_stage_uuids",
    "paths",
    "private_paths",
    "directories",
}
_OPTIONAL_CONFIG_KEYS = {
    "allow_trusted_private_http",
    "saved_origin",
    "development_receipt_sha256",
    "development_stage_uuid",
}
_PATH_KEYS = {
    "cohorts",
    "protocol",
    "qualified_source_closure",
    "coverage_source",
    "production_rerank_source",
    "dependency_lock",
    "reference_manifest",
    "answer_assessment_plan",
    "capture_plan",
    "candidate_identity",
    "initial_registration",
}
_OPTIONAL_PATH_KEYS = {"band_companion"}
_PRIVATE_PATH_KEYS = {"candidate_operator_token", "selector_api_key", "answer_api_key"}
_OPTIONAL_PRIVATE_PATH_KEYS = {"capture_ca_bundle", "development_pass_receipt"}
_DIRECTORY_KEYS = {
    "operator_handoff",
    "stage_inventory",
    "lease_root",
    "capture_receipts",
    "selector_archive",
    "selector_results",
    "answer_archive",
    "answer_results",
    "grader_handoff",
}
_STATIC_REGISTRATION_PIN_NAMES = {
    "protocol",
    "qualified_source_closure",
    "coverage_source",
    "production_rerank_source",
    "dependency_lock",
    "reference_manifest",
    "answer_assessment_plan",
    "acquisition_plan",
    "capture_plan",
}
_SAFE_PREFLIGHT_ERRORS = {
    "config-external-pin-mismatch",
    "config-schema-invalid",
    "config-path-inventory-invalid",
    "config-path-invalid",
    "config-stage-identity-invalid",
    "config-forbidden-stage-inventory-invalid",
    "checkout-audit-unavailable",
    "checkout-revision-mismatch",
    "checkout-not-clean",
    "checkout-source-path-inventory",
    "checkout-source-material-inventory",
    "checkout-source-path-unavailable",
    "loaded-source-path-unavailable",
    "loaded-source-path-mismatch",
    "critical-loaded-source-module-missing",
    "loaded-project-module-outside-checkout",
    "loaded-project-module-source-unverifiable",
    "static-material-shape-invalid",
    "registered-source-materials-required",
    "cohort-inventory-invalid",
    "protocol-cohort-pin-mismatch",
    "source-closure-preflight-failed",
    "active-environment-preflight-failed",
    "initial-registration-config-pin-mismatch",
    "initial-registration-stage-mismatch",
    "initial-registration-schema-invalid",
    "initial-registration-stage-kind-mismatch",
    "initial-registration-source-revision-mismatch",
    "initial-registration-task-inventory-mismatch",
    "initial-registration-pin-inventory-invalid",
    "initial-registration-selector-inventory-invalid",
    "initial-registration-pins-invalid",
    "initial-registration-dynamic-pin-invalid:task_input_manifest",
    "initial-registration-dynamic-pin-invalid:source_capture_manifest",
    "config-development-receipt-binding-incomplete",
    "config-development-receipt-pin-invalid",
    "config-development-stage-uuid-invalid",
    "config-development-receipt-unexpected",
    "confirmation-development-receipt-required",
    "confirmation-development-receipt-pin-mismatch",
    "confirmation-development-receipt-invalid",
    "configured-private-root-inventory-invalid",
    "configured-private-root-path-invalid",
    "configured-private-root-parent-unavailable",
    "configured-private-root-parent-unsafe",
    "configured-private-root-unavailable",
    "configured-private-root-unsafe",
}
_REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
_CHECKOUT_SOURCE_PATHS = {
    "coverage_source": Path(coverage_impl.__file__).resolve(),
    "production_rerank_source": Path(production_rerank_impl.__file__).resolve(),
    "dependency_lock": _REPOSITORY_ROOT / "uv.lock",
}


class OperatorRunnerError(RuntimeError):
    """The explicit operator config or its verified bindings are invalid."""


def _safe_preflight_error_code(exc: Exception) -> str:
    """Return only fixed or enumerated validation codes to the preflight CLI."""
    if type(exc) is not OperatorRunnerError:
        return "preflight-validation-failed"
    code = str(exc)
    if code in _SAFE_PREFLIGHT_ERRORS:
        return code
    prefix = "initial-registration-static-pin-mismatch:"
    if code.startswith(prefix) and code[len(prefix) :] in _STATIC_REGISTRATION_PIN_NAMES:
        return code
    return "preflight-validation-failed"


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _artifact_json(value: object) -> object:
    """Convert checked JSON-like selector control data without string fallbacks."""
    if type(value) is dict:
        if any(type(key) is not str for key in value):
            raise OperatorRunnerError("phase-artifact-json-key-invalid")
        return {key: _artifact_json(child) for key, child in value.items()}
    if type(value) in {list, tuple}:
        return [_artifact_json(child) for child in value]
    if type(value) is set:
        return sorted(_artifact_json(child) for child in value)
    if value is None or type(value) in {str, int, float, bool}:
        return value
    raise OperatorRunnerError("phase-artifact-json-value-invalid")


def _answer_task_artifact_bytes(stage_uuid: str, tasks: tuple[object, ...]) -> bytes:
    rows = []
    for task in tasks:
        rows.append(
            {
                "task_id": task.task_id,
                "kind": task.kind,
                "query": task.query,
                "purpose": task.purpose,
                "critical_checks": _artifact_json(task.critical_checks),
                "answers": [
                    {
                        "arm": item.arm,
                        "body_base64": base64.b64encode(item.body).decode("ascii"),
                        "body_sha256": item.body_sha256,
                        "delivered_source_ids": list(item.delivered_source_ids),
                        "delivered_evidence_ids": list(item.delivered_evidence_ids),
                    }
                    for item in task.answers
                ],
                "catalog_base64": base64.b64encode(task.catalog_bytes).decode("ascii"),
                "catalog_sha256": task.catalog_sha256,
            }
        )
    return _canonical({"schema": "coverage-answer-task-material/1", "stage_uuid": stage_uuid, "tasks": rows})


def _source_bootstrap_ready() -> bool:
    marker = getattr(sys, "_coverage_operator_source_bootstrap", None)
    prefix = getattr(sys, "pycache_prefix", None)
    if marker != "coverage-source-bootstrap/1" or not sys.dont_write_bytecode or type(prefix) is not str:
        return False
    try:
        directory = Path(prefix)
        info = directory.stat(follow_symlinks=False)
        return (
            stat.S_ISDIR(info.st_mode)
            and info.st_uid == os.geteuid()
            and stat.S_IMODE(info.st_mode) == 0o700
            and not any(directory.iterdir())
        )
    except OSError:
        return False


def _require_source_bootstrap() -> None:
    if not _source_bootstrap_ready():
        raise OperatorRunnerError("source-bootstrap-required")


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise OperatorRunnerError("config-duplicate-key")
        result[key] = value
    return result


def _strict(raw: bytes, label: str) -> object:
    try:
        return json.loads(
            raw.decode("utf-8"), object_pairs_hook=_pairs, parse_constant=lambda _: (_ for _ in ()).throw(ValueError())
        )
    except OperatorRunnerError:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        raise OperatorRunnerError(f"{label}-json-invalid") from exc


def _read_file(path: str | os.PathLike[str], maximum: int, *, private: bool = False) -> bytes:
    try:
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    except OSError as exc:
        raise OperatorRunnerError("configured-file-unavailable") from exc
    try:
        info = os.fstat(fd)
        mode = stat.S_IMODE(info.st_mode)
        if not stat.S_ISREG(info.st_mode) or info.st_size > maximum:
            raise OperatorRunnerError("configured-file-invalid")
        if private and (mode & 0o077 or info.st_uid != os.geteuid()):
            raise OperatorRunnerError("configured-secret-file-not-private")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(maximum + 1)
        if len(data) > maximum:
            raise OperatorRunnerError("configured-file-over-cap")
        return data
    finally:
        os.close(fd)


def _verified_selector_operation_states(*, prepared, results, terminal, terminal_bytes: bytes) -> dict[str, str]:
    """Return execution states only from the stage-bound terminal ledger.

    Per-operation result statuses describe ranking outcomes (for example
    ``selected`` or ``fallback``), while the terminal inventory records
    whether each registered operation completed. They are separate contracts.
    """
    if type(terminal_bytes) is not bytes or type(terminal) is not dict:
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    if set(terminal) != {"sha256", "bytes", "status", "all_calls_complete"}:
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    if (
        type(terminal.get("sha256")) is not str
        or not _SHA.fullmatch(terminal["sha256"])
        or terminal["sha256"] != _sha(terminal_bytes)
        or type(terminal.get("bytes")) is not int
        or terminal["bytes"] != len(terminal_bytes)
    ):
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    try:
        document = core._strict_json(terminal_bytes, "selector-terminal-inventory")
    except core.StudyError as exc:
        raise OperatorRunnerError("selector-terminal-inventory-invalid") from exc
    if type(document) is not dict:
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    expected_bindings = {
        "schema": coverage_jev_execution.INVENTORY_SCHEMA,
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "registration_sha256": prepared.registration_sha256,
        "protocol_sha256": prepared.pins.get("protocol"),
        "source_closure_sha256": prepared.pins.get("qualified_source_closure"),
        "status": "all-registered-calls-complete",
        "all_calls_complete": True,
    }
    if (
        any(document.get(name) != value for name, value in expected_bindings.items())
        or document.get("all_calls_complete") is not True
        or type(document.get("status")) is not str
    ):
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    if terminal.get("status") != document["status"] or terminal.get("all_calls_complete") is not True:
        raise OperatorRunnerError("selector-terminal-inventory-invalid")

    operation_ids = prepared.operation_ids
    if (
        type(operation_ids) is not tuple
        or not operation_ids
        or any(type(operation_id) is not str or not operation_id for operation_id in operation_ids)
        or len(set(operation_ids)) != len(operation_ids)
    ):
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    operations = document.get("operations")
    if type(operations) is not list or len(operations) != len(operation_ids):
        raise OperatorRunnerError("selector-terminal-inventory-invalid")
    ledger_ids = []
    states = {}
    for row in operations:
        if type(row) is not dict or set(row) != {"operation_id", "state"}:
            raise OperatorRunnerError("selector-terminal-inventory-invalid")
        operation_id = row.get("operation_id")
        state = row.get("state")
        if type(operation_id) is not str or not operation_id or type(state) is not str:
            raise OperatorRunnerError("selector-terminal-inventory-invalid")
        if operation_id in states:
            raise OperatorRunnerError("selector-terminal-inventory-invalid")
        ledger_ids.append(operation_id)
        states[operation_id] = state
    if tuple(ledger_ids) != operation_ids or any(state != "complete-success" for state in states.values()):
        raise OperatorRunnerError("selector-terminal-inventory-invalid")

    if type(results) not in {list, tuple} or len(results) != len(operation_ids):
        raise OperatorRunnerError("selector-result-inventory-mismatch")
    result_ids = tuple(getattr(row, "operation_id", None) for row in results)
    if (
        any(type(operation_id) is not str or not operation_id for operation_id in result_ids)
        or result_ids != operation_ids
        or len(set(result_ids)) != len(result_ids)
    ):
        raise OperatorRunnerError("selector-result-inventory-mismatch")
    successful_result_statuses = {"complete-original-v1", "fallback", "selected"}
    if any(
        type(getattr(row, "status", None)) is not str
        or row.status not in successful_result_statuses
        or not hasattr(row, "terminal_reason")
        or row.terminal_reason is not None
        for row in results
    ):
        raise OperatorRunnerError("selector-result-outcome-invalid")
    return states


def _saved_origin_from_config(value: object) -> saved_corpus.SavedCorpusOrigin:
    """Read only the registered archive description, never historical grades."""
    from scripts import coverage_frozen_pool_inputs

    origin_fields = {field.name for field in fields(saved_corpus.SavedCorpusOrigin)}
    optional_origin_fields = {
        "acquisition_plan_sidecar",
        "operation_runner_source_sidecar",
        "operation_runner_source_sha256",
    }
    legacy_origin_fields = origin_fields - optional_origin_fields
    acquisition_plan_fields = legacy_origin_fields | {"acquisition_plan_sidecar"}
    historical_source_fields = legacy_origin_fields | {
        "operation_runner_source_sidecar",
        "operation_runner_source_sha256",
    }
    if type(value) is not dict or frozenset(value) not in {
        frozenset(legacy_origin_fields),
        frozenset(acquisition_plan_fields),
        frozenset(historical_source_fields),
        frozenset(origin_fields),
    }:
        raise OperatorRunnerError("saved-origin-config-shape-invalid")
    args = dict(value)
    for optional_field in optional_origin_fields:
        args.setdefault(optional_field, None)
    registration = args["registration"]
    if type(registration) is not dict or set(registration) != {
        field.name for field in fields(coverage_frozen_pool_inputs.FrozenPoolRegistrationContext)
    }:
        raise OperatorRunnerError("saved-origin-registration-invalid")
    registration = dict(registration)
    for key in ("task_ids", "navigation_ids"):
        if type(registration[key]) is not list:
            raise OperatorRunnerError("saved-origin-task-inventory-invalid")
        registration[key] = tuple(registration[key])
    args["registration"] = coverage_frozen_pool_inputs.FrozenPoolRegistrationContext(**registration)
    for key in ("research_cases", "navigation_targets"):
        if type(args[key]) is not list or any(type(row) is not dict for row in args[key]):
            raise OperatorRunnerError("saved-origin-cohort-invalid")
        args[key] = tuple(args[key])
    for key in ("collection_directory", "capture_stage_directory"):
        if type(args[key]) is not str or not Path(args[key]).is_absolute():
            raise OperatorRunnerError("saved-origin-path-invalid")
        args[key] = Path(args[key])
    sidecar = args["acquisition_plan_sidecar"]
    if sidecar is not None:
        if type(sidecar) is not str or not Path(sidecar).is_absolute():
            raise OperatorRunnerError("saved-origin-acquisition-plan-sidecar-path-invalid")
        args["acquisition_plan_sidecar"] = Path(sidecar)
    source_sidecar = args["operation_runner_source_sidecar"]
    if source_sidecar is not None:
        if type(source_sidecar) is not str or not Path(source_sidecar).is_absolute():
            raise OperatorRunnerError("saved-origin-operation-runner-sidecar-path-invalid")
        args["operation_runner_source_sidecar"] = Path(source_sidecar)
    origin = saved_corpus.SavedCorpusOrigin(**args)
    if (
        saved_corpus.package_sha256(origin.collection_directory, origin.capture_stage_directory)
        != origin.expected_package_sha256
    ):
        raise OperatorRunnerError("saved-origin-package-mismatch")
    try:
        saved_corpus.origin_acquisition_plan_bytes(origin)
    except saved_corpus.SavedCorpusError as exc:
        raise OperatorRunnerError("saved-origin-acquisition-plan-invalid") from exc
    try:
        saved_corpus.verify_historical_operation_runner_module(origin)
    except saved_corpus.SavedCorpusError as exc:
        raise OperatorRunnerError("saved-origin-control-source-invalid") from exc
    return origin


def _secret(path: str) -> str:
    raw = _read_file(path, 16_384, private=True)
    try:
        value = raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise OperatorRunnerError("configured-secret-invalid") from exc
    value = value.rstrip("\r\n")
    if not value or value != value.strip() or "\n" in value or "\r" in value:
        raise OperatorRunnerError("configured-secret-invalid")
    return value


def _load_config(path: str | os.PathLike[str], expected_sha256: str) -> dict[str, Any]:
    raw = _read_file(path, 1_000_000, private=True)
    if type(expected_sha256) is not str or not _SHA.fullmatch(expected_sha256) or _sha(raw) != expected_sha256:
        raise OperatorRunnerError("config-external-pin-mismatch")
    value = _strict(raw, "config")
    if (
        type(value) is not dict
        or not _CONFIG_KEYS <= set(value)
        or set(value) - _CONFIG_KEYS - _OPTIONAL_CONFIG_KEYS
        or value.get("schema") != CONFIG_SCHEMA
        or _canonical(value) != raw
    ):
        raise OperatorRunnerError("config-schema-invalid")
    if (
        not _PATH_KEYS <= set(value["paths"])
        or set(value["paths"]) - _PATH_KEYS - _OPTIONAL_PATH_KEYS
        or not _PRIVATE_PATH_KEYS <= set(value["private_paths"])
        or set(value["private_paths"]) - _PRIVATE_PATH_KEYS - _OPTIONAL_PRIVATE_PATH_KEYS
        or set(value["directories"]) != _DIRECTORY_KEYS
    ):
        raise OperatorRunnerError("config-path-inventory-invalid")
    if "allow_trusted_private_http" in value and type(value["allow_trusted_private_http"]) is not bool:
        raise OperatorRunnerError("config-private-http-option-invalid")
    receipt_fields_present = (
        "development_pass_receipt" in value["private_paths"],
        "development_receipt_sha256" in value,
        "development_stage_uuid" in value,
    )
    if any(receipt_fields_present) and not all(receipt_fields_present):
        raise OperatorRunnerError("config-development-receipt-binding-incomplete")
    if "development_receipt_sha256" in value and (
        type(value["development_receipt_sha256"]) is not str or not _SHA.fullmatch(value["development_receipt_sha256"])
    ):
        raise OperatorRunnerError("config-development-receipt-pin-invalid")
    if "development_stage_uuid" in value:
        try:
            if (
                type(value["development_stage_uuid"]) is not str
                or str(uuid.UUID(value["development_stage_uuid"])) != value["development_stage_uuid"]
            ):
                raise ValueError
        except ValueError as exc:
            raise OperatorRunnerError("config-development-stage-uuid-invalid") from exc
    if value["stage_kind"] == "development" and any(receipt_fields_present):
        raise OperatorRunnerError("config-development-receipt-unexpected")
    for key in (*_PATH_KEYS, *_OPTIONAL_PATH_KEYS, *_PRIVATE_PATH_KEYS, *_OPTIONAL_PRIVATE_PATH_KEYS, *_DIRECTORY_KEYS):
        collection = (
            value["paths"]
            if key in _PATH_KEYS or key in _OPTIONAL_PATH_KEYS
            else value["private_paths"]
            if key in _PRIVATE_PATH_KEYS or key in _OPTIONAL_PRIVATE_PATH_KEYS
            else value["directories"]
        )
        if key in (_OPTIONAL_PRIVATE_PATH_KEYS | _OPTIONAL_PATH_KEYS) and key not in collection:
            continue
        if type(collection[key]) is not str or not collection[key]:
            raise OperatorRunnerError("config-path-invalid")
        if not Path(collection[key]).is_absolute():
            raise OperatorRunnerError("config-path-must-be-absolute")
    if type(value["source_revision"]) is not str or not _GIT.fullmatch(value["source_revision"]):
        raise OperatorRunnerError("config-source-revision-invalid")
    for key in ("stage_uuid", "packet_stage_uuid"):
        try:
            if type(value[key]) is not str or str(uuid.UUID(value[key])) != value[key]:
                raise ValueError
        except ValueError as exc:
            raise OperatorRunnerError("config-stage-uuid-invalid") from exc
    if type(value["initial_registration_sha256"]) is not str or not _SHA.fullmatch(
        value["initial_registration_sha256"]
    ):
        raise OperatorRunnerError("config-registration-pin-invalid")
    if value["stage_uuid"] == value["packet_stage_uuid"] or value["stage_kind"] not in {
        "development",
        "confirmation",
    }:
        raise OperatorRunnerError("config-stage-identity-invalid")
    forbidden = value["forbidden_stage_uuids"]
    try:
        if (
            type(forbidden) is not list
            or not forbidden
            or any(type(item) is not str or str(uuid.UUID(item)) != item for item in forbidden)
            or value["stage_uuid"] in forbidden
            or value["packet_stage_uuid"] in forbidden
        ):
            raise ValueError
    except ValueError as exc:
        raise OperatorRunnerError("config-forbidden-stage-inventory-invalid") from exc
    return value


def _verify_checkout_sources(
    source_revision: str,
    paths: dict[str, str],
    material_bytes: dict[str, bytes],
) -> None:
    """Bind supplied material pins and loaded Python code to the clean checkout."""
    try:
        revision = subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD^{commit}"],
            cwd=_REPOSITORY_ROOT,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        ).stdout.strip()
        status = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            cwd=_REPOSITORY_ROOT,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        ).stdout
    except (OSError, subprocess.SubprocessError) as exc:
        raise OperatorRunnerError("checkout-audit-unavailable") from exc
    if revision != source_revision:
        raise OperatorRunnerError("checkout-revision-mismatch")
    if status.strip():
        raise OperatorRunnerError("checkout-not-clean")
    if not set(_CHECKOUT_SOURCE_PATHS) <= set(paths):
        raise OperatorRunnerError("checkout-source-path-inventory")
    if not set(_CHECKOUT_SOURCE_PATHS) <= set(material_bytes):
        raise OperatorRunnerError("checkout-source-material-inventory")
    for name, expected_path in _CHECKOUT_SOURCE_PATHS.items():
        try:
            configured_path = Path(paths[name]).resolve(strict=True)
            actual_path = expected_path.resolve(strict=True)
            if name == "production_rerank_source":
                protocol_raw = material_bytes.get("protocol")
                protocol = core._strict_json(protocol_raw, "checkout-protocol") if type(protocol_raw) is bytes else None
                frozen_v1_path = (
                    _REPOSITORY_ROOT / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
                ).resolve(strict=True)
                if (
                    type(protocol) is dict
                    and protocol.get("pool_input_mode") == "frozen-pool/1"
                    and configured_path == frozen_v1_path
                ):
                    actual_path = frozen_v1_path
            actual_bytes = actual_path.read_bytes()
        except OSError as exc:
            raise OperatorRunnerError("checkout-source-path-unavailable") from exc
        if configured_path != actual_path:
            raise OperatorRunnerError(f"checkout-source-path-mismatch:{name}")
        if material_bytes[name] != actual_bytes:
            raise OperatorRunnerError(f"checkout-source-material-mismatch:{name}")
    critical_modules = (
        (coverage_impl, _CHECKOUT_SOURCE_PATHS["coverage_source"]),
        (production_rerank_impl, _CHECKOUT_SOURCE_PATHS["production_rerank_source"]),
        (native_host_dispatch, _REPOSITORY_ROOT / "scripts" / "coverage_native_host_dispatch.py"),
    )
    for module, expected_path in critical_modules:
        try:
            module_path = Path(module.__file__).resolve(strict=True)
        except (AttributeError, OSError) as exc:
            raise OperatorRunnerError("loaded-source-path-unavailable") from exc
        if module_path != expected_path.resolve(strict=True):
            raise OperatorRunnerError("loaded-source-path-mismatch")
    critical_modules = (
        coverage_grade_closure,
        coverage_jev_execution,
        coverage_legacy_control,
        coverage_selector_input_map,
        native_host_dispatch,
    )
    if any(module.__name__ not in sys.modules for module in critical_modules):
        raise OperatorRunnerError("critical-loaded-source-module-missing")
    for name, module in tuple(sys.modules.items()):
        if not name.startswith(("scripts.", "slopsearx.", "engines.")):
            continue
        module_file = getattr(module, "__file__", None)
        if module_file is None:
            continue
        try:
            module_path = Path(module_file).resolve(strict=True)
            module_path.relative_to(_REPOSITORY_ROOT)
        except (OSError, ValueError) as exc:
            raise OperatorRunnerError("loaded-project-module-outside-checkout") from exc
        if module_path.suffix != ".py":
            raise OperatorRunnerError("loaded-project-module-source-unverifiable")
        _verify_loaded_module_code(name, module, module_path)


def _code_fingerprint(code: types.CodeType) -> tuple[object, ...]:
    """Return executable code structure without source-location metadata."""
    constants = tuple(
        _code_fingerprint(item) if isinstance(item, types.CodeType) else (type(item).__name__, item)
        for item in code.co_consts
    )
    return (
        code.co_argcount,
        code.co_posonlyargcount,
        code.co_kwonlyargcount,
        code.co_nlocals,
        code.co_stacksize,
        code.co_flags,
        code.co_code,
        code.co_exceptiontable,
        constants,
        code.co_names,
        code.co_varnames,
        code.co_freevars,
        code.co_cellvars,
        code.co_name,
        code.co_qualname,
    )


def _function_code(value: object) -> types.CodeType | None:
    if isinstance(value, (staticmethod, classmethod)):
        value = value.__func__
    if isinstance(value, property):
        return None
    wrapped = getattr(value, "__wrapped__", None)
    if wrapped is not None:
        value = wrapped
    return value.__code__ if isinstance(value, types.FunctionType) else None


def _verify_runtime_callables(module: types.ModuleType, expected: types.CodeType) -> None:
    """Require every source-defined top-level function/method to remain loaded."""
    namespace = vars(module)
    for definition in expected.co_consts:
        if not isinstance(definition, types.CodeType) or "." in definition.co_qualname:
            continue
        if definition.co_name.startswith("<"):
            continue
        value = namespace.get(definition.co_name)
        if definition.co_name == "<lambda>":
            continue
        if isinstance(value, type):
            for method in definition.co_consts:
                if (
                    not isinstance(method, types.CodeType)
                    or method.co_qualname.count(".") != 1
                    or method.co_name.startswith("<")
                ):
                    continue
                member = vars(value).get(method.co_name)
                actual = _function_code(member)
                if isinstance(member, property):
                    actuals = [
                        accessor.__code__
                        for accessor in (member.fget, member.fset, member.fdel)
                        if accessor is not None
                    ]
                else:
                    actuals = [] if actual is None else [actual]
                if not any(_code_fingerprint(code) == _code_fingerprint(method) for code in actuals):
                    raise OperatorRunnerError("loaded-source-function-mismatch")
        else:
            actual = _function_code(value)
            if actual is None or _code_fingerprint(actual) != _code_fingerprint(definition):
                raise OperatorRunnerError("loaded-source-function-mismatch")


def _verify_loaded_module_code(name: str, module: types.ModuleType, source_path: Path) -> None:
    """Reject stale/poisoned pyc and runtime function code that differs from source."""
    try:
        source = _read_file(source_path, 64_000_000)
        expected = compile(source, str(source_path), "exec", dont_inherit=True, optimize=sys.flags.optimize)
        spec = getattr(module, "__spec__", None)
        loader = getattr(spec, "loader", None)
        origin = getattr(spec, "origin", None)
        if (
            not isinstance(loader, importlib.machinery.SourceFileLoader)
            or type(origin) is not str
            or Path(origin).resolve(strict=True) != source_path
        ):
            raise OperatorRunnerError("loaded-source-loader-mismatch")
        loaded = loader.get_code(name) if loader is not None else None
    except OperatorRunnerError:
        raise
    except (OSError, SyntaxError, ValueError, ImportError, AttributeError) as exc:
        raise OperatorRunnerError("loaded-source-code-unavailable") from exc
    if not isinstance(loaded, types.CodeType) or _code_fingerprint(loaded) != _code_fingerprint(expected):
        raise OperatorRunnerError("loaded-source-bytecode-mismatch")
    _verify_runtime_callables(module, expected)


def _build_acquisition_plan(stage_uuid: str, cases: list[dict[str, Any]], targets: list[dict[str, Any]]) -> bytes:
    tasks = []
    for row in cases:
        engines = row.get("pool_plan", {}).get("engines")
        if type(engines) is not list:
            raise OperatorRunnerError("cohort-pool-plan-invalid")
        calls = {
            engine: (2 if engine == "wikipedia" and engine in engines else 1 if engine in engines else 0)
            for engine in ("wikipedia", "arxiv", "github", "openalex")
        }
        tasks.append({"task_id": row["task_id"], "engine_calls": calls})
    for row in targets:
        tasks.append(
            {"task_id": row["target_id"], "engine_calls": {"wikipedia": 0, "arxiv": 0, "github": 1, "openalex": 0}}
        )
    return _canonical({"schema": core.ACQUISITION_SCHEMA, "stage_uuid": stage_uuid, "tasks": tasks})


def _stage_cohort(cohorts: object, stage_kind: str) -> dict[str, Any]:
    if stage_kind not in {"development", "confirmation"} or type(cohorts) is not dict:
        raise OperatorRunnerError("configured-stage-cohort-invalid")
    stage_rows = cohorts.get("stages")
    if type(stage_rows) is not list:
        raise OperatorRunnerError("configured-stage-cohort-invalid")
    rows = [row for row in stage_rows if type(row) is dict and row.get("stage") == stage_kind]
    if len(rows) != 1:
        raise OperatorRunnerError("configured-stage-cohort-not-unique")
    return rows[0]


def _inspect_private_directories(directories: dict[str, str]) -> list[Path]:
    """Validate configured roots without creating or changing filesystem paths."""
    if type(directories) is not dict or set(directories) != _DIRECTORY_KEYS:
        raise OperatorRunnerError("configured-private-root-inventory-invalid")

    missing: list[Path] = []
    owner = os.geteuid()
    for name in sorted(_DIRECTORY_KEYS):
        value = directories[name]
        if type(value) is not str or not value:
            raise OperatorRunnerError("configured-private-root-path-invalid")
        path = Path(value)
        if not path.is_absolute() or path == path.parent:
            raise OperatorRunnerError("configured-private-root-path-invalid")
        try:
            parent_info = path.parent.lstat()
        except OSError as exc:
            raise OperatorRunnerError("configured-private-root-parent-unavailable") from exc
        if stat.S_ISLNK(parent_info.st_mode) or not stat.S_ISDIR(parent_info.st_mode):
            raise OperatorRunnerError("configured-private-root-parent-unsafe")
        try:
            info = path.lstat()
        except FileNotFoundError:
            missing.append(path)
        except OSError as exc:
            raise OperatorRunnerError("configured-private-root-unavailable") from exc
        else:
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISDIR(info.st_mode)
                or stat.S_IMODE(info.st_mode) != 0o700
                or info.st_uid != owner
            ):
                raise OperatorRunnerError("configured-private-root-unsafe")
    return missing


def _prepare_private_directories(directories: dict[str, str]) -> None:
    """Create configured stage roots without repairing unsafe existing paths.

    Static registration validation must finish before this helper is called.
    All roots are checked before any missing root is created, so an unsafe
    existing path cannot leave a partial set of new roots behind.
    """
    missing = _inspect_private_directories(directories)
    owner = os.geteuid()
    for path in missing:
        try:
            os.mkdir(path, mode=0o700)
        except FileExistsError:
            # A concurrent creator is acceptable only if the resulting root
            # already satisfies the exact private-directory contract.
            pass
        except OSError as exc:
            raise OperatorRunnerError("configured-private-root-create-failed") from exc
        try:
            info = path.lstat()
            parent_fd = os.open(
                path.parent,
                os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
            )
        except OSError as exc:
            raise OperatorRunnerError("configured-private-root-create-failed") from exc
        try:
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISDIR(info.st_mode)
                or stat.S_IMODE(info.st_mode) != 0o700
                or info.st_uid != owner
            ):
                raise OperatorRunnerError("configured-private-root-unsafe")
            os.fsync(parent_fd)
        except OSError as exc:
            raise OperatorRunnerError("configured-private-root-create-failed") from exc
        finally:
            os.close(parent_fd)


class OperatorStageRunner:
    """Loads a pinned registered bundle and invokes only the fixed phase graph."""

    def __init__(self, config: dict[str, Any], *, config_sha256: str, preflight_only: bool = False):
        _require_source_bootstrap()
        self.config = config
        self.config_sha256 = config_sha256
        self.paths = config["paths"]
        self.private_paths = config["private_paths"]
        self.directories = config["directories"]
        self.preflight_only = preflight_only
        self._frozen_stage_clock_bytes: bytes | None = None
        self._active_registration_bytes: bytes | None = None
        self._active_input_manifest_bytes: bytes | None = None
        self._pipeline_inputs: dict[str, object] | None = None
        self._frozen_pool_reference_bytes: bytes | None = None
        self._saved_corpus_reference_bytes: bytes | None = None
        self._build_plan(preflight_only=preflight_only)
        if not preflight_only:
            self.handoff = operator_handoff.OperatorReceiptHandoff(self.directories["operator_handoff"])

    @classmethod
    def from_file(cls, path: str | os.PathLike[str], expected_sha256: str) -> "OperatorStageRunner":
        return cls(_load_config(path, expected_sha256), config_sha256=expected_sha256)

    @classmethod
    def preflight_from_file(cls, path: str | os.PathLike[str], expected_sha256: str) -> "OperatorStageRunner":
        """Validate static stage bindings without starting the run or making filesystem changes."""
        return cls(_load_config(path, expected_sha256), config_sha256=expected_sha256, preflight_only=True)

    def _build_plan(self, *, preflight_only: bool = False) -> None:
        path_bytes = {key: _read_file(value, 64_000_000) for key, value in self.paths.items()}
        _verify_checkout_sources(self.config["source_revision"], self.paths, path_bytes)
        cohorts = core._strict_json(path_bytes["cohorts"], "cohorts")
        protocol = core._strict_json(path_bytes["protocol"], "protocol")
        closure = core._strict_json(path_bytes["qualified_source_closure"], "source-closure")
        identity = core._strict_json(path_bytes["candidate_identity"], "candidate-identity")
        if (
            type(cohorts) is not dict
            or type(protocol) is not dict
            or type(closure) is not dict
            or type(identity) is not dict
        ):
            raise OperatorRunnerError("static-material-shape-invalid")
        if (
            protocol.get("selector_input_map_schema") != "coverage-selector-input-map/2-registered"
            or protocol.get("selector_input_map_status") != "registered"
            or closure.get("source_revision") != self.config["source_revision"]
            or closure.get("schema")
            not in {"coverage-study-qualified-source-closure/1", "coverage-study-qualified-source-closure/2"}
            or type(identity.get("runtime")) is not dict
            or type(identity["runtime"].get("revision")) is not str
        ):
            raise OperatorRunnerError("registered-source-materials-required")
        companion_path_supplied = "band_companion" in path_bytes
        companion_closure_enabled = closure.get("schema") == "coverage-study-qualified-source-closure/2"
        if companion_path_supplied != companion_closure_enabled:
            raise OperatorRunnerError("source-closure-companion-path-mismatch")
        try:
            protocol, _protocol_sha256 = capture._validate_protocol(path_bytes["protocol"])
        except capture.SourceCaptureError as exc:
            raise OperatorRunnerError("capture-protocol-preflight-invalid") from exc
        try:
            identity = capture._validate_candidate_identity(
                path_bytes["candidate_identity"], _sha(path_bytes["candidate_identity"])
            )
        except capture.SourceCaptureError as exc:
            raise OperatorRunnerError("candidate-identity-preflight-invalid") from exc
        stage_row = _stage_cohort(cohorts, self.config["stage_kind"])
        cases = stage_row.get("research_cases")
        targets = stage_row.get("navigation_targets")
        if type(cases) is not list or type(targets) is not list:
            raise OperatorRunnerError("cohort-inventory-invalid")
        pool_input_mode = coverage_study_acquire.pool_input_mode_for_stage(protocol, self.config["stage_kind"])
        self.saved_origin = (
            _saved_origin_from_config(self.config["saved_origin"]) if "saved_origin" in self.config else None
        )
        if (pool_input_mode == "saved-corpus/1") != (self.saved_origin is not None):
            raise OperatorRunnerError("saved-origin-mode-mismatch")
        acquisition_plan = _build_acquisition_plan(self.config["stage_uuid"], cases, targets)
        if pool_input_mode == "saved-corpus/1":
            plan_doc = core._strict_json(acquisition_plan, "saved-search-plan")
            for row in plan_doc["tasks"]:
                row["engine_calls"] = {name: 0 for name in row["engine_calls"]}
            acquisition_plan = _canonical(plan_doc)
        if protocol.get("cohorts_sha256") != _sha(path_bytes["cohorts"]):
            raise OperatorRunnerError("protocol-cohort-pin-mismatch")
        companion_sha = _sha(path_bytes["band_companion"]) if "band_companion" in path_bytes else None
        if pool_input_mode in {"frozen-pool/1", "saved-corpus/1"}:
            from scripts import coverage_band_companion as band_companion

            if closure.get("schema") != "coverage-study-qualified-source-closure/2":
                raise OperatorRunnerError("frozen-pool-source-closure-v2-required")
            expected_companion = band_companion.contract_sha256_for(
                _sha(path_bytes["protocol"]), _sha(path_bytes["cohorts"])
            )
            if companion_sha != expected_companion:
                raise OperatorRunnerError("frozen-pool-band-companion-required")
        initial_registration_sha256 = _sha(path_bytes["initial_registration"])
        if initial_registration_sha256 != self.config["initial_registration_sha256"]:
            raise OperatorRunnerError("initial-registration-config-pin-mismatch")
        try:
            source_pins = None
            if closure.get("schema") == "coverage-study-qualified-source-closure/2":
                source_names = {
                    "coverage_study_core": "scripts/coverage_study_core.py",
                    "coverage_gate_calculation": "scripts/coverage_gate_calculation.py",
                    "coverage_stage_orchestration": "scripts/coverage_stage_orchestration.py",
                    "coverage_stage_finalization": "scripts/coverage_stage_finalization.py",
                    "coverage_operator_runner": "scripts/coverage_operator_runner.py",
                    "coverage_operator_handoff": "scripts/coverage_operator_handoff.py",
                    "coverage_atomic_io": "scripts/coverage_atomic_io.py",
                    "coverage_stage_runtime": "scripts/coverage_stage_runtime.py",
                    "coverage_local_authority": "scripts/coverage_local_authority.py",
                    "coverage_frozen_pool_inputs": "scripts/coverage_frozen_pool_inputs.py",
                    "coverage_frozen_pool_collect": "scripts/coverage_frozen_pool_collect.py",
                    "coverage_resource_evidence": "scripts/coverage_resource_evidence.py",
                    "coverage_pipeline_inputs": "scripts/coverage_pipeline_inputs.py",
                    "coverage_selector_input_map": "scripts/coverage_selector_input_map.py",
                    "coverage_band_companion": "scripts/coverage_band_companion.py",
                    "coverage_late_registration": "scripts/coverage_late_registration.py",
                }
                if "saved-corpus/1" in (protocol.get("pool_input_mode_by_stage") or {}).values():
                    source_names.update(
                        {
                            "coverage_saved_corpus": "scripts/coverage_saved_corpus.py",
                            "coverage_assessment_checkpoint": "scripts/coverage_assessment_checkpoint.py",
                        }
                    )
                root = Path(__file__).resolve().parents[1]
                source_pins = {name: _sha((root / relative).read_bytes()) for name, relative in source_names.items()}
            core._verify_qualified_source_closure(
                path_bytes["qualified_source_closure"],
                expected_revision=self.config["source_revision"],
                expected_pins={
                    "coverage_source": _sha(path_bytes["coverage_source"]),
                    "production_rerank_source": _sha(path_bytes["production_rerank_source"]),
                    "dependency_lock": _sha(path_bytes["dependency_lock"]),
                    **(
                        {"band_companion": _sha(path_bytes["band_companion"])} if "band_companion" in path_bytes else {}
                    ),
                },
                expected_source_pins=source_pins,
                expected_band_companion_sha256=companion_sha,
            )
        except Exception as exc:
            raise OperatorRunnerError("source-closure-preflight-failed") from exc
        self.band_companion_source_pins = source_pins
        try:
            self.active_environment_receipt_bytes = runtime_environment.verify_active_environment(
                path_bytes["dependency_lock"],
                expected_lock_sha256=closure["material_pins"]["dependency_lock"],
            )
        except runtime_environment.RuntimeEnvironmentError as exc:
            raise OperatorRunnerError("active-environment-preflight-failed") from exc
        endpoint = self.config["candidate_base_url"]
        _health_url, _scrape_url, endpoint_sha = capture._candidate_endpoints(endpoint)
        try:
            execution_profile = coverage_study_acquire.execution_profile_from_protocol(
                protocol.get("acquisition"),
                pool_input_mode=pool_input_mode,
            )
        except ValueError as exc:
            raise OperatorRunnerError("acquisition-execution-profile-invalid") from exc
        preacquisition_input = {
            "schema": "coverage-preacquisition-input-manifest/1",
            "stage_uuid": self.config["stage_uuid"],
            "protocol_sha256": _sha(path_bytes["protocol"]),
            "cohorts_sha256": _sha(path_bytes["cohorts"]),
            "task_ids": [row["task_id"] for row in cases] + [row["target_id"] for row in targets],
        }
        if companion_sha is not None:
            preacquisition_input.update(
                {
                    "band_companion_sha256": companion_sha,
                    "initial_registration_sha256": initial_registration_sha256,
                }
            )
        preacq = _canonical(preacquisition_input)
        acquisition_manifest_doc = {
            "schema": execution_profile.manifest_schema,
            "stage": self.config["stage_kind"],
            "stage_uuid": self.config["stage_uuid"],
            "source_revision": self.config["source_revision"],
            "source_closure_sha256": _sha(path_bytes["qualified_source_closure"]),
            "protocol_sha256": _sha(path_bytes["protocol"]),
            "cohorts_sha256": _sha(path_bytes["cohorts"]),
            "input_manifest_sha256": _sha(preacq),
            "acquisition_plan_sha256": _sha(acquisition_plan),
            "research_cases": cases,
            "navigation_targets": targets,
        }
        if execution_profile is coverage_study_acquire.CONTINUE_ACQUISITION_PROFILE:
            acquisition_manifest_doc["execution_profile"] = execution_profile.identifier
        if pool_input_mode == "saved-corpus/1":
            acquisition_manifest_doc.update(
                {
                    "schema": "coverage-saved-import-plan/1",
                    "packet_stage_uuid": self.config["packet_stage_uuid"],
                    "origin_package_sha256": self.saved_origin.expected_package_sha256,
                }
            )
        acquisition_manifest = (
            _canonical(acquisition_manifest_doc)
            if pool_input_mode == "saved-corpus/1"
            else coverage_live_acquire._canonical(acquisition_manifest_doc)
        )
        self.static_materials = {
            "protocol": path_bytes["protocol"],
            "qualified_source_closure": path_bytes["qualified_source_closure"],
            "coverage_source": path_bytes["coverage_source"],
            "production_rerank_source": path_bytes["production_rerank_source"],
            "dependency_lock": path_bytes["dependency_lock"],
            "reference_manifest": path_bytes["reference_manifest"],
            "answer_assessment_plan": path_bytes["answer_assessment_plan"],
            "acquisition_plan": acquisition_plan,
            "capture_plan": path_bytes["capture_plan"],
        }
        self.initial_registration = path_bytes["initial_registration"]
        self.initial_registration_sha256 = initial_registration_sha256
        self.candidate_identity = identity
        # Verify every static registration pin before taking the stage clock or
        # creating operator/lease directories. Dynamic pins are replaced only
        # by the separately pinned late registration after acquisition.
        try:
            self.initial_doc = core._parse_registration(self.initial_registration)
        except core.StudyError as exc:
            raise OperatorRunnerError("initial-registration-schema-invalid") from exc
        if self.initial_doc.get("stage_uuid") != self.config["stage_uuid"]:
            raise OperatorRunnerError("initial-registration-stage-mismatch")
        if self.initial_doc.get("stage_kind") != self.config["stage_kind"]:
            raise OperatorRunnerError("initial-registration-stage-kind-mismatch")
        if self.initial_doc.get("source_revision") != self.config["source_revision"]:
            raise OperatorRunnerError("initial-registration-source-revision-mismatch")
        expected_task_ids = tuple(row["task_id"] for row in cases) + tuple(row["target_id"] for row in targets)
        try:
            registered_task_ids = core._task_ids(
                self.initial_doc.get("task_ids"), "registration-task-ids", expected_count=13
            )
        except core.StudyError as exc:
            raise OperatorRunnerError("initial-registration-task-inventory-mismatch") from exc
        if registered_task_ids != expected_task_ids:
            raise OperatorRunnerError("initial-registration-task-inventory-mismatch")
        pins = self.initial_doc.get("pins")
        if type(pins) is not dict or set(pins) != set(core.PIN_NAMES):
            raise OperatorRunnerError("initial-registration-pin-inventory-invalid")
        companion_sha = _sha(path_bytes["band_companion"]) if "band_companion" in path_bytes else None
        if self.initial_doc.get("band_companion_sha256") != companion_sha:
            raise OperatorRunnerError("initial-registration-companion-binding-mismatch")
        if protocol.get("pool_input_mode") == "frozen-pool/1" and companion_sha is None:
            raise OperatorRunnerError("initial-registration-frozen-companion-required")
        try:
            # The initial registration declares the navigation skip decisions;
            # validate their fixed inventory and the corresponding selector
            # schedule without inventing acquisition results during preflight.
            registered_operations = core.selector_inventory(self.initial_doc.get("navigation_skip_reasons"))
        except (core.StudyError, TypeError) as exc:
            raise OperatorRunnerError("initial-registration-selector-inventory-invalid") from exc
        operation_ids = self.initial_doc.get("operation_ids")
        if type(operation_ids) is not list or tuple(operation_ids) != tuple(
            call.operation_id for call in registered_operations
        ):
            raise OperatorRunnerError("initial-registration-selector-inventory-invalid")
        for name, body in self.static_materials.items():
            if pins.get(name) != _sha(body):
                raise OperatorRunnerError(f"initial-registration-static-pin-mismatch:{name}")
        for name in ("task_input_manifest", "source_capture_manifest"):
            if type(pins.get(name)) is not str or not _SHA.fullmatch(pins[name]):
                raise OperatorRunnerError(f"initial-registration-dynamic-pin-invalid:{name}")
        development_receipt_bytes = None
        development_receipt_sha256 = self.config.get("development_receipt_sha256")
        development_stage_uuid = self.config.get("development_stage_uuid")
        receipt_path = self.private_paths.get("development_pass_receipt")
        if self.config["stage_kind"] == "confirmation":
            if receipt_path is None:
                if not preflight_only:
                    raise OperatorRunnerError("confirmation-development-receipt-required")
            else:
                development_receipt_bytes = _read_file(receipt_path, 64_000, private=True)
                if _sha(development_receipt_bytes) != development_receipt_sha256:
                    raise OperatorRunnerError("confirmation-development-receipt-pin-mismatch")
                try:
                    core._verify_development_receipt(
                        development_receipt_bytes,
                        expected_sha256=development_receipt_sha256,
                        current_stage_uuid=self.config["stage_uuid"],
                        expected_development_stage_uuid=development_stage_uuid,
                        forbidden_stage_uuids=tuple(self.config["forbidden_stage_uuids"]),
                        protocol_sha256=_sha(path_bytes["protocol"]),
                        source_revision=self.config["source_revision"],
                        source_closure_sha256=_sha(path_bytes["qualified_source_closure"]),
                    )
                except core.StudyError as exc:
                    raise OperatorRunnerError("confirmation-development-receipt-invalid") from exc
        elif receipt_path is not None:
            raise OperatorRunnerError("development-stage-receipt-unexpected")
        if preflight_only:
            # This path verifies configured roots and all source-bound plan
            # content without starting a clock, creating directories, or
            # exposing handoff/credential-capable runtime methods.
            _inspect_private_directories(self.directories)
            self.plan = orchestration.StagePlan(
                stage_uuid=self.config["stage_uuid"],
                stage_kind=self.config["stage_kind"],
                source_revision=self.config["source_revision"],
                protocol_bytes=path_bytes["protocol"],
                cohorts_bytes=path_bytes["cohorts"],
                acquisition_plan_bytes=acquisition_plan,
                acquisition_manifest_bytes=acquisition_manifest,
                source_closure_sha256=_sha(path_bytes["qualified_source_closure"]),
                research_cases=tuple(cases),
                navigation_targets=tuple(targets),
                candidate_identity_bytes=path_bytes["candidate_identity"],
                candidate_endpoint_sha256=endpoint_sha,
                packet_stage_uuid=self.config["packet_stage_uuid"],
                stage_started_monotonic=0.0,
                stage_deadline_monotonic=0.0,
                expected_acquisition_manifest_sha256=_sha(acquisition_manifest),
                pool_input_mode=pool_input_mode,
                band_companion_bytes=path_bytes.get("band_companion"),
                band_companion_sha256=companion_sha,
                initial_registration_bytes=self.initial_registration if companion_sha is not None else None,
                initial_registration_sha256=self.initial_registration_sha256 if companion_sha is not None else None,
                band_companion_source_pins=self.band_companion_source_pins,
                saved_origin=self.saved_origin,
                development_receipt_bytes=development_receipt_bytes,
                development_receipt_sha256=development_receipt_sha256,
                development_stage_uuid=development_stage_uuid,
            )
            self.preflight_validation = orchestration._validate_plan_static(self.plan)
            return
        # Directory preparation is deliberately after all registered static
        # pins, source bytes, and dynamic-pin shapes have been checked, but
        # before the stage clock or any phase/permit can begin.
        _prepare_private_directories(self.directories)
        self.started_monotonic = time.monotonic()
        self.started_utc = datetime.now(timezone.utc)
        self.deadline_monotonic = self.started_monotonic + orchestration.MAX_STAGE_WALL_SECONDS
        # Addition can round upward across a float boundary; retain the strict duration cap.
        while self.deadline_monotonic - self.started_monotonic > orchestration.MAX_STAGE_WALL_SECONDS:
            self.deadline_monotonic = math.nextafter(self.deadline_monotonic, self.started_monotonic)
        self.deadline_utc = self.started_utc + timedelta(seconds=orchestration.MAX_STAGE_WALL_SECONDS)
        self.plan = orchestration.StagePlan(
            stage_uuid=self.config["stage_uuid"],
            stage_kind=self.config["stage_kind"],
            source_revision=self.config["source_revision"],
            protocol_bytes=path_bytes["protocol"],
            cohorts_bytes=path_bytes["cohorts"],
            acquisition_plan_bytes=acquisition_plan,
            acquisition_manifest_bytes=acquisition_manifest,
            source_closure_sha256=_sha(path_bytes["qualified_source_closure"]),
            research_cases=tuple(cases),
            navigation_targets=tuple(targets),
            candidate_identity_bytes=path_bytes["candidate_identity"],
            candidate_endpoint_sha256=endpoint_sha,
            packet_stage_uuid=self.config["packet_stage_uuid"],
            stage_started_monotonic=self.started_monotonic,
            stage_deadline_monotonic=self.deadline_monotonic,
            expected_acquisition_manifest_sha256=_sha(acquisition_manifest),
            pool_input_mode=pool_input_mode,
            band_companion_bytes=path_bytes.get("band_companion"),
            band_companion_sha256=companion_sha,
            initial_registration_bytes=self.initial_registration if companion_sha is not None else None,
            initial_registration_sha256=self.initial_registration_sha256 if companion_sha is not None else None,
            band_companion_source_pins=self.band_companion_source_pins,
            saved_origin=self.saved_origin,
            development_receipt_bytes=development_receipt_bytes,
            development_receipt_sha256=development_receipt_sha256,
            development_stage_uuid=development_stage_uuid,
        )
        orchestration._validate_plan(self.plan)

    async def _request(
        self, scope: str, request_id: str, bindings: dict[str, object], *, artifacts: dict[str, bytes] | None = None
    ) -> tuple[bytes, str]:
        request_bindings, request_artifacts = self._phase_request_context(artifacts)
        for key, value in request_bindings.items():
            if key in bindings and bindings[key] != value:
                raise OperatorRunnerError("phase-request-context-binding-collision")
        bindings = {**bindings, **request_bindings}
        return await self.handoff.request_async(
            stage_uuid=self.plan.stage_uuid,
            scope=scope,
            request_id=request_id,
            bindings=bindings,
            deadline_monotonic=self.plan.stage_deadline_monotonic,
            artifacts=request_artifacts,
        )

    def _phase_request_context(self, artifacts: dict[str, bytes] | None = None):
        """Return the common immutable registration context used by every authority handoff."""
        bindings = {
            "source_closure_sha256": self.plan.source_closure_sha256,
            "active_python_environment": json.loads(self.active_environment_receipt_bytes),
            "active_python_environment_sha256": _sha(self.active_environment_receipt_bytes),
        }
        if self.plan.stage_kind == "confirmation":
            if (
                type(self.plan.development_receipt_bytes) is not bytes
                or type(self.plan.development_receipt_sha256) is not str
                or type(self.plan.development_stage_uuid) is not str
            ):
                raise OperatorRunnerError("confirmation-development-receipt-required")
            bindings.update(
                {
                    "development_receipt_sha256": self.plan.development_receipt_sha256,
                    "development_stage_uuid": self.plan.development_stage_uuid,
                }
            )
            artifacts = dict(artifacts or {})
            existing = artifacts.setdefault("development-pass-receipt", self.plan.development_receipt_bytes)
            if existing != self.plan.development_receipt_bytes:
                raise OperatorRunnerError("phase-artifact-development-receipt-collision")
        elif self.plan.development_receipt_bytes is not None:
            raise OperatorRunnerError("development-stage-receipt-unexpected")
        reference = None
        reference_bytes = self._saved_corpus_reference_bytes
        if self.plan.pool_input_mode == "saved-corpus/1" and reference_bytes is not None:
            try:
                reference = core._strict_json(reference_bytes, "saved-corpus-import-reference")
                origin = self.plan.saved_origin
                if (
                    type(reference) is not dict
                    or set(reference)
                    != {
                        "schema",
                        "stage_uuid",
                        "origin_stage_uuid",
                        "origin_package_sha256",
                        "import_receipt_sha256",
                        "capture_manifest_sha256",
                        "capture_inventory_sha256",
                        "source_rows",
                        "captured_contexts",
                        "failed_contexts",
                        "current_search_calls",
                        "current_capture_calls",
                    }
                    or core._canonical(reference) != reference_bytes
                    or reference.get("schema") != "coverage-saved-corpus-import-reference/1"
                    or reference.get("stage_uuid") != self.plan.stage_uuid
                    or origin is None
                    or reference.get("origin_stage_uuid") != origin.registration.stage_uuid
                    or reference.get("origin_package_sha256") != origin.expected_package_sha256
                    or type(reference.get("import_receipt_sha256")) is not str
                    or re.fullmatch(r"[0-9a-f]{64}", reference["import_receipt_sha256"]) is None
                    or any(
                        type(reference.get(name)) is not str or re.fullmatch(r"[0-9a-f]{64}", reference[name]) is None
                        for name in ("capture_manifest_sha256", "capture_inventory_sha256")
                    )
                    or any(
                        type(reference.get(name)) is not int or reference[name] < 0
                        for name in ("source_rows", "captured_contexts", "failed_contexts")
                    )
                    or reference["captured_contexts"] + reference["failed_contexts"] != reference["source_rows"]
                    or type(reference.get("current_search_calls")) is not int
                    or reference.get("current_search_calls") != 0
                    or type(reference.get("current_capture_calls")) is not int
                    or reference.get("current_capture_calls") != 0
                ):
                    raise ValueError("saved import reference does not match the registered stage")
            except Exception as exc:
                raise OperatorRunnerError("saved-corpus-import-reference-invalid") from exc
            bindings.update(
                {
                    "saved_corpus_import_receipt_sha256": reference["import_receipt_sha256"],
                    "origin_package_sha256": reference["origin_package_sha256"],
                }
            )
        if artifacts is not None or reference is not None:
            artifacts = dict(artifacts or {})
            fixed_materials = {
                "protocol": self.plan.protocol_bytes,
                "cohorts": self.plan.cohorts_bytes,
                "acquisition-plan": self.plan.acquisition_plan_bytes,
                "candidate-identity": self.plan.candidate_identity_bytes,
                "source-closure": self.static_materials["qualified_source_closure"],
                "initial-registration": self.initial_registration,
                "active-registration": self._active_registration_bytes or self.initial_registration,
            }
            for name, raw in fixed_materials.items():
                existing = artifacts.setdefault(name, raw)
                if existing != raw:
                    raise OperatorRunnerError("phase-artifact-static-material-collision")
            for name, raw in self.static_materials.items():
                artifact_name = f"static-{name.replace('_', '-')}"
                existing = artifacts.setdefault(artifact_name, raw)
                if existing != raw:
                    raise OperatorRunnerError("phase-artifact-static-material-collision")
            optional_materials = {
                "task-input-manifest": self._active_input_manifest_bytes,
                "frozen-pool-reference": self._frozen_pool_reference_bytes,
                "saved-corpus-import-reference": self._saved_corpus_reference_bytes,
                "acquisition-manifest": self.plan.acquisition_manifest_bytes,
                "stage-clock": self._frozen_stage_clock_bytes,
            }
            if self._pipeline_inputs is not None:
                optional_materials["capture-manifest"] = self._pipeline_inputs.get("capture_manifest_bytes")
            for name, raw in optional_materials.items():
                if type(raw) is bytes:
                    existing = artifacts.setdefault(name, raw)
                    if existing != raw:
                        raise OperatorRunnerError("phase-artifact-dynamic-material-collision")
            if reference is not None:
                import_receipt_bytes = reference["import_receipt_sha256"].encode("ascii")
                existing = artifacts.setdefault("saved-corpus-import-receipt-sha256", import_receipt_bytes)
                if existing != import_receipt_bytes:
                    raise OperatorRunnerError("phase-artifact-dynamic-material-collision")
        return bindings, artifacts

    async def _acquisition_authority(self, plan):
        manifest = core._strict_json(plan.acquisition_manifest_bytes, "acquisition-manifest")
        canonical_manifest_bytes = coverage_live_acquire._canonical(manifest)
        if canonical_manifest_bytes != plan.acquisition_manifest_bytes:
            raise OperatorRunnerError("acquisition-manifest-not-canonical")
        canonical_manifest_sha256 = _sha(canonical_manifest_bytes)
        receipt, pin = await self._request(
            "acquisition",
            "permit",
            {
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "source_closure_sha256": plan.source_closure_sha256,
                "protocol_sha256": _sha(plan.protocol_bytes),
                "cohorts_sha256": _sha(plan.cohorts_bytes),
                "input_manifest_sha256": manifest["input_manifest_sha256"],
                "acquisition_manifest_sha256": plan.expected_acquisition_manifest_sha256,
                "stage_manifest_sha256": canonical_manifest_sha256,
                "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
                **({"execution_profile": manifest["execution_profile"]} if "execution_profile" in manifest else {}),
            },
        )
        return {
            "permit_receipt_bytes": receipt,
            "expected_permit_receipt_sha256": pin,
            "permit_verifier": local_authority.AcquisitionReceiptVerifier(),
            "one_shot_lease": local_authority.FileOneShotLease(self.directories["lease_root"], scope="acquisition"),
            "receipt_directory": str(Path(self.directories["stage_inventory"]) / plan.stage_uuid / "acquisition"),
        }

    async def _import_saved_corpus(self, plan):
        from scripts import coverage_frozen_pool_collect, coverage_late_registration

        if plan.pool_input_mode != "saved-corpus/1" or plan.saved_origin is None:
            raise OperatorRunnerError("saved-corpus-mode-not-registered")
        clock = coverage_frozen_pool_collect.FrozenCollectionStageClock(
            stage_started_monotonic=plan.stage_started_monotonic,
            stage_deadline_monotonic=plan.stage_deadline_monotonic,
            stage_started_utc=self.started_utc.isoformat().replace("+00:00", "Z"),
            stage_deadline_utc=self.deadline_utc.isoformat().replace("+00:00", "Z"),
            clock_origin_sha256=coverage_frozen_pool_collect._local_clock_origin_sha256(),
        )
        self._frozen_stage_clock_bytes = coverage_frozen_pool_collect._canonical(
            coverage_frozen_pool_collect._validate_clock(clock, test_mode=False, test_pacer=None)
        )
        phase_bindings, phase_artifacts = self._phase_request_context({})
        imported = await coverage_late_registration.request_saved_corpus_import(
            exchange=self.handoff,
            plan=plan,
            expected_initial_registration_sha256=self.initial_registration_sha256,
            stage_clock_bytes=self._frozen_stage_clock_bytes,
            phase_bindings=phase_bindings,
            phase_artifacts=phase_artifacts,
        )
        if type(imported) is not saved_corpus.SavedCorpusImportResult:
            raise OperatorRunnerError("saved-corpus-import-result-invalid")
        receipt_directory = Path(self.directories["stage_inventory"]) / plan.stage_uuid / "saved-inputs"
        receipt_directory.mkdir(mode=0o700)
        orchestration._write_new(receipt_directory / "import-receipt.json", imported.import_receipt_bytes)
        capture_directory = Path(self.directories["capture_receipts"]) / plan.stage_uuid
        capture_directory.mkdir(mode=0o700)
        for name, raw in imported.context_artifacts.items():
            orchestration._write_new(capture_directory / name, raw)
        orchestration._write_new(capture_directory / "inventory.json", imported.capture_inventory_bytes)
        orchestration._write_new(capture_directory / "source-manifest.json", imported.capture_manifest_bytes)
        self._saved_corpus_reference_bytes = coverage_late_registration.saved_corpus_import_reference(
            imported, plan=plan
        )
        return orchestration.SavedCorpusStageEvidence(plan.saved_origin, imported, receipt_directory)

    async def _prepare_after_saved_corpus(self, plan, evidence, pipeline_inputs):
        if type(evidence) is not orchestration.SavedCorpusStageEvidence:
            raise OperatorRunnerError("saved-corpus-late-preflight-context-invalid")
        self._pipeline_inputs = dict(pipeline_inputs)
        phase_bindings, phase_artifacts = self._phase_request_context({})
        late = await request_late_registration(
            exchange=self.handoff,
            plan=plan,
            acquisition_evidence=None,
            saved_corpus_import=evidence.imported,
            pipeline_inputs=pipeline_inputs,
            static_materials=self.static_materials,
            initial_registration_bytes=self.initial_registration,
            expected_initial_registration_sha256=self.initial_registration_sha256,
            forbidden_stage_uuids=tuple(self.config["forbidden_stage_uuids"]),
            task_ids=tuple(row["task_id"] for row in plan.research_cases)
            + tuple(row["target_id"] for row in plan.navigation_targets),
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            stage_clock_bytes=self._frozen_stage_clock_bytes,
            phase_bindings=phase_bindings,
            phase_artifacts=phase_artifacts,
        )
        self._pin_active_registration(late)
        return late

    async def _frozen_pool_collection_authority(self, plan):
        if plan.pool_input_mode != "frozen-pool/1":
            raise OperatorRunnerError("frozen-pool-mode-not-registered")
        from scripts import coverage_frozen_pool_collect

        origin_sha = coverage_frozen_pool_collect._local_clock_origin_sha256()
        stage_clock = coverage_frozen_pool_collect.FrozenCollectionStageClock(
            stage_started_monotonic=plan.stage_started_monotonic,
            stage_deadline_monotonic=plan.stage_deadline_monotonic,
            stage_started_utc=self.started_utc.isoformat().replace("+00:00", "Z"),
            stage_deadline_utc=self.deadline_utc.isoformat().replace("+00:00", "Z"),
            clock_origin_sha256=origin_sha,
        )
        clock_doc = coverage_frozen_pool_collect._validate_clock(stage_clock, test_mode=False, test_pacer=None)
        clock_bytes = coverage_frozen_pool_collect._canonical(clock_doc)
        stage_clock_sha256 = _sha(clock_bytes)
        self._frozen_stage_clock_bytes = clock_bytes
        manifest = core._strict_json(plan.acquisition_manifest_bytes, "frozen-pool-acquisition-manifest")
        if type(manifest) is not dict or coverage_live_acquire._canonical(manifest) != plan.acquisition_manifest_bytes:
            raise OperatorRunnerError("frozen-pool-acquisition-manifest-invalid")
        candidate_identity_sha256 = _sha(plan.candidate_identity_bytes)
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "stage_kind": plan.stage_kind,
            "source_revision": plan.source_revision,
            "source_closure_sha256": plan.source_closure_sha256,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "input_manifest_sha256": manifest["input_manifest_sha256"],
            "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
            "legacy_manifest_sha256": plan.expected_acquisition_manifest_sha256,
            "pool_input_mode": plan.pool_input_mode,
            "stage_clock_sha256": stage_clock_sha256,
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
        }
        receipt, pin = await self._request(
            "frozen-pool-collection",
            "permit",
            bindings,
            artifacts={"stage-clock": coverage_frozen_pool_collect._canonical(clock_doc)},
        )
        return {
            "stage_clock": stage_clock,
            "candidate_identity_sha256": candidate_identity_sha256,
            "candidate_endpoint_sha256": plan.candidate_endpoint_sha256,
            "permit_receipt_bytes": receipt,
            "expected_permit_receipt_sha256": pin,
            "permit_verifier": local_authority.FrozenPoolCollectionReceiptVerifier(
                pool_input_mode=plan.pool_input_mode,
                stage_clock_sha256=stage_clock_sha256,
                candidate_identity_sha256=candidate_identity_sha256,
                candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
            ),
            "one_shot_lease": local_authority.FileOneShotLease(
                self.directories["lease_root"], scope="frozen-pool-collection"
            ),
            "stage_inventory_directory": str(Path(self.directories["stage_inventory"]) / plan.stage_uuid),
        }

    async def _verify_acquisition(self, _plan, evidence):
        return evidence.source_bound_receipt_status == "externally-verified"

    async def _capture_authority(self, plan, pipeline_inputs):
        manifest_bytes = pipeline_inputs["capture_manifest_bytes"]
        manifest = core._strict_json(manifest_bytes, "capture-manifest")
        endpoint = self.config["candidate_base_url"]
        _health, _scrape, endpoint_sha = capture._candidate_endpoints(endpoint)
        candidate = self.candidate_identity["runtime"]["revision"]
        ca_bundle_bytes = None
        ca_bundle_sha256 = None
        ca_bundle_path = self.private_paths.get("capture_ca_bundle")
        if ca_bundle_path is not None:
            ca_bundle_bytes = _read_file(ca_bundle_path, 262_144, private=True)
            ca_bundle_sha256, _ssl_context = capture._validated_ca_bundle(ca_bundle_bytes, _sha(ca_bundle_bytes))
        qual_bindings = capture.ProtectedCaptureQualificationBindings(
            stage_uuid=plan.stage_uuid,
            source_revision=plan.source_revision,
            protocol_sha256=_sha(plan.protocol_bytes),
            candidate_identity_sha256=_sha(plan.candidate_identity_bytes),
            candidate_endpoint_sha256=endpoint_sha,
            candidate_endpoint_scheme="https",
            candidate_runtime_revision=candidate,
            capture_module_sha256=hashlib.sha256(Path(capture.__file__).read_bytes()).hexdigest(),
            ca_bundle_sha256=ca_bundle_sha256,
        )
        qual_receipt, qual_pin = await self._request(
            "protected-source-capture",
            "permit",
            asdict(qual_bindings),
            artifacts={"capture-manifest": manifest_bytes},
        )
        capture_bindings = {
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
        permit, permit_pin = await self._request(
            "source-capture",
            "permit",
            capture_bindings,
            artifacts={"capture-manifest": manifest_bytes},
        )
        return {
            "candidate_base_url": endpoint,
            "operator_token": _secret(self.private_paths["candidate_operator_token"]),
            "permit_receipt_bytes": permit,
            "expected_permit_receipt_sha256": permit_pin,
            "permit_verifier": local_authority.SourceCaptureReceiptVerifier(),
            "one_shot_lease": local_authority.FileOneShotLease(self.directories["lease_root"], scope="source-capture"),
            "receipt_root": self.directories["capture_receipts"],
            "qualification_receipt_bytes": qual_receipt,
            "expected_qualification_receipt_sha256": qual_pin,
            "qualification_verifier": local_authority.ProtectedCaptureReceiptVerifier(),
            "ca_bundle_pem_bytes": ca_bundle_bytes,
            "expected_ca_bundle_sha256": ca_bundle_sha256,
        }

    async def _prepare_late(self, plan, acquisition_evidence, pipeline_inputs):
        self._pipeline_inputs = dict(pipeline_inputs)
        phase_bindings, phase_artifacts = self._phase_request_context({})
        evidence = await request_late_registration(
            exchange=self.handoff,
            plan=plan,
            acquisition_evidence=acquisition_evidence,
            pipeline_inputs=pipeline_inputs,
            static_materials=self.static_materials,
            initial_registration_bytes=self.initial_registration,
            expected_initial_registration_sha256=self.initial_registration_sha256,
            forbidden_stage_uuids=tuple(self.config["forbidden_stage_uuids"]),
            task_ids=tuple(row["task_id"] for row in plan.research_cases)
            + tuple(row["target_id"] for row in plan.navigation_targets),
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            stage_clock_bytes=self._frozen_stage_clock_bytes,
            phase_bindings=phase_bindings,
            phase_artifacts=phase_artifacts,
        )
        self._pin_active_registration(evidence)
        return evidence

    async def _prepare_after_frozen_pool(self, plan, frozen_evidence, pipeline_inputs):
        if type(frozen_evidence) is not orchestration.FrozenPoolStageEvidence:
            raise OperatorRunnerError("frozen-pool-late-preflight-context-invalid")
        self._pipeline_inputs = dict(pipeline_inputs)
        self._frozen_pool_reference_bytes = _canonical(
            {
                "schema": "coverage-frozen-pool-reference/1",
                "stage_uuid": plan.stage_uuid,
                "source_revision": plan.source_revision,
                "source_closure_sha256": plan.source_closure_sha256,
                "protocol_sha256": _sha(plan.protocol_bytes),
                "cohorts_sha256": _sha(plan.cohorts_bytes),
                "pool_manifest_sha256": frozen_evidence.phase_result.verified.pool_manifest_sha256,
                "snapshot_index_sha256": frozen_evidence.phase_result.verified.snapshot_index_sha256,
                "evidence_sha256": frozen_evidence.phase_result.verified.evidence_sha256,
                "availability_sha256": frozen_evidence.phase_result.verified.availability_sha256,
                "collector_receipt_sha256": frozen_evidence.phase_result.verified.collector_receipt_sha256,
                "proof_sha256": frozen_evidence.phase_result.verified.proof_sha256,
                "relative_inventory": "collection",
            }
        )
        phase_bindings, phase_artifacts = self._phase_request_context({})
        evidence = await request_late_registration(
            exchange=self.handoff,
            plan=plan,
            acquisition_evidence=None,
            verified_pool_source=frozen_evidence.phase_result.verified,
            pipeline_inputs=pipeline_inputs,
            static_materials=self.static_materials,
            initial_registration_bytes=self.initial_registration,
            expected_initial_registration_sha256=self.initial_registration_sha256,
            forbidden_stage_uuids=tuple(self.config["forbidden_stage_uuids"]),
            task_ids=tuple(row["task_id"] for row in plan.research_cases)
            + tuple(row["target_id"] for row in plan.navigation_targets),
            navigation_skip_reasons=tuple(row.get("skip_reason") for row in plan.navigation_targets),
            stage_clock_bytes=self._frozen_stage_clock_bytes,
            phase_bindings=phase_bindings,
            phase_artifacts=phase_artifacts,
        )
        self._pin_active_registration(evidence)
        return evidence

    def _pin_active_registration(self, evidence) -> None:
        registration_path = (
            Path(self.directories["operator_handoff"])
            / evidence.prepared.stage_uuid
            / "late-registration"
            / "responses"
            / "registration-1.receipt"
        )
        raw = _read_file(registration_path, 64_000_000, private=True)
        if _sha(raw) != evidence.preflight_receipt_sha256 or evidence.prepared.registration_sha256 != _sha(raw):
            raise OperatorRunnerError("late-registration-artifact-pin-mismatch")
        self._active_registration_bytes = raw
        self._active_input_manifest_bytes = evidence.input_manifest_bytes

    async def _verify_late(self, plan, evidence):
        return (
            evidence.prepared.stage_uuid == plan.stage_uuid
            and not evidence.prepared.fresh_execution_authorized
            and evidence.prepared.selector_input_map_schema == "coverage-selector-input-map/2-registered"
        )

    async def _selector_admission(self, plan, prepared, map_bytes, map_sha):
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "source_revision": plan.source_revision,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "registration_sha256": prepared.registration_sha256,
            "source_closure_sha256": plan.source_closure_sha256,
            "selector_input_map_sha256": map_sha,
        }
        receipt, pin = await self._request(
            "selector-map-admission", "map", bindings, artifacts={"selector-map": map_bytes}
        )
        return selector_admission.verify_selector_map_admission(
            receipt_bytes=receipt,
            expected_receipt_sha256=pin,
            protocol_bytes=plan.protocol_bytes,
            prepared=prepared,
            selector_input_map_sha256=map_sha,
        )

    async def _selector_permits(self, plan, prepared, map_bytes, map_sha, operation_materials):
        map_doc = core._strict_json(map_bytes, "selector-map")
        entries = map_doc.get("operations") if type(map_doc) is dict else None
        if type(entries) is not list:
            raise OperatorRunnerError("selector-map-operation-list-invalid")
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "registration_sha256": prepared.registration_sha256,
            "selector_input_map_sha256": map_sha,
            "operations": entries,
        }
        artifacts = {"selector-map": map_bytes}
        operation_artifact_rows = []
        legacy_rows = []
        for operation_id, material in operation_materials.items():
            operation_bytes = material.get("operation_input_bytes")
            if type(operation_bytes) is not bytes:
                raise OperatorRunnerError("selector-operation-artifact-invalid")
            suffix = hashlib.sha256(operation_id.encode()).hexdigest()[:16]
            input_name = f"input-{suffix}"
            artifacts[input_name] = operation_bytes
            operation_artifact_rows.append({"operation_id": operation_id, "input_artifact": input_name})
            legacy = material.get("legacy_control")
            if legacy is not None:
                if type(legacy) is not dict or set(legacy) != {
                    "service",
                    "query",
                    "canonical_pool",
                    "frozen_v1_source_bytes",
                    "current_service_source_bytes",
                }:
                    raise OperatorRunnerError("selector-legacy-artifact-invalid")
                for key, name in (
                    ("frozen_v1_source_bytes", "frozen-v1-source"),
                    ("current_service_source_bytes", "current-service-source"),
                ):
                    source_bytes = legacy[key]
                    if type(source_bytes) is not bytes:
                        raise OperatorRunnerError("selector-legacy-source-artifact-invalid")
                    existing = artifacts.setdefault(name, source_bytes)
                    if existing != source_bytes:
                        raise OperatorRunnerError("selector-legacy-source-artifact-mismatch")
                legacy_rows.append(
                    {
                        "operation_id": operation_id,
                        "query": legacy["query"],
                        "pool_reference": "frozen-pool-index",
                    }
                )
        artifacts["selector-artifact-index"] = _canonical(
            {
                "schema": "coverage-selector-artifact-index/1",
                "operations": operation_artifact_rows,
                "legacy_controls": legacy_rows,
            }
        )
        raw, pin = await self._request("selector-operation-permits", "batch", bindings, artifacts=artifacts)
        if _sha(raw) != pin:
            raise OperatorRunnerError("selector-permit-batch-pin-mismatch")
        batch = core._strict_json(raw, "selector-permit-batch")
        if (
            type(batch) is not dict
            or set(batch) != {"schema", "stage_uuid", "selector_input_map_sha256", "operations"}
            or batch.get("schema") != _BATCH_SCHEMA
            or batch.get("stage_uuid") != plan.stage_uuid
            or batch.get("selector_input_map_sha256") != map_sha
            or type(batch.get("operations")) is not list
        ):
            raise OperatorRunnerError("selector-permit-batch-binding-invalid")
        permits = {}
        for row in batch["operations"]:
            if type(row) is not dict or set(row) != {"operation_id", "permit_base64", "permit_sha256"}:
                raise OperatorRunnerError("selector-permit-row-invalid")
            operation_id = row["operation_id"]
            if operation_id in permits or operation_id not in operation_materials:
                raise OperatorRunnerError("selector-permit-operation-invalid")
            try:
                permit_bytes = base64.b64decode(row["permit_base64"], validate=True)
            except Exception as exc:
                raise OperatorRunnerError("selector-permit-base64-invalid") from exc
            if _sha(permit_bytes) != row["permit_sha256"]:
                raise OperatorRunnerError("selector-permit-digest-invalid")
            permits[operation_id] = (permit_bytes, row["permit_sha256"])
        if set(permits) != set(prepared.operation_ids):
            raise OperatorRunnerError("selector-permit-inventory-incomplete")
        return permits

    def _selector_evidence(self, plan, prepared, pipeline_inputs, reference_closure, results, terminal):
        operation_materials = pipeline_inputs.get("selector_operation_materials")
        task_input_manifest_bytes = pipeline_inputs.get("task_input_manifest_bytes")
        if type(operation_materials) is not dict or type(task_input_manifest_bytes) is not bytes:
            raise OperatorRunnerError("selector-evidence-source-context-missing")
        terminal_path = Path(self.directories["selector_results"]) / f"{prepared.stage_uuid}.terminal-inventory.json"
        terminal_bytes = _read_file(terminal_path, 4_000_000, private=True)
        operation_states = _verified_selector_operation_states(
            prepared=prepared,
            results=results,
            terminal=terminal,
            terminal_bytes=terminal_bytes,
        )
        input_manifest = core._strict_json(task_input_manifest_bytes, "selector-evidence-input-manifest")
        navigation_tasks = input_manifest.get("navigation_tasks") if type(input_manifest) is dict else None
        if type(navigation_tasks) is not list:
            raise OperatorRunnerError("selector-evidence-navigation-context-missing")
        by_id = {row.operation_id: row for row in results}
        orders = {}
        stability = {}
        for index, task in enumerate(pipeline_inputs["tasks"], start=1):
            task_id = task["task_id"]
            prefix = f"research-{index:02d}-"

            def order(operation_id, is_candidate):
                result = by_id[operation_id]
                found = (
                    result.ranking.ordered_ids
                    if is_candidate and result.ranking is not None
                    else result.native_ordered_ids
                )
                if found is None:
                    raise OperatorRunnerError("selector-order-missing")
                order = tuple(found)
                if not is_candidate:
                    # The registered original V1 control ranks only the first
                    # forty candidates. Its untouched native tail remains in
                    # the exact order present in the sealed task inventory.
                    material = operation_materials.get(operation_id)
                    legacy_control = material.get("legacy_control") if type(material) is dict else None
                    canonical_pool = legacy_control.get("canonical_pool") if type(legacy_control) is dict else None
                    if type(canonical_pool) not in {list, tuple}:
                        raise OperatorRunnerError("selector-w0-canonical-pool-missing")
                    expected_prefix = tuple(f"c{index}" for index in range(len(canonical_pool)))
                    if (
                        len(order) != len(expected_prefix)
                        or len(set(order)) != len(order)
                        or set(order) != set(expected_prefix)
                        or not set(order).issubset(set(task["native_order"]))
                    ):
                        raise OperatorRunnerError("selector-w0-prefix-invalid")
                    order += tuple(card_id for card_id in task["native_order"] if card_id not in order)
                if len(order) != len(task["native_order"]) or set(order) != set(task["native_order"]):
                    raise OperatorRunnerError("selector-order-not-complete-task-permutation")
                return order

            orders[task_id] = {
                "w0": order(prefix + "base-w0", False),
                "candidate": order(prefix + "base-candidate", True),
            }
            if index in {1, 4, 7, 8}:
                stability[task_id] = {
                    variant: {
                        "w0": order(prefix + variant + "-w0", False),
                        "candidate": order(prefix + variant + "-candidate", True),
                    }
                    for variant in ("repeat", "rotate")
                }
        rank_one = {}
        top_urls = {}
        for index, target in enumerate(plan.navigation_targets, start=1):
            operation_id = f"navigation-{index:02d}-w0"
            row = by_id[operation_id]
            ordered = row.native_ordered_ids
            if ordered is None:
                raise OperatorRunnerError("navigation-order-missing")
            nav = next(item for item in navigation_tasks if item["task_id"] == target["target_id"])
            rank_one[target["target_id"]] = ordered[0] == nav["target_candidate_id"]
            material = pipeline_inputs["selector_operation_materials"][operation_id]["operation_input_bytes"]
            doc = core._strict_json(material, "navigation-operation-input")
            candidate_by_id = {item["id"]: item["url"] for item in doc.get("candidates", [])}
            top_urls[target["target_id"]] = candidate_by_id[ordered[0]]
        usage_known = all(
            type(row.input_tokens_observed) is int and type(row.output_tokens_observed) is int for row in results
        )
        return orchestration.SelectorEvidence(
            stage_uuid=prepared.stage_uuid,
            terminal_inventory_sha256=terminal["sha256"],
            operation_rows=tuple(
                {
                    "operation_id": row.operation_id,
                    "state": operation_states[row.operation_id],
                    "request_sha256": row.request_sha256,
                    "response_sha256": row.response_sha256,
                    "input_tokens_observed": row.input_tokens_observed,
                    "output_tokens_observed": row.output_tokens_observed,
                }
                for row in results
            ),
            research_orders=orders,
            navigation_rank_one=rank_one,
            reference_grade_receipt_sha256=reference_closure.receipt_sha256,
            usage_status="known" if usage_known else "unknown",
            stability_orders=stability,
            navigation_top1_urls=top_urls,
            terminal_inventory_bytes=terminal_bytes,
            result_root=Path(self.directories["selector_results"]),
            archive_root=Path(self.directories["selector_archive"]),
        )

    async def _answer_authority(self, plan, tasks):
        endpoint = self.config["answer_endpoint"]
        allow_private_http = self.config.get("allow_trusted_private_http", False)
        endpoint_url, mode = answer._chat_url(endpoint, allow_trusted_private_http=allow_private_http)
        requests = answer._make_requests(tasks)
        _manifest, manifest_sha = answer._request_manifest(requests, endpoint_security_mode=mode)
        if mode == "trusted-private-http":
            _dial_url, _host_header, resolved_sha = await answer._resolve_private_http_destination(endpoint_url)
        else:
            resolved_sha = answer._sha(answer._canonical({"mode": "https-required"}))
        endpoint_sha = answer._sha(
            answer._canonical(
                {"endpoint": endpoint, "security_mode": mode, "resolved_destination_sha256": resolved_sha}
            )
        )
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "source_revision": plan.source_revision,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "cohorts_sha256": _sha(plan.cohorts_bytes),
            "operation_manifest_sha256": manifest_sha,
            "endpoint_sha256": endpoint_sha,
            "resolved_destination_sha256": resolved_sha,
            "endpoint_security_mode": mode,
            "operation_ids": tuple(row.operation_id for row in requests),
        }
        answer_artifacts = {
            "answer-request-manifest": _manifest,
            "answer-task-material": _answer_task_artifact_bytes(plan.stage_uuid, tuple(tasks)),
        }
        if getattr(plan, "pool_input_mode", None) in {"frozen-pool/1", "saved-corpus/1"}:
            capture_directory = Path(self.directories["capture_receipts"]) / plan.stage_uuid
            selector_terminal = (
                Path(self.directories["selector_results"]) / f"{plan.stage_uuid}.terminal-inventory.json"
            )
            capture_inventory = _read_file(capture_directory / "inventory.json", 4_000_000, private=True)
            selector_inventory = _read_file(selector_terminal, 4_000_000, private=True)
            answer_artifacts["answer-source-reference"] = _canonical(
                {
                    "schema": "coverage-answer-source-reference/1",
                    "stage_uuid": plan.stage_uuid,
                    "capture_inventory_sha256": _sha(capture_inventory),
                    "selector_terminal_inventory_sha256": _sha(selector_inventory),
                }
            )
        receipt, pin = await self._request("answer-execution", "permit", bindings, artifacts=answer_artifacts)
        return {
            # The permit binds the configured endpoint string. The executor
            # canonicalizes it with the same helper before dispatch; returning
            # that value here would make its endpoint digest differ for a
            # base URL or a `/v1` URL even though both resolve to this path.
            "endpoint": endpoint,
            "allow_trusted_private_http": allow_private_http,
            "api_key": _secret(self.private_paths["answer_api_key"]),
            "permit_bytes": receipt,
            "expected_permit_sha256": pin,
            "permit_verifier": local_authority.AnswerReceiptVerifier(),
            "one_shot_lease": local_authority.FileOneShotLease(
                self.directories["lease_root"], scope="answer-execution"
            ),
            "lease_root": self.directories["lease_root"],
            "archive_root": self.directories["answer_archive"],
            "result_root": self.directories["answer_results"],
            "stage_started_utc": self.started_utc.isoformat().replace("+00:00", "Z"),
            "stage_deadline_utc": self.deadline_utc.isoformat().replace("+00:00", "Z"),
            "answer_manifest_sha256": manifest_sha,
        }

    def runtime_bindings(self) -> runtime.RuntimeBindings:
        if getattr(self, "preflight_only", False):
            raise OperatorRunnerError("preflight-only-runtime-bindings-forbidden")
        return runtime.RuntimeBindings(
            acquisition_authority=self._acquisition_authority,
            verify_acquisition=self._verify_acquisition,
            capture_authority=self._capture_authority,
            prepare_after_acquisition=self._prepare_late,
            verify_late_preflight=self._verify_late,
            selector_permit_resolver=self._selector_permits,
            selector_admission_verifier=self._selector_admission,
            selector_evidence_builder=self._selector_evidence,
            answer_authority=self._answer_authority,
            selector_api_key=_secret(self.private_paths["selector_api_key"]),
            selector_lease_root=self.directories["lease_root"],
            selector_archive_root=self.directories["selector_archive"],
            selector_result_root=self.directories["selector_results"],
            native_handoff=native_handoff.NativeGraderHandoff(
                self.directories["grader_handoff"], require_native_host_transcripts=True
            ),
            frozen_pool_collection_authority=self._frozen_pool_collection_authority,
            prepare_after_frozen_pool=self._prepare_after_frozen_pool,
            import_saved_corpus=self._import_saved_corpus,
            prepare_after_saved_corpus=self._prepare_after_saved_corpus,
        )

    async def run(self) -> orchestration.StageResult:
        if getattr(self, "preflight_only", False):
            raise OperatorRunnerError("preflight-only-run-forbidden")
        return await orchestration.coordinate_coverage_stage(
            plan=self.plan,
            executors=runtime.build_stage_executors(self.plan, self.runtime_bindings()),
            inventory_root=self.directories["stage_inventory"],
        )


def main(argv: list[str] | None = None) -> int:
    import argparse
    import asyncio

    if not _source_bootstrap_ready():
        print(json.dumps({"status": "refused-or-terminal", "error_class": "source-bootstrap-required"}, sort_keys=True))
        return 1

    parser = argparse.ArgumentParser(description="Run one externally registered coverage development stage.")
    parser.add_argument("--config", required=True, help="private operator JSON config")
    parser.add_argument("--config-sha256", required=True, help="out-of-band SHA-256 pin for config bytes")
    parser.add_argument(
        "--preflight-only",
        action="store_true",
        help="validate registered static bindings without starting a stage, clock, or handoff",
    )
    args = parser.parse_args(argv)
    try:
        runner = (
            OperatorStageRunner.preflight_from_file(args.config, args.config_sha256)
            if args.preflight_only
            else OperatorStageRunner.from_file(args.config, args.config_sha256)
        )
        if args.preflight_only:
            print(
                json.dumps(
                    {
                        "status": "static-preflight-passed",
                        "source_revision": runner.plan.source_revision,
                        "config_sha256": args.config_sha256,
                        "registration_sha256": runner.initial_registration_sha256,
                    },
                    sort_keys=True,
                )
            )
            return 0
        result = asyncio.run(runner.run())
    except Exception as exc:
        # Keep all potentially sensitive details out of the terminal.
        error_key = _safe_preflight_error_code(exc) if args.preflight_only else ""
        error_detail = {"error_code": error_key} if args.preflight_only else {"error_class": type(exc).__name__}
        print(json.dumps({"status": "refused-or-terminal", **error_detail}, sort_keys=True))
        return 1
    print(
        json.dumps(
            {
                "status": result.status,
                "stage_uuid": result.stage_uuid,
                "inventory_sha256": result.inventory_sha256,
                "closeout_receipt_sha256": result.closeout_receipt_sha256,
                "product_authorized": False,
                "admission_created": False,
                "scientific_calls_made_by_coordinator": False,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
