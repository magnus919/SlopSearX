import os
import subprocess
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).parents[1]


@pytest.mark.parametrize("arch", ["amd64", "arm64"])
@pytest.mark.parametrize(
    "invalid_ref",
    [
        "ghcr.io/example/foreign@sha256:" + "a" * 64,
        "ghcr.io/example/slopsearx:latest",
        "ghcr.io/example/slopsearx@sha256:bad",
    ],
)
def test_publish_manifest_rejects_invalid_tested_image_refs(tmp_path: Path, arch: str, invalid_ref: str) -> None:
    workflow = yaml.safe_load((ROOT / ".github/workflows/docker.yml").read_text())
    steps = workflow["jobs"]["publish-manifest"]["steps"]
    command = next(step["run"] for step in steps if step.get("name") == "Publish manifest from tested digests")
    image = "ghcr.io/example/slopsearx"
    digests = tmp_path / "digests"
    digests.mkdir()
    refs = {
        "amd64": f"{image}@sha256:" + "b" * 64,
        "arm64": f"{image}@sha256:" + "c" * 64,
    }
    refs[arch] = invalid_ref
    for platform, ref in refs.items():
        (digests / platform).write_text(f"{ref}\n")

    result = subprocess.run(
        ["bash", "-c", "docker() { exit 91; }\n" + command],
        cwd=tmp_path,
        env={"PATH": os.defpath, "IMAGE_NAME": image, "IMAGE_TAGS": f"{image}:latest"},
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 1
    assert "::error::Expected immutable tested digests" in result.stdout
