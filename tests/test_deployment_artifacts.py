import re
from pathlib import Path

import yaml

ROOT = Path(__file__).parents[1]


def test_dockerfile_uses_a_digest_pinned_python_base() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text()
    assert re.search(r"^FROM python:[^@\s]+@sha256:[0-9a-f]{64}$", dockerfile, re.MULTILINE)

    workflow = (ROOT / ".github/workflows/docker.yml").read_text()
    assert '"$SSX_IMAGE_OPS_BIN" verify-base-digests --dockerfile Dockerfile' in workflow


def _deployment_image_pin() -> str:
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    pin = compose["services"]["slopsearx"]["image"]
    assert isinstance(pin, str)
    assert re.fullmatch(r"ghcr\.io/magnus919/slopsearx@sha256:[0-9a-f]{64}", pin)
    return pin


GHCR_DIGEST_PIN = _deployment_image_pin()


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


def test_compose_passes_parent_and_checkout_env_files_to_each_variant() -> None:
    """Engine credentials in either supported .env location reach the app."""
    compose = yaml.safe_load((ROOT / "docker-compose.yml").read_text())
    expected_env_files = [
        {"path": "../.env", "required": False},
        {"path": ".env", "required": False},
    ]

    for service_name in ("slopsearx", "slopsearx-build"):
        assert compose["services"][service_name]["env_file"] == expected_env_files


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
    assert workflow["permissions"] == {"contents": "write"}
    assert workflow["jobs"]["promote"]["permissions"] == {"contents": "write", "pull-requests": "write"}

    step_text = (ROOT / ".github/workflows/image-promotion.yml").read_text()
    assert '"$SSX_IMAGE_OPS_BIN" promote-image-digest --write --root .' in step_text
    pr = next(
        step for step in workflow["jobs"]["promote"]["steps"] if step.get("name") == "Open or update promotion PR"
    )
    assert pr["with"]["add-paths"].splitlines() == ["docker-compose.yml", "k8s/deployment.yaml", "spec.md"]
    assert pr["with"]["branch"] == "chore/promote-image-digest"
    assert pr["if"] == "steps.promote.outputs.changed != '0'"


def test_docker_prune_keeps_a_retention_window_for_orphaned_pins() -> None:
    """Re-runs rebuild the same SHA tag with a fresh digest; pruning must not
    delete the currently pinned digest before promotion can catch up."""
    workflow_text = (ROOT / ".github/workflows/docker.yml").read_text()
    assert "RETENTION_DAYS" in workflow_text
    assert "created_at" in workflow_text


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


def test_container_publication_requires_both_native_architectures() -> None:
    workflow = yaml.safe_load((ROOT / ".github/workflows/docker.yml").read_text())
    build = workflow["jobs"]["build-and-push"]
    assert build["strategy"]["matrix"]["include"] == [
        {"arch": "amd64", "runner": "ubuntu-24.04"},
        {"arch": "arm64", "runner": "ubuntu-24.04-arm"},
    ]
    steps = build["steps"]
    names = [step.get("name") for step in steps]
    push_index = names.index("Push tested architecture image")
    for gate in ("Scan image with Trivy", "Smoke test container", "Smoke test configured Darker default"):
        index = names.index(gate)
        assert index < push_index
        assert not steps[index].get("continue-on-error", False)
    build_options = steps[names.index("Build and load image")]["with"]
    assert build_options["platforms"] == "linux/${{ matrix.arch }}"
    assert build_options["load"] is True
    assert build_options["push"] is False
    assert steps[push_index]["if"] == "github.event_name == 'push'"

    publish = workflow["jobs"]["publish-manifest"]
    assert publish["needs"] == "build-and-push"
    assert publish["if"] == "github.event_name == 'push'"
    command = next(
        step["run"] for step in publish["steps"] if step.get("name") == "Publish manifest from tested digests"
    )
    assert '"$(cat digests/amd64)" "$(cat digests/arm64)"' in command
    assert "verify-image-platforms --manifest manifest.json" in command
    assert "python" not in command


def test_architecture_tags_follow_commit_retention_instead_of_each_run() -> None:
    workflow = yaml.safe_load((ROOT / ".github/workflows/docker.yml").read_text())
    steps = workflow["jobs"]["build-and-push"]["steps"]
    tag_command = next(step["run"] for step in steps if step.get("name") == "Set architecture image tag")
    assert "${GITHUB_SHA::7}-${{ matrix.arch }}" in tag_command
    assert "GITHUB_RUN_ID" not in tag_command
    assert "GITHUB_RUN_ATTEMPT" not in tag_command


def test_native_health_builder_inputs_and_tool_handoff_remain_bounded() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text()
    builder = dockerfile.split("# SlopSearX production image", 1)[0]
    assert [line for line in builder.splitlines() if line.startswith("COPY ")] == [
        "COPY Cargo.toml Cargo.lock rust-toolchain.toml ./",
        "COPY crates/ crates/",
        "COPY vendor/ vendor/",
    ]
    assert 'CMD ["/usr/local/bin/ssx-healthcheck"]' in dockerfile
    assert "uvicorn" in dockerfile
    workflow = yaml.safe_load((ROOT / ".github/workflows/docker.yml").read_text())
    jobs = workflow["jobs"]
    handoff = next(
        step for step in jobs["build-and-push"]["steps"] if step.get("name") == "Hand off native manifest verifier"
    )
    assert handoff["if"] == "github.event_name == 'push' && matrix.arch == 'amd64'"
    download = next(
        step
        for step in jobs["publish-manifest"]["steps"]
        if step.get("name") == "Download native manifest verifier from this run"
    )
    assert handoff["with"]["name"] == download["with"]["name"]
    assert "run-id" not in download["with"]
    assert "repository" not in download["with"]
    assert not (ROOT / "scripts/verify_docker_base_digest.py").exists()
