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
    "initial_registration": "absolute path",
    "band_companion": "optional absolute path to shared public contract"
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

Before starting the stage clock, the loader verifies the external config pin, current clean checkout revision, actual loaded candidate and production reranker source paths, dependency lock bytes, registered protocol status, cohort/protocol binding, qualified-source-closure file against the exact source bytes, initial registration SHA, and its immutable static material pins. The three source materials must point to `scripts/intent_ranking_coverage.py`, `slopsearx/rerank.py`, and `uv.lock` in this checkout. It generates the acquisition plan and manifest from only the explicitly configured registered stage cohort and refuses a draft or mismatched registration without creating stage roots. After static validation, it preflights all nine configured private directory roots, creates only missing roots with mode `0700`, and verifies each is an owned, non-symlink directory before starting the clock. An existing root with unsafe type, owner, or mode is refused without changing it; an unavailable parent or failed creation also refuses before any phase begins. This setup does not issue a permit or make a provider call.

To check a config before consuming a one-shot stage invocation, run `.venv/bin/python scripts/coverage_operator_launcher.py --config ABSOLUTE_CONFIG_PATH --config-sha256 OUT_OF_BAND_SHA256 --preflight-only`. This uses the same source, active-environment, protocol/cohort, generated acquisition-plan, and exact registration-byte checks as stage construction. It checks configured roots read-only: missing roots are allowed, and unsafe existing roots are refused without repair. The preflight does not read credential files, create or change configured study roots, construct handoffs, start the stage clock, or invoke phases. The documented launcher still creates and removes its temporary owner-only bytecode-cache directory as part of source bootstrap. Success reports the config, source-revision, and registration hashes with `static-preflight-passed`; this confirms static consistency only, not admission or authority to run. A known refusal returns a fixed safe error code; otherwise the CLI emits a generic `preflight-validation-failed` code without exposing exception text.

The local operator handoff is a request/receipt/pin exchange under `<operator_handoff>/<stage_uuid>/<scope>/`. The runner writes `requests/<request_id>.json`; the trusted operator writes exact response bytes to `responses/<request_id>.receipt` and a separate lowercase SHA-256 plus newline to `pins/<request_id>.sha256`. The required request slots are acquisition/permit, protected-source-capture/permit, source-capture/permit, late-registration/registration-1, selector-map-admission/map, selector-operation-permits/batch, and answer-execution/permit. Each returned object is then passed to its existing typed verifier. Missing or bad receipts, or an existing stage inventory, stop without resumption. Native assessor packets use the existing grader handoff; the runner never invokes a model.

For the acquisition permit, the request includes both `acquisition_manifest_sha256` (the established field) and `stage_manifest_sha256` (the typed verifier's binding name). They are equal by construction: the runner stores the manifest in the executor's exact canonical JSON representation, including its trailing newline, and static plan validation refuses any other representation. The pin on `StagePlan.acquisition_manifest_bytes` therefore equals the executor's normalized stage-manifest digest; do not remove the newline or substitute the acquisition-plan digest.

For fresh operator-runner stages, grader result records are accepted only with a matching native-host app-server transcript for every packet. The runner verifies the exact request packet bytes, requested and returned model, stage/phase/thread/turn chain, and final response bytes, then records transcript hashes in `results-closed.json`. Missing or mismatched transcripts prevent any submissions from being returned to the phase coordinator. The result envelope's `tool_call_id` remains a correlation field; by itself, it does not prove a native model call. See `native-host-grader-dispatch.md` for the bounded host dispatcher and its evidence limits.

`capture_ca_bundle` is optional. When present, it must be an owner-only file containing a bounded public CA PEM bundle (never a private key); its digest is included in both capture authority requests and the protected-capture qualification binding. Without it, the capture client uses the platform's normal certificate trust store.

The current prospective public bundle is [the October 13 band-companion registration v3](prospective-2026-10-13-band-companion-v3/README.md), bound to merged source `278bbe4b36b91aff706ff9814eaecd2bae452f84`. Its source-closure-v2 SHA-256 is `2f6b1b7f0946683df4b6e232a94e825d6f381733951b2b5b33ad9bc880d4f33f`; the shared contract SHA-256 remains `29a25b38edd13712ab19eddc75b46a1232937487029fa53dd9668ab8c0ecdbc8`. Both stages passed real Linux static preflight; receipts bind each config and registration, report zero scientific calls and no stage roots created, and are summarized in the bundle status file. The prior v2 registration remains archived as uninvoked and its four identities are retired. Neither registration was admitted or invoked, and the current v3 remains unqualified. The October 11 [setup-retry registration](registered-2026-10-11-setup-retry/README.md), [earlier static bundle](registered-2026-10-11/README.md), and [intermediate startup amendment](registered-2026-10-11-startup-amended/README.md) also remain historical. When present, optional `paths.band_companion` points to the shared canonical public contract. The separate [October 13 lab-preparation packet](https://github.com/magnus919/groktocrawl-x/blob/f770d75b4d2e0422ad9aa6631d1be0b644d9edc1/docs/experiments/evidence/protected-capture-lab-2026-10-13/README.md) from [GroktoCrawl-X PR #453](https://github.com/magnus919/groktocrawl-x/pull/453) is API-only candidate lab readiness, not research-quality evidence, a study result, or a permit. Static preflight does not establish study qualification; no test fixture or mocked transport is production evidence.

Offline phase-graph coverage is exercised by `tests/test_coverage_stage_orchestration.py`; config, draft refusal, private-secret handling, handoff adapters, and late-registration lineage have focused tests in `tests/test_coverage_operator_runner.py`, `tests/test_coverage_stage_runtime.py`, `tests/test_coverage_operator_handoff.py`, and `tests/test_coverage_late_registration.py`.

## October 13 v3 execution result

The v3 development stage was subsequently invoked once after refreshed lab
qualification. It ended terminal-incomplete during acquisition: three complete
queries, one arXiv engine failure, nine operations uninvoked, and nine physical
requests. Capture, Jev, answers and native grading were never invoked; untouched
confirmation remains uninvoked. The earlier static-preflight and registration
statements above describe their historical time of creation. See the
[terminal result](completed-2026-10-13-v3/README.md). Do not invoke either v3
stage as a restart or rescue of this ended study.
