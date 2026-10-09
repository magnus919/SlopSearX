#!/usr/bin/env python3
"""Enable Google's public Docker Hub pull-through cache on hosted Linux CI only."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable, Mapping

MIRROR = "https://mirror.gcr.io"
DAEMON_CONFIG = Path("/etc/docker/daemon.json")


def hosted_linux_runner(environment: Mapping[str, str]) -> bool:
    return (
        environment.get("GITHUB_ACTIONS") == "true"
        and environment.get("RUNNER_ENVIRONMENT") == "github-hosted"
        and environment.get("RUNNER_OS") == "Linux"
    )


def merge_daemon_config(raw: bytes | None) -> bytes:
    if raw is None or not raw.strip():
        settings: object = {}
    else:
        settings = json.loads(raw)
    if type(settings) is not dict:
        raise ValueError("Docker daemon config must be a JSON object")
    mirrors = settings.get("registry-mirrors", [])
    if type(mirrors) is not list or any(type(item) is not str or not item for item in mirrors):
        raise ValueError("Docker registry-mirrors must be a list of nonempty strings")
    merged = dict(settings)
    if MIRROR not in mirrors:
        merged["registry-mirrors"] = [*mirrors, MIRROR]
    return (json.dumps(merged, indent=2, sort_keys=True) + "\n").encode()


def configure_daemon(
    config_path: Path,
    *,
    restart: Callable[[], None],
    has_running_containers: Callable[[], bool],
) -> bool:
    """Atomically preserve existing daemon settings and restart only on change."""
    try:
        previous = config_path.read_bytes()
    except FileNotFoundError:
        previous = None
    updated = merge_daemon_config(previous)
    if previous is not None:
        try:
            already_configured = json.loads(previous) == json.loads(updated)
        except (json.JSONDecodeError, UnicodeDecodeError):
            already_configured = False
        if already_configured:
            return False
    if has_running_containers():
        raise RuntimeError("refusing to restart Docker while containers are running")

    config_path.parent.mkdir(parents=True, exist_ok=True)
    mode = config_path.stat().st_mode & 0o777 if config_path.exists() else 0o644
    fd, temp_name = tempfile.mkstemp(prefix=".daemon.json.", dir=config_path.parent)
    temp_path = Path(temp_name)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(updated)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temp_path, mode)
        os.replace(temp_path, config_path)
        directory_fd = os.open(config_path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temp_path.unlink(missing_ok=True)
    restart()
    return True


def configure_for_runner(
    environment: Mapping[str, str],
    config_path: Path,
    *,
    restart: Callable[[], None],
    has_running_containers: Callable[[], bool],
) -> bool:
    if not hosted_linux_runner(environment):
        return False
    return configure_daemon(
        config_path,
        restart=restart,
        has_running_containers=has_running_containers,
    )


def _restart_docker() -> None:
    subprocess.run(["systemctl", "restart", "docker"], check=True)


def _has_running_containers() -> bool:
    result = subprocess.run(["docker", "ps", "-q"], check=True, capture_output=True, text=True)
    return bool(result.stdout.strip())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--as-root", action="store_true")
    arguments = parser.parse_args()
    if not hosted_linux_runner(os.environ):
        print("Docker Hub mirror setup skipped outside GitHub-hosted Linux.")
        return 0
    if not arguments.as_root:
        subprocess.run(
            [
                "sudo",
                "--preserve-env=GITHUB_ACTIONS,RUNNER_ENVIRONMENT,RUNNER_OS",
                sys.executable,
                str(Path(__file__).resolve()),
                "--as-root",
            ],
            check=True,
        )
        return 0
    changed = configure_for_runner(
        os.environ,
        DAEMON_CONFIG,
        restart=_restart_docker,
        has_running_containers=_has_running_containers,
    )
    print("Docker Hub mirror configured." if changed else "Docker Hub mirror already configured.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
