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
import os
import re
import stat
import subprocess
import sys
import time
import types
import uuid
from dataclasses import asdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from scripts import coverage_answer_execution as answer
from scripts import coverage_grade_closure, coverage_jev_execution, coverage_legacy_control, coverage_selector_input_map
from scripts import coverage_local_authority as local_authority
from scripts import coverage_native_grader_handoff as native_handoff
from scripts import coverage_native_host_dispatch as native_host_dispatch
from scripts import coverage_operator_handoff as operator_handoff
from scripts import coverage_runtime_environment as runtime_environment
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
_OPTIONAL_CONFIG_KEYS = {"allow_trusted_private_http"}
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
_PRIVATE_PATH_KEYS = {"candidate_operator_token", "selector_api_key", "answer_api_key"}
_OPTIONAL_PRIVATE_PATH_KEYS = {"capture_ca_bundle"}
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
_REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
_CHECKOUT_SOURCE_PATHS = {
    "coverage_source": Path(coverage_impl.__file__).resolve(),
    "production_rerank_source": Path(production_rerank_impl.__file__).resolve(),
    "dependency_lock": _REPOSITORY_ROOT / "uv.lock",
}


class OperatorRunnerError(RuntimeError):
    """The explicit operator config or its verified bindings are invalid."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


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
        set(value["paths"]) != _PATH_KEYS
        or not _PRIVATE_PATH_KEYS <= set(value["private_paths"])
        or set(value["private_paths"]) - _PRIVATE_PATH_KEYS - _OPTIONAL_PRIVATE_PATH_KEYS
        or set(value["directories"]) != _DIRECTORY_KEYS
    ):
        raise OperatorRunnerError("config-path-inventory-invalid")
    if "allow_trusted_private_http" in value and type(value["allow_trusted_private_http"]) is not bool:
        raise OperatorRunnerError("config-private-http-option-invalid")
    for key in (*_PATH_KEYS, *_PRIVATE_PATH_KEYS, *_OPTIONAL_PRIVATE_PATH_KEYS, *_DIRECTORY_KEYS):
        collection = (
            value["paths"]
            if key in _PATH_KEYS
            else value["private_paths"]
            if key in _PRIVATE_PATH_KEYS or key in _OPTIONAL_PRIVATE_PATH_KEYS
            else value["directories"]
        )
        if key in _OPTIONAL_PRIVATE_PATH_KEYS and key not in collection:
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


class OperatorStageRunner:
    """Loads a pinned registered bundle and invokes only the fixed phase graph."""

    def __init__(self, config: dict[str, Any], *, config_sha256: str):
        _require_source_bootstrap()
        self.config = config
        self.config_sha256 = config_sha256
        self.paths = config["paths"]
        self.private_paths = config["private_paths"]
        self.directories = config["directories"]
        self._build_plan()
        self.handoff = operator_handoff.OperatorReceiptHandoff(self.directories["operator_handoff"])

    @classmethod
    def from_file(cls, path: str | os.PathLike[str], expected_sha256: str) -> "OperatorStageRunner":
        return cls(_load_config(path, expected_sha256), config_sha256=expected_sha256)

    def _build_plan(self) -> None:
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
            or closure.get("schema") != "coverage-study-qualified-source-closure/1"
            or type(identity.get("runtime")) is not dict
            or type(identity["runtime"].get("revision")) is not str
        ):
            raise OperatorRunnerError("registered-source-materials-required")
        stage_row = _stage_cohort(cohorts, self.config["stage_kind"])
        cases = stage_row.get("research_cases")
        targets = stage_row.get("navigation_targets")
        if type(cases) is not list or type(targets) is not list:
            raise OperatorRunnerError("cohort-inventory-invalid")
        acquisition_plan = _build_acquisition_plan(self.config["stage_uuid"], cases, targets)
        if protocol.get("cohorts_sha256") != _sha(path_bytes["cohorts"]):
            raise OperatorRunnerError("protocol-cohort-pin-mismatch")
        try:
            core._verify_qualified_source_closure(
                path_bytes["qualified_source_closure"],
                expected_revision=self.config["source_revision"],
                expected_pins={
                    "coverage_source": _sha(path_bytes["coverage_source"]),
                    "production_rerank_source": _sha(path_bytes["production_rerank_source"]),
                    "dependency_lock": _sha(path_bytes["dependency_lock"]),
                },
            )
        except Exception as exc:
            raise OperatorRunnerError("source-closure-preflight-failed") from exc
        try:
            self.active_environment_receipt_bytes = runtime_environment.verify_active_environment(
                path_bytes["dependency_lock"],
                expected_lock_sha256=closure["material_pins"]["dependency_lock"],
            )
        except runtime_environment.RuntimeEnvironmentError as exc:
            raise OperatorRunnerError("active-environment-preflight-failed") from exc
        endpoint = self.config["candidate_base_url"]
        _health_url, _scrape_url, endpoint_sha = capture._candidate_endpoints(endpoint)
        preacq = _canonical(
            {
                "schema": "coverage-preacquisition-input-manifest/1",
                "stage_uuid": self.config["stage_uuid"],
                "protocol_sha256": _sha(path_bytes["protocol"]),
                "cohorts_sha256": _sha(path_bytes["cohorts"]),
                "task_ids": [row["task_id"] for row in cases] + [row["target_id"] for row in targets],
            }
        )
        acquisition_manifest = _canonical(
            {
                "schema": "coverage-live-acquisition-manifest/1",
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
        self.initial_registration_sha256 = _sha(self.initial_registration)
        if self.initial_registration_sha256 != self.config["initial_registration_sha256"]:
            raise OperatorRunnerError("initial-registration-config-pin-mismatch")
        self.candidate_identity = identity
        # Verify every static registration pin before taking the stage clock or
        # creating operator/lease directories. Dynamic pins are replaced only
        # by the separately pinned late registration after acquisition.
        self.initial_doc = core._strict_json(self.initial_registration, "initial-registration")
        if type(self.initial_doc) is not dict or self.initial_doc.get("stage_uuid") != self.config["stage_uuid"]:
            raise OperatorRunnerError("initial-registration-stage-mismatch")
        pins = self.initial_doc.get("pins")
        if type(pins) is not dict:
            raise OperatorRunnerError("initial-registration-pins-invalid")
        for name, body in self.static_materials.items():
            if pins.get(name) != _sha(body):
                raise OperatorRunnerError(f"initial-registration-static-pin-mismatch:{name}")
        for name in ("task_input_manifest", "source_capture_manifest"):
            if type(pins.get(name)) is not str or not _SHA.fullmatch(pins[name]):
                raise OperatorRunnerError(f"initial-registration-dynamic-pin-invalid:{name}")
        self.started_monotonic = time.monotonic()
        self.started_utc = datetime.now(timezone.utc)
        self.deadline_monotonic = self.started_monotonic + orchestration.MAX_STAGE_WALL_SECONDS
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
        )
        orchestration._validate_plan(self.plan)

    async def _request(self, scope: str, request_id: str, bindings: dict[str, object]) -> tuple[bytes, str]:
        if "active_python_environment" in bindings or "active_python_environment_sha256" in bindings:
            raise OperatorRunnerError("active-environment-binding-collision")
        bindings = {
            **bindings,
            "active_python_environment": json.loads(self.active_environment_receipt_bytes),
            "active_python_environment_sha256": _sha(self.active_environment_receipt_bytes),
        }
        return await self.handoff.request_async(
            stage_uuid=self.plan.stage_uuid,
            scope=scope,
            request_id=request_id,
            bindings=bindings,
            deadline_monotonic=self.plan.stage_deadline_monotonic,
        )

    async def _acquisition_authority(self, plan):
        manifest = core._strict_json(plan.acquisition_manifest_bytes, "acquisition-manifest")
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
                "acquisition_plan_sha256": _sha(plan.acquisition_plan_bytes),
            },
        )
        return {
            "permit_receipt_bytes": receipt,
            "expected_permit_receipt_sha256": pin,
            "permit_verifier": local_authority.AcquisitionReceiptVerifier(),
            "one_shot_lease": local_authority.FileOneShotLease(self.directories["lease_root"], scope="acquisition"),
            "receipt_directory": str(Path(self.directories["stage_inventory"]) / plan.stage_uuid / "acquisition"),
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
        qual_receipt, qual_pin = await self._request("protected-source-capture", "permit", asdict(qual_bindings))
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
        permit, permit_pin = await self._request("source-capture", "permit", capture_bindings)
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
        return await request_late_registration(
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
        )

    async def _verify_late(self, plan, evidence):
        return (
            evidence.prepared.stage_uuid == plan.stage_uuid
            and not evidence.prepared.fresh_execution_authorized
            and evidence.prepared.selector_input_map_schema == "coverage-selector-input-map/2-registered"
        )

    async def _selector_admission(self, plan, prepared, map_bytes, map_sha, _operation_materials):
        bindings = {
            "stage_uuid": plan.stage_uuid,
            "source_revision": plan.source_revision,
            "protocol_sha256": _sha(plan.protocol_bytes),
            "registration_sha256": prepared.registration_sha256,
            "source_closure_sha256": plan.source_closure_sha256,
            "selector_input_map_sha256": map_sha,
        }
        receipt, pin = await self._request("selector-map-admission", "map", bindings)
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
        raw, pin = await self._request("selector-operation-permits", "batch", bindings)
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
                return tuple(found)

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
            nav = next(item for item in pipeline_inputs["navigation_tasks"] if item["task_id"] == target["target_id"])
            rank_one[target["target_id"]] = ordered[0] == nav["target_candidate_id"]
            material = pipeline_inputs["selector_operation_materials"][operation_id]["operation_input_bytes"]
            doc = core._strict_json(material, "navigation-operation-input")
            candidate_by_id = {item["id"]: item["url"] for item in doc.get("candidates", [])}
            top_urls[target["target_id"]] = candidate_by_id[ordered[0]]
        terminal_path = Path(self.directories["selector_results"]) / f"{prepared.stage_uuid}.terminal-inventory.json"
        terminal_bytes = _read_file(terminal_path, 4_000_000, private=True)
        usage_known = all(
            type(row.input_tokens_observed) is int and type(row.output_tokens_observed) is int for row in results
        )
        return orchestration.SelectorEvidence(
            stage_uuid=prepared.stage_uuid,
            terminal_inventory_sha256=terminal["sha256"],
            operation_rows=tuple(
                {
                    "operation_id": row.operation_id,
                    "state": row.status,
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
        receipt, pin = await self._request("answer-execution", "permit", bindings)
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
        )

    async def run(self) -> orchestration.StageResult:
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
    args = parser.parse_args(argv)
    try:
        runner = OperatorStageRunner.from_file(args.config, args.config_sha256)
        result = asyncio.run(runner.run())
    except Exception as exc:
        # Keep all potentially sensitive details out of the terminal.
        print(json.dumps({"status": "refused-or-terminal", "error_class": type(exc).__name__}, sort_keys=True))
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
