import re
from pathlib import Path

import yaml

ROOT = Path(__file__).parents[1]


def test_dockerfile_uses_a_digest_pinned_python_base() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text()
    assert re.search(r"^FROM python:3\.12-slim@sha256:[0-9a-f]{64}$", dockerfile, re.MULTILINE)


def test_existing_compose_source_build_remains_fail_closed() -> None:
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    service = compose["services"]["slopsearx"]

    assert service["build"]["context"] == "."
    refresh_arg = service["build"]["args"]["DEBIAN_SECURITY_REFRESH"]
    assert refresh_arg == "${DEBIAN_SECURITY_REFRESH:-}"
    assert service["image"] == "slopsearx:0.1.0"


def test_dependabot_tracks_docker_digest_updates() -> None:
    dependabot = yaml.safe_load((ROOT / ".github/dependabot.yml").read_text())
    assert any(
        update["package-ecosystem"] == "docker" and update["directory"] == "/" for update in dependabot["updates"]
    )
