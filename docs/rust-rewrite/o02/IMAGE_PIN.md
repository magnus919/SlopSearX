# O02 native image-pin and base-digest tools

## What landed

`crates/xtask/src/o02_image_pin.rs` implements the two operational helpers that
the release jobs used to run as Python, exposed through the existing reviewed
`ci-helper` seam:

| command | replaces |
| --- | --- |
| `cargo xtask ci-helper image-pin-promote --check [--root DIR]` | `scripts/promote_image_digest.py --check` |
| `cargo xtask ci-helper image-pin-promote --write [--root DIR]` | guarded no-op when current; exits `2` rather than partially writing on digest drift |
| `cargo xtask ci-helper image-pin-verify-base [--dockerfile PATH]` | `scripts/verify_docker_base_digest.py` |

Base verification preserves the prior registry behavior. Promotion checks the
unique image digest across `docker-compose.yml`, `k8s/deployment.yaml`, and the
fixture-bound Python test oracle. Native `--write` is fail-closed while that
oracle's source hash is fixed: on digest drift it refuses the write before
changing either manifest, preventing a split pin. Until a separately reviewed
F01/W03 source-custody work defines an atomic promotion path; the existing
image-promotion workflow currently uses the Python writer. Updating its generated
source successor is outside this bounded Docker-verifier wiring PR.

Base verifier policy:
- retains the allowlisted hosts (`auth.docker.io`, `registry-1.docker.io`,
  `ghcr.io`), accepted manifest media types, a 30-second timeout, three attempts
  and 1s/2s backoff for transport errors and HTTP 429/500/502/503/504;
- requests curl's 1 MiB downloaded-body limit (`--max-filesize`), unlike the
  Python predecessor. Enforcement for bodies whose size is initially unknown
  requires curl >=8.4.0; this tool does not check or pin that version. Headers
  captured with `--dump-header -` are not bounded by the option. This is not a
  portable total-response or process-memory limit;
- `--check` uses `0` for current, `1` for drift and `2` for registry/pin errors.
- only HTTPS targets are permitted; the transport rejects anything else before
  spawning `curl`. The tool never builds, pushes or promotes an image.

The transport is `curl`, which the release runners already provide. This is the
"Rust or portable shell client" path the O02 issue allows; parsing, read-only
verification, registry retry classification and base-check behavior are native
Rust. Thirty active tests cover the sealed source contract; no legacy scanner
or ignored scanner tests remain. They cover both-base match/drift verification, failure
classification and non-mutation, injected-transport retry, permanent failure,
unusable digests, explicit promotion modes, bounded Dockerfile and manifest
reads, active-field decoys, symlinked-parent refusal, and drift refusal without
modifying manifests or the test oracle. The non-custodied parser accepts only a
single sealed assignment, not arbitrary Python. The real checkout oracle takes
a separate exact-byte path authenticated against the F01/W03 custody successor,
including the expected module pin. Native write remains no-op on current digest;
digest transitions stay on the existing Python promotion path until source
custody is amended.

## Evidence

The CI/release environment must provide a trusted `curl` in PATH and trusted
curl configuration and environment. The transport inherits HOME/CURL_HOME
configuration and proxy variables as well as executable lookup; this slice does
not isolate those caller-controlled settings.

Historical local production checks (from the image-promotion increment, not
repeated in this bounded wiring slice):

- `image-pin-verify-base` — exit `0`; both `library/rust:1.99.0-bookworm` and
  `library/python:3.14.7-slim-trixie` matched their recorded registry digests.
- `image-pin-promote --check` — exit `1` on a recorded mismatch, exercising
  drift detection and exclusive pin extraction. No credentials or promotion
  write were used here.

Current candidate checks are offline/unit-only:
`SLOPSEARX_TEST_TEMP_ROOT=$SLOPSEARX_BUILD_RUN/tmp cargo test --locked --offline -p xtask o02_image_pin -- --test-threads=8` — 30 passed, none ignored. The passing frozen-source path verifies the exact source SHA-256 and unique expected literal, then checks a compiled text fragment for the W03 successor row's path, historical Git blob, current/predecessor byte counts, and SHA-256 values. This source-fragment check does not execute the row's inverse transform or validate its replacement spans, and it does not check Git status. A separate T08A test exercises the W03 inverse control. The registry transport also launches `curl` by PATH lookup and relies on the caller's toolchain PATH; substituting a hostile `curl` is outside the current verifier's trust boundary. The synthetic parser rejects CR/CRLF line endings, backslash escapes, extra statements, and executable suffixes; it accepts only a single double-quoted digest assignment. Full xtask and the scoped T08A source-binding control were rerun after these parser guards.
`cargo clippy --locked --offline -p xtask --all-targets -- -D warnings` — clean.

## CI wiring and the one behaviour fix

`image-promotion.yml` now runs the native `--check` as a parity cross-check
before the Python `--write`, so the native implementation is exercised against
the real registry on every scheduled run. `docker.yml` now invokes `cargo xtask
ci-helper image-pin-verify-base` after installing and confirming the
repository-pinned Rust toolchain. The Python verifier and test module remain
tracked and source-preserved in the historical fixture; this wiring does not
claim Python-file retirement.

While touching the promotion job we fixed a latent defect: GitHub Actions runs
`run:` blocks under `bash -e`, so the pre-existing `python3
scripts/promote_image_digest.py --write` step aborted on its own documented
non-zero exits (`10` = pins updated, `2` = error) before the `if [ "$code" = 2 ]`
guard could run. Both commands are now wrapped in `set +e` / `set -e`.

## Why the Python twins remain tracked

Every tracked `.py` file and every file under `docs/rust-rewrite/inventory/` is
authenticated by F01's source inventory. The active deployment-test oracle is
also listed in the W03/O02 finite current-source successor and historical inverse:

- `crates/xtask/fixtures/F01.json` lists each path with its `git_blob`;
- `source_check("F01")` requires that set to equal the tracked `.py`/`.pyi`/
  `.pyw`/`ssx` set (minus the retired `docker/healthcheck.py`), and requires the
  Python-disposition inventory to equal the tracked Python set;
- `docs/rust-rewrite/snapshot-fixture-amendment/validator.rs` is sealed by a
  compiled-in capsule SHA, pins `tests/test_coverage_pipeline_inputs.py` as its
  only amendment source, and hard-codes the exact bytes of
  `python-disposition.json`.

`tests/test_deployment_artifacts.py` now asserts that the Docker workflow invokes
the native base verifier and does not invoke the Python verifier. An offline xtask
source regression control also checks the native invocation and ordering. The test
module is an authenticated successor of its retained historical source bytes. The O02 verifier additionally checks the active checkout source SHA-256 against its fixed W03/O02 successor constant and pins its expected digest line; it does not execute the inverse transformation at this runtime boundary. The source-fragment linkage and standalone W03 inverse tests are separate controls.

The current bounded Docker verifier wiring does not delete the Python source or
the test module. It leaves the historical invocation row in the existing F01
inventory; this increment does not change the frozen source fixture, disposition,
or invocation inventory. This is not a claim of G08 source retirement or overall
O02 completion.

## Remaining O02 scope

- Retire `scripts/portal_release_smoke.py` and its source-bound Python deployment-test oracle only through the reviewed F01/G08 successor. The Docker jobs now use the native smoke helper; the historical source remains byte-identical until that retirement is accepted.
- The CI and Docker workflows now call `scripts/configure_ci_dockerhub_mirror.sh`,
  with deterministic fake-Docker tests in the O02-owned `monitoring-rules`
  workflow step and matching CI/F03 workflow catalog entry. This does not add
  `cargo xtask check O02`; the new test runs through that workflow/catalog gate
  only. This is a partial O02 mirror-helper slice: the historical Python helper
  and its test remain byte-identical under F01/W03 custody, and O02 is not
  complete. After a configuration change, the helper bounds the Docker restart
  to 60 seconds, then waits for Docker info readiness with at most 20 probes of
  two seconds each, a one-second KILL grace per probe, and one-second backoff
  (79 seconds maximum, excluding host scheduling delay). It emits a sanitized
  readiness error if the daemon never responds and distinguishes storage-driver
  from driver-status diagnostic failures.
- Replace `scripts/upstream_contracts.py` (`.github/workflows/upstream-contracts.yml`).
- Complete O02's evidence and ownership criteria; source retirement is a separate
  G08 concern, and this verifier wiring does not delete tracked Python files.

The portal-smoke transition updates the current workflow source and executable
gate catalog together, and records both current native invocations in
`PORTAL_SMOKE_TRANSITION.json`. It leaves the F01 invocation, Python-disposition,
and tooling inventories unchanged because they are historical evidence. The
Python source and source-bound deployment test remain until a separately
reviewed G08 retirement amendment. This increment does not complete O02.
