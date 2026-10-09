from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from scripts import configure_ci_dockerhub_mirror as mirror


def test_hosted_linux_only_and_config_preservation(tmp_path: Path) -> None:
    hosted = {
        "GITHUB_ACTIONS": "true",
        "RUNNER_ENVIRONMENT": "github-hosted",
        "RUNNER_OS": "Linux",
    }
    assert mirror.hosted_linux_runner(hosted)
    for changed in (
        {**hosted, "GITHUB_ACTIONS": "false"},
        {**hosted, "RUNNER_ENVIRONMENT": "self-hosted"},
        {**hosted, "RUNNER_OS": "macOS"},
    ):
        assert not mirror.hosted_linux_runner(changed)

    path = tmp_path / "docker" / "daemon.json"
    path.parent.mkdir()
    path.write_text(json.dumps({"debug": True, "insecure-registries": ["registry.example:5000"]}))
    restarts: list[bool] = []
    assert mirror.configure_daemon(
        path, restart=lambda: restarts.append(True), has_running_containers=lambda: False
    )
    config = json.loads(path.read_text())
    assert config["debug"] is True
    assert config["insecure-registries"] == ["registry.example:5000"]
    assert config["registry-mirrors"] == [mirror.MIRROR]
    assert restarts == [True]
    assert not mirror.configure_daemon(
        path, restart=lambda: restarts.append(True), has_running_containers=lambda: False
    )
    assert restarts == [True]


def test_non_hosted_runner_does_not_touch_daemon_or_restart(tmp_path: Path) -> None:
    path = tmp_path / "daemon.json"
    path.write_text('{"debug":true}\n')
    original = path.read_bytes()
    restarts: list[bool] = []
    assert not mirror.configure_for_runner(
        {"GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "self-hosted", "RUNNER_OS": "Linux"},
        path,
        restart=lambda: restarts.append(True),
        has_running_containers=lambda: False,
    )
    assert path.read_bytes() == original
    assert restarts == []


@pytest.mark.parametrize("raw", [b"[]", b'{"registry-mirrors":"bad"}', b"{"])
def test_invalid_daemon_config_fails_without_restart(tmp_path: Path, raw: bytes) -> None:
    path = tmp_path / "daemon.json"
    path.write_bytes(raw)
    restarts: list[bool] = []
    with pytest.raises((ValueError, json.JSONDecodeError)):
        mirror.configure_daemon(
            path, restart=lambda: restarts.append(True), has_running_containers=lambda: False
        )
    assert path.read_bytes() == raw
    assert restarts == []


def test_running_container_refuses_change_before_write_or_restart(tmp_path: Path) -> None:
    path = tmp_path / "daemon.json"
    original = b'{"debug":true}\n'
    path.write_bytes(original)
    restarts: list[bool] = []
    with pytest.raises(RuntimeError, match="containers are running"):
        mirror.configure_daemon(
            path, restart=lambda: restarts.append(True), has_running_containers=lambda: True
        )
    assert path.read_bytes() == original
    assert restarts == []


def test_buildkit_and_workflow_ordering() -> None:
    workflow = Path(".github/workflows/docker.yml").read_text()
    assert workflow.index("name: Configure hosted-runner Docker Hub cache") < workflow.index(
        "uses: docker/setup-buildx-action@v4"
    )
    assert '[registry."docker.io"]' in workflow
    assert 'mirrors = ["mirror.gcr.io"]' in workflow
    ci = Path(".github/workflows/ci.yml").read_text()
    monitoring = ci.split("  monitoring-rules:", 1)[1]
    assert monitoring.index("name: Configure hosted-runner Docker Hub cache") < monitoring.index("docker run")
    valkey_match = re.search(r"(?ms)^  valkey-integration:\n(.*?)(?=^  [A-Za-z0-9_-]+:|\Z)", ci)
    assert valkey_match is not None
    valkey = valkey_match.group(1)
    assert "Configure hosted-runner Docker Hub cache" not in valkey
