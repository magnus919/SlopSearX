import re
from pathlib import Path

import yaml

ROOT = Path(__file__).parents[1]
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
IMAGE_RE = re.compile(r"^ghcr\.io/magnus919/slopsearx@(?P<digest>sha256:[0-9a-f]{64})$")


def _service(document: dict) -> dict:
    return document["services"]["slopsearx"]


def test_dockerfile_uses_a_digest_pinned_python_base() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text()
    assert re.search(r"^FROM python:3\.12-slim@sha256:[0-9a-f]{64}$", dockerfile, re.MULTILINE)


def test_compose_defaults_to_the_promoted_immutable_image() -> None:
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    service = _service(compose)
    match = IMAGE_RE.fullmatch(service["image"])

    assert match is not None
    assert "build" not in service

    build_override = yaml.safe_load((ROOT / "docker-compose.build.yml").read_text())
    build_service = _service(build_override)
    assert build_service["image"] == "slopsearx:0.1.0-dev"
    assert build_service["build"]["args"]["DEBIAN_SECURITY_REFRESH"].startswith("${")
    assert ":?" in build_service["build"]["args"]["DEBIAN_SECURITY_REFRESH"]


def test_kubernetes_uses_the_same_immutable_image_and_records_provenance() -> None:
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    promoted_image = _service(compose)["image"]

    deployment = yaml.safe_load((ROOT / "k8s/deployment.yaml").read_text())
    container = deployment["spec"]["template"]["spec"]["containers"][0]
    image_match = IMAGE_RE.fullmatch(container["image"])
    digest = image_match.group("digest") if image_match else None

    assert image_match is not None
    assert container["image"] == promoted_image
    assert container["imagePullPolicy"] == "IfNotPresent"

    kustomization = yaml.safe_load((ROOT / "k8s/kustomization.yaml").read_text())
    annotations = kustomization["commonAnnotations"]
    assert annotations["slopsearx.org/image-digest"] == digest
    assert re.fullmatch(r"[0-9a-f]{40}", annotations["slopsearx.org/source-revision"])
    assert DIGEST_RE.fullmatch(annotations["slopsearx.org/image-digest"])


def test_dependabot_tracks_docker_digest_updates() -> None:
    dependabot = yaml.safe_load((ROOT / ".github/dependabot.yml").read_text())
    assert any(
        update["package-ecosystem"] == "docker" and update["directory"] == "/" for update in dependabot["updates"]
    )
