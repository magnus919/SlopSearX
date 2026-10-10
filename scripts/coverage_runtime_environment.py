"""Fail-closed binding of the active Python environment to ``uv.lock``.

The coverage-study runner is supported only from an isolated environment synced
from this repository's lock with its ``dev`` extra. This module reports the
observed interpreter and installed package versions; it does not install or
modify anything.
"""

from __future__ import annotations

import hashlib
import importlib.metadata
import json
import os
import platform
import re
import sys
import tomllib
from collections import deque
from collections.abc import Mapping
from pathlib import Path
from typing import Any, cast

from packaging.markers import Marker, default_environment
from packaging.specifiers import SpecifierSet
from packaging.utils import canonicalize_name
from packaging.version import Version

SCHEMA = "coverage-active-python-environment/1"
PROJECT_NAME = "slopsearx"
REQUIRED_EXTRAS = ("dev",)
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class RuntimeEnvironmentError(RuntimeError):
    """The interpreter or installed environment is not the locked study env."""


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _dependency_active(dependency: Mapping[str, Any], environment: Mapping[str, str], extra: str = "") -> bool:
    marker = dependency.get("marker")
    if marker is None:
        return True
    if type(marker) is not str:
        raise RuntimeEnvironmentError("locked-dependency-marker-invalid")
    try:
        return cast(bool, Marker(marker).evaluate({**environment, "extra": extra}))
    except (TypeError, ValueError) as exc:
        raise RuntimeEnvironmentError("locked-dependency-marker-invalid") from exc


def _locked_active_packages(lock: Mapping[str, Any], *, environment: Mapping[str, str]) -> dict[str, str]:
    packages = lock.get("package")
    if type(packages) is not list:
        raise RuntimeEnvironmentError("lock-package-inventory-invalid")
    by_name: dict[str, Mapping[str, Any]] = {}
    for package in packages:
        if type(package) is not dict or type(package.get("name")) is not str:
            raise RuntimeEnvironmentError("lock-package-entry-invalid")
        normalized_name = canonicalize_name(package["name"])
        if normalized_name in by_name:
            raise RuntimeEnvironmentError("lock-duplicate-package-name")
        by_name[normalized_name] = package

    if PROJECT_NAME not in by_name:
        raise RuntimeEnvironmentError("lock-project-package-missing")
    active: dict[str, str] = {}
    requested_extras: dict[str, set[str]] = {}
    queue: deque[tuple[str, tuple[str, ...]]] = deque([(PROJECT_NAME, ())])

    def enqueue_dependency(dependency: Mapping[str, Any], *, extra_context: str = "") -> None:
        if not _dependency_active(dependency, environment, extra_context):
            return
        dependency_name = dependency.get("name")
        if type(dependency_name) is not str:
            raise RuntimeEnvironmentError("locked-dependency-name-invalid")
        dependency_extras = dependency.get("extra", dependency.get("extras", []))
        if type(dependency_extras) is str:
            dependency_extras = [dependency_extras]
        if type(dependency_extras) is not list or any(type(item) is not str for item in dependency_extras):
            raise RuntimeEnvironmentError("locked-dependency-extras-invalid")
        queue.append((canonicalize_name(dependency_name), tuple(dependency_extras)))

    # The operator runner imports jsonschema and its declared execution tools
    # from the project's sole optional group. Require the complete locked group
    # so an incomplete or independently augmented venv cannot masquerade as it.
    root = by_name[PROJECT_NAME]
    root_dependencies = root.get("dependencies", [])
    if type(root_dependencies) is not list:
        raise RuntimeEnvironmentError("lock-project-dependencies-invalid")
    for dependency in root_dependencies:
        if type(dependency) is not dict:
            raise RuntimeEnvironmentError("lock-dependency-entry-invalid")
        enqueue_dependency(dependency)
    optional_groups = root.get("optional-dependencies", {})
    if type(optional_groups) is not dict:
        raise RuntimeEnvironmentError("lock-project-extras-invalid")
    for extra in REQUIRED_EXTRAS:
        group = optional_groups.get(extra)
        if type(group) is not list:
            raise RuntimeEnvironmentError("lock-required-extra-missing")
        for dependency in group:
            if type(dependency) is not dict:
                raise RuntimeEnvironmentError("lock-dependency-entry-invalid")
            enqueue_dependency(dependency, extra_context=extra)

    while queue:
        package_name, extras = queue.popleft()
        package = by_name.get(package_name)
        if package is None:
            raise RuntimeEnvironmentError("locked-dependency-package-missing")
        version = package.get("version")
        if type(version) is not str:
            raise RuntimeEnvironmentError("lock-package-version-invalid")
        try:
            Version(version)
        except Exception as exc:
            raise RuntimeEnvironmentError("lock-package-version-invalid") from exc
        prior = active.get(package_name)
        if prior is not None and prior != version:
            raise RuntimeEnvironmentError("lock-active-version-ambiguous")
        active[package_name] = version
        known_extras = requested_extras.setdefault(package_name, set())
        new_extras = set(extras) - known_extras
        if prior is not None and not new_extras:
            continue
        known_extras.update(new_extras)
        dependencies = package.get("dependencies", [])
        if type(dependencies) is not list:
            raise RuntimeEnvironmentError("lock-package-dependencies-invalid")
        for dependency in dependencies:
            if type(dependency) is not dict:
                raise RuntimeEnvironmentError("lock-dependency-entry-invalid")
            enqueue_dependency(dependency)
        groups = package.get("optional-dependencies", {})
        if type(groups) is not dict:
            raise RuntimeEnvironmentError("lock-package-extras-invalid")
        for extra in new_extras:
            group = groups.get(extra, [])
            if type(group) is not list:
                raise RuntimeEnvironmentError("lock-package-extra-invalid")
            for dependency in group:
                if type(dependency) is not dict:
                    raise RuntimeEnvironmentError("lock-dependency-entry-invalid")
                enqueue_dependency(dependency, extra_context=extra)
    return active


def verify_active_environment(
    lock_bytes: bytes,
    *,
    expected_lock_sha256: str,
    python_executable: str | os.PathLike[str] | None = None,
    prefix: str | os.PathLike[str] | None = None,
    base_prefix: str | os.PathLike[str] | None = None,
    python_version: tuple[int, int, int] | None = None,
    installed_distributions: Mapping[str, str] | None = None,
    marker_environment: Mapping[str, str] | None = None,
) -> bytes:
    """Validate the isolated active interpreter and return its canonical receipt."""
    if (
        type(lock_bytes) is not bytes
        or type(expected_lock_sha256) is not str
        or not _SHA256.fullmatch(expected_lock_sha256)
        or _sha(lock_bytes) != expected_lock_sha256
    ):
        raise RuntimeEnvironmentError("active-environment-lock-pin-mismatch")
    try:
        lock = tomllib.loads(lock_bytes.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise RuntimeEnvironmentError("active-environment-lock-invalid") from exc
    if type(lock) is not dict or type(lock.get("requires-python")) is not str:
        raise RuntimeEnvironmentError("active-environment-lock-invalid")

    active_prefix = Path(prefix if prefix is not None else sys.prefix).resolve()
    base = Path(base_prefix if base_prefix is not None else sys.base_prefix).resolve()
    if active_prefix == base:
        raise RuntimeEnvironmentError("active-environment-venv-required")
    current_python = python_version or (sys.version_info.major, sys.version_info.minor, sys.version_info.micro)
    if (
        type(current_python) is not tuple
        or len(current_python) != 3
        or any(type(part) is not int or part < 0 for part in current_python)
    ):
        raise RuntimeEnvironmentError("active-environment-interpreter-invalid")
    try:
        if Version(".".join(map(str, current_python))) not in SpecifierSet(lock["requires-python"]):
            raise RuntimeEnvironmentError("active-environment-python-outside-lock")
    except ValueError as exc:
        raise RuntimeEnvironmentError("active-environment-python-requirement-invalid") from exc
    if sys.implementation.name != "cpython":
        raise RuntimeEnvironmentError("active-environment-implementation-unsupported")

    environment: dict[str, str] = {
        str(key): str(value) for key, value in (marker_environment or default_environment()).items()
    }
    environment["python_version"] = f"{current_python[0]}.{current_python[1]}"
    environment["python_full_version"] = ".".join(map(str, current_python))
    environment["implementation_name"] = sys.implementation.name
    environment["platform_python_implementation"] = platform.python_implementation()
    expected = _locked_active_packages(lock, environment=environment)
    if installed_distributions is None:
        observed: dict[str, str] = {}
        duplicates: set[str] = set()
        try:
            for distribution in importlib.metadata.distributions():
                name = distribution.metadata.get("Name")
                if not name:
                    continue
                normalized = canonicalize_name(name)
                if normalized in observed:
                    duplicates.add(normalized)
                observed[normalized] = distribution.version
        except (OSError, ValueError) as exc:
            raise RuntimeEnvironmentError("active-environment-distribution-read-failed") from exc
        if duplicates:
            raise RuntimeEnvironmentError("active-environment-duplicate-distribution")
    else:
        observed = {canonicalize_name(name): version for name, version in installed_distributions.items()}
    if set(observed) != set(expected):
        raise RuntimeEnvironmentError("active-environment-distribution-inventory-mismatch")
    if any(type(observed.get(name)) is not str or observed[name] != version for name, version in expected.items()):
        raise RuntimeEnvironmentError("active-environment-distribution-version-mismatch")

    interpreter = Path(python_executable if python_executable is not None else sys.executable).resolve()
    receipt = {
        "schema": SCHEMA,
        "lock_sha256": expected_lock_sha256,
        "required_extras": list(REQUIRED_EXTRAS),
        "python": {
            "implementation": sys.implementation.name,
            "version": ".".join(map(str, current_python)),
            "cache_tag": sys.implementation.cache_tag,
            "executable": str(interpreter),
            "prefix": str(active_prefix),
            "base_prefix": str(base),
        },
        "platform": {
            "system": platform.system(),
            "machine": platform.machine(),
            "sys_platform": environment.get("sys_platform"),
        },
        "distributions": [{"name": name, "version": expected[name]} for name in sorted(expected)],
    }
    return _canonical(receipt)
