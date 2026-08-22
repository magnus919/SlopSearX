import re
from pathlib import Path
from urllib.error import HTTPError, URLError

import pytest
import yaml

from scripts import verify_docker_base_digest as verify

ROOT = Path(__file__).parents[1]


def test_dockerfile_uses_a_digest_pinned_python_base() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text()
    assert re.search(r"^FROM python:[^@\s]+@sha256:[0-9a-f]{64}$", dockerfile, re.MULTILINE)

    workflow = (ROOT / ".github/workflows/docker.yml").read_text()
    assert "python3 scripts/verify_docker_base_digest.py" in workflow


def test_pinned_base_parser_accepts_generic_python_tag() -> None:
    digest = "a" * 64
    assert verify._pinned_python_base(f"FROM python:3.13-slim-bookworm@sha256:{digest}\n") == (
        "3.13-slim-bookworm",
        f"sha256:{digest}",
    )


def test_pinned_base_parser_rejects_multiple_from_lines() -> None:
    digest = "a" * 64
    dockerfile = f"FROM python:3.12-slim@sha256:{digest}\nFROM scratch\n"

    with pytest.raises(SystemExit, match="exactly one FROM"):
        verify._pinned_python_base(dockerfile)


def test_registry_lookup_retries_transient_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[object] = []

    class Response:
        def __enter__(self) -> "Response":
            return self

        def __exit__(self, *args: object) -> None:
            return None

    def fake_urlopen(request: object, timeout: int) -> Response:
        calls.append((request, timeout))
        if len(calls) < 3:
            raise URLError("temporary failure")
        return Response()

    monkeypatch.setattr(verify.urllib.request, "urlopen", fake_urlopen)
    monkeypatch.setattr(verify.time, "sleep", lambda _seconds: None)

    with verify._urlopen_with_retries("https://example.test", label="test request"):
        pass

    assert len(calls) == 3


def test_registry_lookup_classifies_permanent_http_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[object] = []

    def fake_urlopen(request: object, timeout: int) -> None:
        calls.append((request, timeout))
        raise HTTPError("https://example.test", 404, "not found", {}, None)

    monkeypatch.setattr(verify.urllib.request, "urlopen", fake_urlopen)

    with pytest.raises(verify.RegistryLookupError, match="HTTP 404"):
        verify._urlopen_with_retries("https://example.test", label="test request")

    assert len(calls) == 1


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
