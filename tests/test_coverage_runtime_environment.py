from __future__ import annotations

import asyncio
import hashlib
import importlib._bootstrap_external
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
import uuid
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import ModuleType, SimpleNamespace

import pytest
from packaging.markers import default_environment

from scripts import coverage_operator_runner as runner
from scripts import coverage_runtime_environment as runtime_environment

REPOSITORY = Path(__file__).resolve().parents[1]


def _lock() -> bytes:
    return (REPOSITORY / "uv.lock").read_bytes()


def _expected_distributions(lock_bytes: bytes) -> dict[str, str]:
    lock = tomllib.loads(lock_bytes.decode("utf-8"))
    environment = default_environment()
    return runtime_environment._locked_active_packages(lock, environment=environment)


def _receipt(distributions: dict[str, str] | None = None) -> bytes:
    lock = _lock()
    expected = distributions if distributions is not None else _expected_distributions(lock)
    return runtime_environment.verify_active_environment(
        lock,
        expected_lock_sha256=hashlib.sha256(lock).hexdigest(),
        python_executable="/tmp/study-env/bin/python",
        prefix="/tmp/study-env",
        base_prefix="/usr/local",
        installed_distributions=expected,
    )


def test_locked_environment_receipt_binds_compatible_isolated_interpreter_and_packages() -> None:
    receipt = json.loads(_receipt())
    assert receipt["schema"] == runtime_environment.SCHEMA
    assert receipt["lock_sha256"] == hashlib.sha256(_lock()).hexdigest()
    assert receipt["required_extras"] == ["dev"]
    assert receipt["python"]["prefix"] == str(Path("/tmp/study-env").resolve())
    assert receipt["distributions"]
    assert {item["name"] for item in receipt["distributions"]} == set(_expected_distributions(_lock()))
    assert "httptools" in {item["name"] for item in receipt["distributions"]}
    installed_names = {item["name"] for item in receipt["distributions"]}
    assert "httpx2-jsfetch" not in installed_names
    if sys.platform != "win32":
        assert not {"pywin32", "pywin32-ctypes"} & installed_names
    if sys.platform != "linux":
        assert "secretstorage" not in installed_names


@pytest.mark.parametrize(
    ("mutation", "error"),
    [
        ("missing", "active-environment-distribution-inventory-mismatch"),
        ("extra", "active-environment-distribution-inventory-mismatch"),
        ("version", "active-environment-distribution-version-mismatch"),
    ],
)
def test_locked_environment_rejects_missing_extra_or_drifted_distribution(mutation: str, error: str) -> None:
    lock = _lock()
    observed = _expected_distributions(lock)
    if mutation == "missing":
        observed.pop(next(iter(observed)))
    elif mutation == "extra":
        observed["unlocked-package"] = "1.0"
    else:
        observed["fastapi"] = "0.0.1"
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match=error):
        _receipt(observed)


def _write_dist_info(site: Path, name: str, version: str, suffix: str = "dist-info") -> None:
    metadata = site / f"{name}-{version}.{suffix}"
    metadata.mkdir(parents=True)
    (metadata / "METADATA").write_text(f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n", encoding="utf-8")


def test_runtime_inventory_ignores_only_pinned_checkout_metadata_and_rejects_external_paths(
    tmp_path: Path,
) -> None:
    prefix = tmp_path / "venv"
    site = prefix / "lib" / "python3.12" / "site-packages"
    site.mkdir(parents=True)
    project = tmp_path / "checkout"
    project.mkdir()
    _write_dist_info(site, "allowed-package", "1.0")
    _write_dist_info(project, runtime_environment.PROJECT_NAME, "0.6.0", "egg-info")
    assert runtime_environment._observed_distributions(prefix, project, search_path=[str(site), str(project)]) == {
        "allowed-package": "1.0"
    }

    external = tmp_path / "unexpected-site"
    external.mkdir()
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match="active-environment-import-path-unexpected"):
        runtime_environment._observed_distributions(
            prefix, project, search_path=[str(site), str(project), str(external)]
        )


def test_runtime_inventory_rejects_duplicate_installed_package_in_venv(
    tmp_path: Path,
) -> None:
    prefix = tmp_path / "venv"
    first = prefix / "lib" / "python3.12" / "site-packages"
    second = prefix / "lib" / "python3.12" / "extra-packages"
    first.mkdir(parents=True)
    second.mkdir(parents=True)
    project = tmp_path / "checkout"
    project.mkdir()
    _write_dist_info(first, "duplicate-package", "1.0")
    _write_dist_info(second, "duplicate-package", "1.0")
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match="active-environment-duplicate-distribution"):
        runtime_environment._observed_distributions(
            prefix, project, search_path=[str(first), str(second), str(project)]
        )


def test_runtime_inventory_rejects_distributions_on_global_site_path(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    base = tmp_path / "base-python"
    stdlib = base / "lib" / "python3.12"
    global_site = stdlib / "site-packages"
    global_site.mkdir(parents=True)
    prefix = tmp_path / "venv"
    venv_site = prefix / "lib" / "python3.12" / "site-packages"
    venv_site.mkdir(parents=True)
    project = tmp_path / "checkout"
    project.mkdir()
    _write_dist_info(global_site, "unlocked-global-package", "1.0")
    monkeypatch.setattr(sys, "base_prefix", str(base))
    monkeypatch.setattr(runtime_environment.sysconfig, "get_path", lambda _key: str(stdlib))

    with pytest.raises(
        runtime_environment.RuntimeEnvironmentError, match="active-environment-distribution-outside-venv"
    ):
        runtime_environment._observed_distributions(
            prefix, project, search_path=[str(venv_site), str(project), str(stdlib), str(global_site)]
        )


def test_runtime_inventory_rejects_extra_metadata_in_source_subpath(tmp_path: Path) -> None:
    prefix = tmp_path / "venv"
    site = prefix / "lib" / "python3.12" / "site-packages"
    site.mkdir(parents=True)
    project = tmp_path / "checkout"
    project.mkdir()
    scripts = project / "scripts"
    scripts.mkdir()
    _write_dist_info(scripts, "unlocked-source-package", "1.0")

    with pytest.raises(
        runtime_environment.RuntimeEnvironmentError, match="active-environment-distribution-outside-venv"
    ):
        runtime_environment._observed_distributions(
            prefix, project, search_path=[str(site), str(project), str(scripts)]
        )


def test_documented_source_launcher_accepts_real_locked_venv_with_source_egg_info(tmp_path: Path) -> None:
    if Path(sys.prefix).resolve() == Path(sys.base_prefix).resolve():
        pytest.skip("requires the uv-synced study venv; the default CI test interpreter is not a venv")
    python = Path(sys.executable)
    source_metadata = REPOSITORY / "slopsearx.egg-info"
    assert not source_metadata.exists(), "do not overwrite pre-existing source metadata"
    source_metadata.mkdir()
    (source_metadata / "PKG-INFO").write_text(
        "Metadata-Version: 2.1\nName: slopsearx\nVersion: 0.6.0\n", encoding="utf-8"
    )
    try:
        script = r"""
import hashlib, json, sys
from pathlib import Path
from scripts.coverage_operator_launcher import configure_source_import
cache = configure_source_import()
try:
    from scripts.coverage_runtime_environment import verify_active_environment
    lock = (Path.cwd() / "uv.lock").read_bytes()
    receipt = verify_active_environment(lock, expected_lock_sha256=hashlib.sha256(lock).hexdigest())
    print(json.dumps({"status": "verified", "receipt_sha256": hashlib.sha256(receipt).hexdigest()}))
finally:
    import shutil
    shutil.rmtree(cache, ignore_errors=True)
"""
        completed = subprocess.run(
            [str(python), "-c", script], cwd=REPOSITORY, check=False, capture_output=True, text=True, timeout=15
        )
        assert completed.returncode == 0, completed.stderr
        assert json.loads(completed.stdout)["status"] == "verified"
    finally:
        shutil.rmtree(source_metadata, ignore_errors=True)


def test_locked_environment_rejects_changed_lock_even_with_its_new_digest() -> None:
    lock = _lock().replace(b'version = "0.6.0"', b'version = "0.6.1"', 1)
    assert lock != _lock()
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match="active-environment-lock-pin-mismatch"):
        runtime_environment.verify_active_environment(
            lock,
            expected_lock_sha256=hashlib.sha256(_lock()).hexdigest(),
            prefix="/tmp/study-env",
            base_prefix="/usr/local",
            installed_distributions=_expected_distributions(_lock()),
        )


def test_locked_environment_requires_venv_and_python_in_lock_range() -> None:
    lock = _lock()
    expected = _expected_distributions(lock)
    kwargs = {
        "expected_lock_sha256": hashlib.sha256(lock).hexdigest(),
        "installed_distributions": expected,
    }
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match="active-environment-venv-required"):
        runtime_environment.verify_active_environment(
            lock,
            prefix="/usr/local",
            base_prefix="/usr/local",
            **kwargs,
        )
    with pytest.raises(runtime_environment.RuntimeEnvironmentError, match="active-environment-python-outside-lock"):
        runtime_environment.verify_active_environment(
            lock,
            prefix="/tmp/study-env",
            base_prefix="/usr/local",
            python_version=(3, 11, 9),
            **kwargs,
        )


def test_phase_handoff_carries_canonical_active_environment_receipt() -> None:
    instance = object.__new__(runner.OperatorStageRunner)
    receipt = _receipt()
    instance.active_environment_receipt_bytes = receipt
    stage_uuid = str(uuid.UUID(int=503))
    instance.plan = SimpleNamespace(stage_uuid=stage_uuid, stage_deadline_monotonic=99.0)

    class Handoff:
        received: dict[str, object] | None = None

        async def request_async(self, **kwargs):
            self.received = kwargs
            return b"synthetic receipt", "0" * 64

    handoff = Handoff()
    instance.handoff = handoff
    returned = asyncio.run(instance._request("preflight", "probe", {"binding": "fixture"}))
    assert returned == (b"synthetic receipt", "0" * 64)
    assert handoff.received is not None
    bindings = handoff.received["bindings"]
    assert bindings["active_python_environment"] == json.loads(receipt)
    assert bindings["active_python_environment_sha256"] == hashlib.sha256(receipt).hexdigest()
    assert bindings["binding"] == "fixture"

    with pytest.raises(runner.OperatorRunnerError, match="active-environment-binding-collision"):
        asyncio.run(instance._request("preflight", "probe-2", {"active_python_environment": {}}))


def test_loaded_module_code_check_rejects_stale_timestamp_pyc(tmp_path: Path) -> None:
    source_path = tmp_path / "pyc_probe.py"
    source = b"VALUE = 'source'\ndef selected():\n    return 'source'\n"
    source_path.write_bytes(source)
    timestamp = int(time.time()) - 2
    os.utime(source_path, (timestamp, timestamp))
    malicious = compile("VALUE = 'cached'\ndef selected():\n    return 'cached'\n", str(source_path), "exec")
    cache_path = Path(importlib.util.cache_from_source(str(source_path)))
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    cache_path.write_bytes(importlib._bootstrap_external._code_to_timestamp_pyc(malicious, timestamp, len(source)))

    name = "_coverage_integrity_pyc_probe"
    loader = SourceFileLoader(name, str(source_path))
    spec = importlib.util.spec_from_loader(name, loader)
    assert spec is not None
    module = ModuleType(name)
    module.__file__ = str(source_path)
    module.__loader__ = loader
    module.__spec__ = spec
    loader.exec_module(module)
    assert module.VALUE == "cached"
    assert module.selected() == "cached"
    with pytest.raises(runner.OperatorRunnerError, match="loaded-source-bytecode-mismatch"):
        runner._verify_loaded_module_code(name, module, source_path)


def test_loaded_module_code_check_rejects_replaced_critical_function(tmp_path: Path) -> None:
    source_path = tmp_path / "function_probe.py"
    source_path.write_text("def selected():\n    return 'source'\n", encoding="utf-8")
    name = "_coverage_integrity_function_probe"
    loader = SourceFileLoader(name, str(source_path))
    spec = importlib.util.spec_from_loader(name, loader)
    assert spec is not None
    module = ModuleType(name)
    module.__file__ = str(source_path)
    module.__loader__ = loader
    module.__spec__ = spec
    loader.exec_module(module)
    exec(compile("def selected():\n    return 'changed'\n", str(source_path), "exec"), vars(module))
    with pytest.raises(runner.OperatorRunnerError, match="loaded-source-function-mismatch"):
        runner._verify_loaded_module_code(name, module, source_path)


def test_loaded_module_code_check_rejects_foreign_filename_callable(tmp_path: Path) -> None:
    source_path = tmp_path / "bound_probe.py"
    source_path.write_text("def selected():\n    return 'source'\n", encoding="utf-8")
    name = "_coverage_integrity_foreign_probe"
    loader = SourceFileLoader(name, str(source_path))
    spec = importlib.util.spec_from_loader(name, loader)
    assert spec is not None
    module = ModuleType(name)
    module.__file__ = str(source_path)
    module.__loader__ = loader
    module.__spec__ = spec
    loader.exec_module(module)
    foreign = compile("def selected():\n    return 'foreign'\n", str(tmp_path / "foreign.py"), "exec")
    exec(foreign, vars(module))
    with pytest.raises(runner.OperatorRunnerError, match="loaded-source-function-mismatch"):
        runner._verify_loaded_module_code(name, module, source_path)


def test_source_launcher_ignores_poisoned_project_pyc_and_direct_runner_refuses(tmp_path: Path) -> None:
    source_path = Path(runner.__file__).resolve()
    cache_path = Path(importlib.util.cache_from_source(str(source_path)))
    original_cache = cache_path.read_bytes() if cache_path.exists() else None
    source_stat = source_path.stat()
    poison = compile("raise RuntimeError('poisoned runner bytecode')", str(source_path), "exec")
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    cache_path.write_bytes(
        importlib._bootstrap_external._code_to_timestamp_pyc(poison, int(source_stat.st_mtime), source_stat.st_size)
    )
    try:
        launcher = REPOSITORY / "scripts" / "coverage_operator_launcher.py"
        missing_config = tmp_path / "missing-config.json"
        launched = subprocess.run(
            [sys.executable, str(launcher), "--config", str(missing_config), "--config-sha256", "0" * 64],
            cwd=REPOSITORY,
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
        assert launched.returncode == 1
        assert json.loads(launched.stdout)["error_class"] == "OperatorRunnerError"
    finally:
        if original_cache is None:
            cache_path.unlink(missing_ok=True)
        else:
            cache_path.write_bytes(original_cache)
    direct = subprocess.run(
        [
            sys.executable,
            "-m",
            "scripts.coverage_operator_runner",
            "--config",
            str(missing_config),
            "--config-sha256",
            "0" * 64,
        ],
        cwd=REPOSITORY,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert direct.returncode == 1
    assert json.loads(direct.stdout)["error_class"] == "source-bootstrap-required"
