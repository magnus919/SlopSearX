import re
from pathlib import Path
from urllib.error import HTTPError, URLError

import pytest
import yaml

from scripts import promote_image_digest as promo
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


@pytest.mark.parametrize(
    "second_from",
    [
        "FROM --platform=linux/amd64 python:3.12-slim",
        "FROM python:3.12-slim  # runtime stage",
    ],
)
def test_pinned_base_parser_rejects_unrecognized_from_lines(second_from: str) -> None:
    digest = "a" * 64
    dockerfile = f"FROM python:3.12-slim@sha256:{digest}\n{second_from}\n"

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


GHCR_DIGEST_PIN = "ghcr.io/magnus919/slopsearx@sha256:91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7"


def test_existing_compose_source_build_remains_fail_closed() -> None:
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    service = compose["services"]["slopsearx"]

    # The default service must consume the CI-scanned artifact, pinned by
    # digest — never a mutable tag (issue #210).
    assert service["image"] == GHCR_DIGEST_PIN

    build = compose["services"]["slopsearx-build"]
    assert build["build"]["context"] == "."
    refresh_arg = build["build"]["args"]["DEBIAN_SECURITY_REFRESH"]
    assert refresh_arg == "${DEBIAN_SECURITY_REFRESH:-}"
    assert build["image"] == "slopsearx:local"
    assert "build" in build.get("profiles", [])


def test_kubernetes_consumes_pinned_ghcr_artifact() -> None:
    deployment = yaml.safe_load((ROOT / "k8s/deployment.yaml").read_text())
    container = deployment["spec"]["template"]["spec"]["containers"][0]
    assert container["image"] == GHCR_DIGEST_PIN


def test_compose_build_variant_stays_reachable_as_slopsearx() -> None:
    """The escape hatch must be a drop-in replacement at the stack's DNS name."""
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    build = compose["services"]["slopsearx-build"]
    assert "slopsearx" in build["networks"]["default"]["aliases"]


def test_promotion_workflow_guards_the_app_image_pin() -> None:
    """Digest pins drift when CI republishes latest; a scheduled job must heal them."""
    workflow = yaml.safe_load((ROOT / ".github/workflows/image-promotion.yml").read_text())
    triggers = workflow[True]  # PyYAML resolves the `on:` key to boolean True
    assert "schedule" in triggers and "workflow_dispatch" in triggers

    step_text = (ROOT / ".github/workflows/image-promotion.yml").read_text()
    assert "scripts/promote_image_digest.py --write" in step_text


def test_docker_prune_keeps_a_retention_window_for_orphaned_pins() -> None:
    """Re-runs rebuild the same SHA tag with a fresh digest; pruning must not
    delete the currently pinned digest before promotion can catch up."""
    workflow_text = (ROOT / ".github/workflows/docker.yml").read_text()
    assert "RETENTION_DAYS" in workflow_text
    assert "created_at" in workflow_text


def test_promotion_script_extracts_the_unique_pinned_digest(tmp_path: Path) -> None:
    digest = "b" * 64
    for name, body in [
        ("docker-compose.yml", f"image: ghcr.io/magnus919/slopsearx@sha256:{digest}\n"),
        ("k8s/deployment.yaml", f"          image: ghcr.io/magnus919/slopsearx@sha256:{digest}\n"),
        ("tests/test_deployment_artifacts.py", f'PIN = "ghcr.io/magnus919/slopsearx@sha256:{digest}"\n'),
    ]:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body)

    assert promo.extract_pinned_digest(tmp_path) == f"sha256:{digest}"


def test_promotion_script_fails_closed_on_conflicting_pins(tmp_path: Path) -> None:
    for name, digest in [
        ("docker-compose.yml", "c" * 64),
        ("k8s/deployment.yaml", "c" * 64),
        ("tests/test_deployment_artifacts.py", "d" * 64),
    ]:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"image: ghcr.io/magnus919/slopsearx@sha256:{digest}\n")

    with pytest.raises(promo.PromotionError, match="exactly one unique pinned digest"):
        promo.extract_pinned_digest(tmp_path)


def test_promotion_script_fails_closed_on_missing_pin_file(tmp_path: Path) -> None:
    (tmp_path / "docker-compose.yml").write_text(f"image: ghcr.io/magnus919/slopsearx@sha256:{'c' * 64}\n")

    with pytest.raises(promo.PromotionError, match="missing expected pin file"):
        promo.extract_pinned_digest(tmp_path)


def test_promotion_script_applies_new_digest_across_all_files(tmp_path: Path) -> None:
    old, new = "e" * 64, "f" * 64
    files = {}
    for name in promo.PIN_FILES:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"ghcr.io/magnus919/slopsearx@sha256:{old}\n")
        files[name] = path

    changed = promo.apply_promotion(tmp_path, f"sha256:{new}")

    assert sorted(changed) == sorted(promo.PIN_FILES)
    for path in files.values():
        assert f"ghcr.io/magnus919/slopsearx@sha256:{new}" in path.read_text()


def test_promotion_script_classifies_permanent_http_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fake_urlopen(_request: object, timeout: int) -> object:
        raise HTTPError("https://example.test", 404, "not found", {}, None)

    monkeypatch.setattr(promo.urllib.request, "urlopen", fake_urlopen)

    with pytest.raises(promo.PromotionError):
        promo.resolve_latest_digest()


def test_compose_default_service_has_no_build_block() -> None:
    """A digest ref cannot carry a build context; the pull-through default
    service must stay declarative so the scanned artifact is authoritative."""
    compose_text = (ROOT / "docker-compose.yml").read_text()
    compose = yaml.safe_load(compose_text)
    assert "build" not in compose["services"]["slopsearx"]
    assert GHCR_DIGEST_PIN in compose_text


def test_spec_examples_use_the_pinned_ghcr_artifact() -> None:
    spec = (ROOT / "spec.md").read_text()
    assert "slopsearx:0.1.0" not in spec
    assert spec.count(GHCR_DIGEST_PIN) >= 2


def test_dependabot_tracks_docker_digest_updates() -> None:
    dependabot = yaml.safe_load((ROOT / ".github/dependabot.yml").read_text())
    assert any(
        update["package-ecosystem"] == "docker" and update["directory"] == "/" for update in dependabot["updates"]
    )
