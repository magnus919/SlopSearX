"""Guard the staged v2 review integration against accidental activation."""

import hashlib
import json
import re
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[1]
HARNESS_SHA = "6f67aeabb812e6992bd868280698cf1031a75e46"
WORKFLOWS = ("pr-review-analysis.yml", "pr-publish.yml", "protected-source-context-probe.yml")


@pytest.mark.parametrize("filename", WORKFLOWS)
def test_staged_jobs_remain_disabled_and_keep_scoped_permissions(filename: str) -> None:
    workflow = yaml.load((ROOT / ".github/workflows" / filename).read_text(), Loader=yaml.BaseLoader)
    assert set(workflow["permissions"].values()) == {"read"}
    for job_name, job in workflow["jobs"].items():
        assert job["if"] in {False, "false", "${{ false }}"}
        if filename == "pr-publish.yml" and job_name == "publisher-disabled":
            assert job["permissions"] == {
                "actions": "read",
                "contents": "read",
                "pull-requests": "write",
            }
        else:
            assert set(job["permissions"].values()) == {"read"}
        if "uses" in job:
            assert job["uses"].endswith("@" + HARNESS_SHA)
        for step in job.get("steps", []):
            if "uses" in step:
                assert re.search(r"@[0-9a-f]{40}$", step["uses"])
            if step.get("uses", "").startswith("actions/checkout@"):
                assert step["with"]["ref"] == "${{ github.sha }}"
                assert step["with"]["persist-credentials"] in {False, "false"}


def test_v2_policy_stays_disabled_and_binds_target_source_and_actions_token() -> None:
    policy = json.loads((ROOT / ".github/pr-review-publisher-policy.json").read_text())
    assert policy["schema"] == "pr-review-protected-publication-policy.v2"
    assert policy["enabled"] is False
    assert policy["authorization_mode"] == "STANDING_BOUNDED"
    assert policy["repository"] == {"name": "magnus919/SlopSearX", "id": 1263452316}
    assert policy["called_harness"]["sha"] == HARNESS_SHA
    assert policy["default_branch"] == "main"
    assert policy["credential"] == {"kind": "ACTIONS_TOKEN", "actor_login": "github-actions[bot]"}
    assert policy["publication"]["allowed_dispositions"] == ["COMMENT"]
    assert policy["artifact_redirect_hosts"] == ["productionresultssa5.blob.core.windows.net"]

    publisher_path = ROOT / ".github/workflows/pr-publish.yml"
    assert policy["publisher_workflow"]["sha256"] == hashlib.sha256(publisher_path.read_bytes()).hexdigest()
    assert policy["publisher_workflow"]["path"] == ".github/workflows/pr-publish.yml"
    assert policy["source_workflow"]["path"] == ".github/workflows/pr-review-analysis.yml"
    assert isinstance(policy["publisher_workflow"]["id"], int)
    assert isinstance(policy["source_workflow"]["id"], int)
    assert "app" not in policy


def test_analysis_trigger_supports_ready_pr_without_enabling_drafts() -> None:
    analysis = yaml.load((ROOT / ".github/workflows/pr-review-analysis.yml").read_text(), Loader=yaml.BaseLoader)
    assert analysis["on"] == {
        "pull_request_target": {
            "branches": ["main"],
            "types": ["opened", "ready_for_review", "reopened", "synchronize"],
        }
    }
    assert analysis["jobs"]["analyze"]["if"] == "false"
    assert analysis["jobs"]["analyze"]["with"]["harness_sha"] == HARNESS_SHA


def test_probe_trigger_and_publisher_serialization_remain_bounded() -> None:
    workflows = {
        name: yaml.load((ROOT / ".github/workflows" / name).read_text(), Loader=yaml.BaseLoader) for name in WORKFLOWS
    }
    probe = workflows["protected-source-context-probe.yml"]
    assert probe["on"] == {
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
