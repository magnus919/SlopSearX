"""Guard the protected review activation and staged diagnostic boundary."""

import hashlib
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
HARNESS_SHA = "1c76d40290cdaf9b6603721f899da6c7e8a382b4"


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
        if "uses" in step and not step["uses"].startswith("./"):
            assert re.search(r"@[0-9a-f]{40}$", step["uses"])
    assert publisher["concurrency"]["group"] == "pr-review-publish-${{ github.repository_id }}"
    assert publisher["concurrency"]["queue"] == "max"
    assert publisher["concurrency"]["cancel-in-progress"] == "false"
    assert job["timeout-minutes"] == "5"


def test_policy_allows_review_dispositions_and_binds_publisher_hash() -> None:
    policy = json.loads((ROOT / ".github/pr-review-publisher-policy.json").read_text())
    assert policy["schema"] == "pr-review-protected-publication-policy.v5"
    assert policy["publication_mode"] == "DISPOSITION"
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
    harness_checkout = next(
        step
        for step in _workflow("pr-publish.yml")["jobs"]["publisher"]["steps"]
        if step.get("name") == "Check out the separately pinned harness"
    )
    assert f'TRUSTED_HARNESS_SHA = "{HARNESS_SHA}"' in harness_checkout["run"]
    assert '"checkout", "--detach", TRUSTED_HARNESS_SHA' in harness_checkout["run"]
    run_action = next(
        step
        for step in _workflow("pr-publish.yml")["jobs"]["publisher"]["steps"]
        if step.get("name") == "Run the protected publication runtime"
    )
    assert run_action["uses"] == f"groktopus/codereview/scripts/protected-publication-action@{HARNESS_SHA}"
    assert run_action["uses"].endswith(f"@{policy['called_harness']['sha']}")
    assert "run" not in run_action
    assert run_action["env"]["PYTHONPATH"] == "${{ runner.temp }}/trusted-review-harness/src"
    assert policy["credential"] == {"kind": "ACTIONS_TOKEN", "actor_login": "github-actions[bot]"}
    assert policy["profile"] == {
        "version": "slopsearx-production-v17-context-target-manifest-candidate",
        "sha256": "28725e806a87812386ff67f5c6e63320cc9f3f2b9c1a79c6942c7cf1be004dac",
        "provider_configuration_identity": (
            "provider-identity-sha256:c87999815e0c8d35d486803558939375130c79a332ed6d421bbfb7171d4a63b6"
        ),
    }
    assert policy["publication"]["allowed_dispositions"] == [
        "COMMENT",
        "APPROVE",
        "REQUEST_CHANGES",
    ]
    assert policy["publication"]["max_review_body_bytes"] == 60000
    assert policy["publication"]["effect_policy"] == "PUBLISH_REVIEW"
    assert policy["limits"] == {
        "max_runs": 20,
        "max_pages": 2,
        "max_reviews": 100,
        "deadline_seconds": 25,
    }
    assert policy["artifact_download_mode"] == "AUTHENTICATED_API_LOCATION_NO_AUTH"
    history = policy["history_reconciliation"]
    assert history["schema"] == "publisher-history-reconciliation.v1"
    assert history["max_historical_exceptions"] == 4
    assert len(history["historical_exceptions"]) == 4
    assert [item["run_id"] for item in history["historical_exceptions"]] == [
        38100234484,
        38100501781,
        38101731057,
        38102353998,
    ]
    expected_exception_fields = {
        "run_id",
        "attempt",
        "publisher_workflow_id",
        "publisher_run_head_sha",
        "runtime_pin",
        "publisher_workflow_blob_sha",
        "analysis_workflow_blob_sha",
        "publisher_job_id",
        "publisher_job_name",
        "runtime_step",
        "log_sha256",
        "log_bytes",
        "outcome",
        "reason_code",
        "proof_kind",
    }
    expected_historical_bindings = {
        38100234484: (
            "39a30957885edcdd2e0f29fb0066ba04ac8bd1ee",
            "537418ca9f542ba403b6bc6b8280eacb52d1ce0f",
        ),
        38100501781: (
            "39a30957885edcdd2e0f29fb0066ba04ac8bd1ee",
            "537418ca9f542ba403b6bc6b8280eacb52d1ce0f",
        ),
        38101731057: (
            "39a30957885edcdd2e0f29fb0066ba04ac8bd1ee",
            "537418ca9f542ba403b6bc6b8280eacb52d1ce0f",
        ),
        38102353998: (
            "e924b7815393a145b2abd121069a7cb9cdbe571b",
            "23852759ed3cfb1e6964b7438141e293508aa178",
        ),
    }
    for exception in history["historical_exceptions"]:
        assert set(exception) == expected_exception_fields
        assert exception["publisher_workflow_id"] == policy["publisher_workflow"]["id"]
        assert (exception["publisher_run_head_sha"], exception["runtime_pin"]) == expected_historical_bindings[
            exception["run_id"]
        ]
        assert exception["publisher_job_name"] == "publisher"
        assert exception["runtime_step"] == 8
        assert exception["outcome"] == "UNKNOWN"
        assert exception["attempt"] == 1
        assert exception["publisher_run_head_sha"] in {
            "39a30957885edcdd2e0f29fb0066ba04ac8bd1ee",
            "e924b7815393a145b2abd121069a7cb9cdbe571b",
        }
        assert len(exception["publisher_workflow_blob_sha"]) == 40
        assert len(exception["analysis_workflow_blob_sha"]) == 40
        assert len(exception["log_sha256"]) == 64
        assert exception["log_bytes"] > 0
        assert exception["reason_code"] in {"source_artifact_redirect_untrusted", "publication_admission_unavailable"}
        assert exception["proof_kind"] == "TRUSTED_PRE_POST_RECONCILIATION"
    assert "artifact_redirect_hosts" not in policy
    assert "app" not in policy


def test_full_cutover_disables_only_droid_auto_review() -> None:
    droid_review = _workflow("droid-review.yml")
    assert droid_review["jobs"]["droid-review"]["if"] in {False, "false", "${{ false }}"}
    tag_workflow = ROOT / ".github/workflows/droid.yml"
    wiki_workflow = ROOT / ".github/workflows/droid-wiki-refresh.yml"
    assert hashlib.sha256(tag_workflow.read_bytes()).hexdigest() == (
        "b5e4a736ef559dcb2aca41f2c5c1c2e7f196eda681c15f8908bc89285eebac4b"
    )
    assert hashlib.sha256(wiki_workflow.read_bytes()).hexdigest() == (
        "76f19668a49507139199f75b7bc182c8ebe56996abe3a5eccb9900b9b28e3b8d"
    )


def test_source_context_probe_remains_disabled_and_read_only() -> None:
    probe = _workflow("protected-source-context-probe.yml")
    assert probe["on"] == {
        "pull_request_target": {"branches": ["main"], "types": ["opened", "reopened", "synchronize"]}
    }
    assert set(probe["permissions"].values()) == {"read"}
    for job in probe["jobs"].values():
        assert job["if"] in {False, "false", "${{ false }}"}
        assert set(job["permissions"].values()) == {"read"}
