# Operator runner (registered development stage)

First create the isolated, lock-matched environment with `uv sync --locked --extra dev`. Then invoke `.venv/bin/python scripts/coverage_operator_launcher.py --config ABSOLUTE_CONFIG_PATH --config-sha256 OUT_OF_BAND_SHA256`. Preflight verifies the running CPython and complete active distribution inventory against `uv.lock` before starting the stage clock, then carries that environment receipt in each phase handoff. The launcher creates a fresh owner-only bytecode-cache namespace and disables bytecode writes before importing project code; direct invocation of `coverage_operator_runner` is refused. There are no callback import names, environment-variable credential lookups, or status-only overrides. The config must be a canonical JSON file owned by the current user with mode `0600`; all referenced paths are absolute. Secret paths point to separate owner-only files with mode `0600`.

The config schema is `coverage-operator-stage-config/1`. Its exact top-level keys are:

```json
{
  "schema": "coverage-operator-stage-config/1",
  "stage_uuid": "fresh canonical UUID",
  "packet_stage_uuid": "different fresh canonical UUID",
  "stage_kind": "development",
  "source_revision": "40 lowercase hex characters",
  "candidate_base_url": "qualified HTTPS capture base URL",
  "answer_endpoint": "qualified HTTPS /v1/chat/completions URL",
  "allow_trusted_private_http": false,
  "initial_registration_sha256": "externally pinned 64 lowercase hex characters",
  "forbidden_stage_uuids": ["previously consumed stage UUID"],
  "paths": {
    "cohorts": "absolute path", "protocol": "absolute path",
    "qualified_source_closure": "absolute path", "coverage_source": "absolute path",
    "production_rerank_source": "absolute path", "dependency_lock": "absolute path",
    "reference_manifest": "absolute path", "answer_assessment_plan": "absolute path",
    "capture_plan": "absolute path", "candidate_identity": "absolute path",
    "initial_registration": "absolute path"
  },
  "private_paths": {
    "candidate_operator_token": "absolute path", "selector_api_key": "absolute path",
    "answer_api_key": "absolute path",
    "capture_ca_bundle": "optional absolute path to a public PEM CA bundle"
  },
  "directories": {
    "operator_handoff": "absolute private directory", "stage_inventory": "absolute private directory",
    "lease_root": "absolute private directory", "capture_receipts": "absolute private directory",
    "selector_archive": "absolute private directory", "selector_results": "absolute private directory",
    "answer_archive": "absolute private directory", "answer_results": "absolute private directory",
    "grader_handoff": "absolute private directory"
  }
}
```

`allow_trusted_private_http` is optional and defaults to `false`. Set it to `true` only for an explicitly approved private-network answer endpoint; the runner resolves the endpoint and binds the private numeric destination and `trusted-private-http` mode into the permit. Public HTTP remains rejected. Capture endpoints remain HTTPS-only. A confirmation stage requires its own registered confirmation cohort and registration bundle; running development never launches confirmation automatically. Both stage UUIDs must be fresh and absent from the consumed-stage inventory.

Before starting the stage clock or creating handoff directories, the loader verifies the external config pin, current clean checkout revision, actual loaded candidate and production reranker source paths, dependency lock bytes, registered protocol status, cohort/protocol binding, qualified-source-closure file against the exact source bytes, initial registration SHA, and its immutable static material pins. The three source materials must point to `scripts/intent_ranking_coverage.py`, `slopsearx/rerank.py`, and `uv.lock` in this checkout. It generates the acquisition plan and manifest from only the explicitly configured registered stage cohort and refuses a draft or mismatched registration.

The local operator handoff is a request/receipt/pin exchange under `<operator_handoff>/<stage_uuid>/<scope>/`. The runner writes `requests/<request_id>.json`; the trusted operator writes exact response bytes to `responses/<request_id>.receipt` and a separate lowercase SHA-256 plus newline to `pins/<request_id>.sha256`. The required request slots are acquisition/permit, protected-source-capture/permit, source-capture/permit, late-registration/registration-1, selector-map-admission/map, selector-operation-permits/batch, and answer-execution/permit. Each returned object is then passed to its existing typed verifier. Missing or bad receipts, or an existing stage inventory, stop without resumption. Native assessor packets use the existing grader handoff; the runner never invokes a model.

For fresh operator-runner stages, grader result records are accepted only with a matching native-host app-server transcript for every packet. The runner verifies the exact request packet bytes, requested and returned model, stage/phase/thread/turn chain, and final response bytes, then records transcript hashes in `results-closed.json`. Missing or mismatched transcripts prevent any submissions from being returned to the phase coordinator. The result envelope's `tool_call_id` remains a correlation field; by itself, it does not prove a native model call. See `native-host-grader-dispatch.md` for the bounded host dispatcher and its evidence limits.

`capture_ca_bundle` is optional. When present, it must be an owner-only file containing a bounded public CA PEM bundle (never a private key); its digest is included in both capture authority requests and the protected-capture qualification binding. Without it, the capture client uses the platform's normal certificate trust store.

The current checked-in protocol is still draft, so the command intentionally refuses before the stage clock, handoff directory creation, or provider dispatch. An operator must supply a separately reviewed and registered protocol/material bundle, plus actual protected-capture qualification evidence for the exact HTTPS endpoint, before this launch path can proceed. The operator handoff itself is not proof of qualification. No test fixture or mocked transport is production evidence.

Offline phase-graph coverage is exercised by `tests/test_coverage_stage_orchestration.py`; config, draft refusal, private-secret handling, handoff adapters, and late-registration lineage have focused tests in `tests/test_coverage_operator_runner.py`, `tests/test_coverage_stage_runtime.py`, `tests/test_coverage_operator_handoff.py`, and `tests/test_coverage_late_registration.py`.
