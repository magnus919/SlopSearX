"""Guard the protected review activation and staged diagnostic boundary."""

import hashlib
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
HARNESS_SHA = "537418ca9f542ba403b6bc6b8280eacb52d1ce0f"


def _workflow(filename: str) -> dict:
    return yaml.load((ROOT / ".github/workflows" / filename).read_text(), Loader=yaml.BaseLoader)


def test_analysis_is_limited_to_non_draft_main_pull_request_targets() -> None:
    analysis = _workflow("pr-review-analysis.yml")
    assert analysis["on"] == {
        "pull_request_target": {
            "branches": ["main"],
            "types": ["opened", "ready_for_review", "reopened", "synchronize"],
        }
    }
    assert set(analysis["permissions"].values()) == {"read"}
    job = analysis["jobs"]["analyze"]
    assert job["if"].split() == [
        "github.event_name",
        "==",
        "'pull_request_target'",
        "&&",
        "github.repository",
        "==",
        "'magnus919/SlopSearX'",
        "&&",
        "github.event.pull_request.base.ref",
        "==",
        "'main'",
        "&&",
        "github.event.pull_request.draft",
        "==",
        "false",
    ]
    assert job["uses"] == f"groktopus/codereview/.github/workflows/pr-analysis.yml@{HARNESS_SHA}"
    assert job["with"]["harness_sha"] == HARNESS_SHA
    assert job["with"]["review_contract"] == "bounded-production-v17"
    assert set(job["permissions"].values()) == {"read"}


def test_publisher_requires_successful_target_run_and_protected_main_ref() -> None:
    publisher = _workflow("pr-publish.yml")
    assert publisher["on"] == {"workflow_run": {"workflows": ["PR Review Analysis"], "types": ["completed"]}}
    assert set(publisher["permissions"].values()) == {"read"}
    job = publisher["jobs"]["publisher"]
    assert job["if"].split() == [
        "github.event_name",
        "==",
        "'workflow_run'",
        "&&",
        "github.event.workflow_run.conclusion",
        "==",
        "'success'",
        "&&",
        "github.event.workflow_run.event",
        "==",
        "'pull_request_target'",
        "&&",
        "github.repository",
        "==",
        "'magnus919/SlopSearX'",
        "&&",
        "github.ref",
        "==",
        "'refs/heads/main'",
    ]
    assert job["permissions"] == {
        "actions": "read",
        "contents": "read",
        "pull-requests": "write",
    }
    assert job["environment"]["name"] == "code-review-publication"
    checkout_steps = [step for step in job["steps"] if step.get("uses", "").startswith("actions/checkout@")]
    assert len(checkout_steps) == 1
    assert checkout_steps[0]["with"]["ref"] == "${{ github.sha }}"
    assert checkout_steps[0]["with"]["persist-credentials"] == "false"
    for step in job["steps"]:
        if "uses" in step:
            assert re.search(r"@[0-9a-f]{40}$", step["uses"])
    assert publisher["concurrency"]["group"] == "pr-review-publish-${{ github.repository_id }}"
    assert publisher["concurrency"]["queue"] == "max"
    assert publisher["concurrency"]["cancel-in-progress"] == "false"
    assert job["timeout-minutes"] == "5"


def test_policy_is_enabled_for_comment_only_canary_and_binds_publisher_hash() -> None:
    policy = json.loads((ROOT / ".github/pr-review-publisher-policy.json").read_text())
    assert policy["schema"] == "pr-review-protected-publication-policy.v2"
    assert policy["enabled"] is True
    assert policy["authorization_mode"] == "STANDING_BOUNDED"
    assert policy["repository"] == {"name": "magnus919/SlopSearX", "id": 1263452316}
    assert policy["default_branch"] == "main"
    assert policy["source_workflow"] == {
        "id": 377341909,
        "path": ".github/workflows/pr-review-analysis.yml",
        "ref": "magnus919/SlopSearX/.github/workflows/pr-review-analysis.yml@refs/heads/main",
    }
    assert policy["publisher_workflow"]["id"] == 377341907
    assert policy["publisher_workflow"]["path"] == ".github/workflows/pr-publish.yml"
    publisher_path = ROOT / policy["publisher_workflow"]["path"]
    assert policy["publisher_workflow"]["sha256"] == hashlib.sha256(publisher_path.read_bytes()).hexdigest()
    assert policy["called_harness"]["sha"] == HARNESS_SHA
    assert policy["credential"] == {"kind": "ACTIONS_TOKEN", "actor_login": "github-actions[bot]"}
    assert policy["profile"] == {
        "version": "slopsearx-production-v17-context-target-manifest-candidate",
        "sha256": "28725e806a87812386ff67f5c6e63320cc9f3f2b9c1a79c6942c7cf1be004dac",
        "provider_configuration_identity": (
            "provider-identity-sha256:c87999815e0c8d35d486803558939375130c79a332ed6d421bbfb7171d4a63b6"
        ),
    }
    assert policy["publication"]["allowed_dispositions"] == ["COMMENT"]
    assert policy["artifact_redirect_hosts"] == [
        "productionresultssa5.blob.core.windows.net",
        "productionresultssa8.blob.core.windows.net",
    ]
    assert "app" not in policy


def test_source_context_probe_remains_disabled_and_read_only() -> None:
    probe = _workflow("protected-source-context-probe.yml")
    assert probe["on"] == {
        "pull_request_target": {"branches": ["main"], "types": ["opened", "reopened", "synchronize"]}
    }
    assert set(probe["permissions"].values()) == {"read"}
    for job in probe["jobs"].values():
        assert job["if"] in {False, "false", "${{ false }}"}
        assert set(job["permissions"].values()) == {"read"}
