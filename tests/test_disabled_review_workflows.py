"""Guard the staged review integration against accidental activation."""

import json
import re
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[1]
HARNESS_SHA = "7eaddf7c549c8060045578bc1702e394a35dfa2d"
WORKFLOWS = ("pr-review-analysis.yml", "pr-publish.yml", "protected-source-context-probe.yml")


@pytest.mark.parametrize("filename", WORKFLOWS)
def test_staged_jobs_are_disabled_and_read_only(filename: str) -> None:
    workflow = yaml.safe_load((ROOT / ".github/workflows" / filename).read_text())
    assert set(workflow["permissions"].values()) == {"read"}
    for job in workflow["jobs"].values():
        assert job["if"] is False or job["if"] == "${{ false }}"
        assert set(job["permissions"].values()) == {"read"}
        if "uses" in job:
            assert job["uses"].endswith("@" + HARNESS_SHA)
        for step in job.get("steps", []):
            if "uses" in step:
                assert re.search(r"@[0-9a-f]{40}$", step["uses"])
            if step.get("uses", "").startswith("actions/checkout@"):
                assert step["with"]["ref"] == "${{ github.sha }}"
                assert step["with"]["persist-credentials"] is False


def test_policy_remains_disabled_and_requires_separate_identity_qualification() -> None:
    policy = json.loads((ROOT / ".github/pr-review-publisher-policy.json").read_text())
    assert policy["enabled"] is False
    assert policy["repository"] == {"name": "magnus919/SlopSearX", "id": 1263452316}
    assert policy["called_harness"]["sha"] == HARNESS_SHA
    assert policy["default_branch"] == "main"
    assert policy["publication"]["allowed_dispositions"] == ["COMMENT"]
    for workflow in ("source_workflow", "publisher_workflow"):
        assert policy[workflow]["id"].startswith("REPLACE_WITH_")
    assert all(value.startswith("REPLACE_WITH_") for value in policy["app"].values())
    assert policy["artifact_redirect_hosts"] == ["REPLACE_WITH_APPROVED_GITHUB_ARTIFACT_DOWNLOAD_HOST"]


def test_protected_triggers_and_publisher_serialization() -> None:
    # BaseLoader preserves the YAML `on` key rather than treating it as a boolean.
    workflows = {
        name: yaml.load((ROOT / ".github/workflows" / name).read_text(), Loader=yaml.BaseLoader) for name in WORKFLOWS
    }
    for name in (WORKFLOWS[0], WORKFLOWS[2]):
        assert workflows[name]["on"] == {
            "pull_request_target": {
                "branches": ["main"],
                "types": ["opened", "reopened", "synchronize"],
            }
        }
    publisher = workflows["pr-publish.yml"]
    assert publisher["on"] == {
        "workflow_run": {
            "workflows": [workflows["pr-review-analysis.yml"]["name"]],
            "types": ["completed"],
        }
    }
    assert publisher["concurrency"]["cancel-in-progress"] == "false"
    assert publisher["concurrency"]["queue"] == "max"
    assert publisher["jobs"]["publisher-disabled"]["environment"]["name"] == "code-review-publication"
