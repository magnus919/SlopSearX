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

`tests/test_deployment_artifacts.py` retains the Python base-verifier assertion;
the native workflow invocation is asserted by an offline xtask source regression
control. The Python test module and its historical source bytes remain unchanged. The O02 verifier additionally checks the active checkout source SHA-256 against its fixed W03/O02 successor constant and pins its expected digest line; it does not execute the inverse transformation at this runtime boundary. The source-fragment linkage and standalone W03 inverse tests are separate controls.

The current bounded Docker verifier wiring does not delete the Python source or
the test module. It leaves the historical invocation row in the existing F01
inventory; this increment does not change the frozen source fixture, disposition,
or invocation inventory. This is not a claim of G08 source retirement or overall
O02 completion.

## Remaining O02 scope

- Replace `scripts/portal_release_smoke.py` (two `docker.yml` uses).
- Replace `scripts/configure_ci_dockerhub_mirror.py` (`ci.yml`, `docker.yml`).
- Replace `scripts/upstream_contracts.py` (`.github/workflows/upstream-contracts.yml`).
- Complete O02's evidence and ownership criteria; source retirement is a separate
  G08 concern, and this verifier wiring does not delete tracked Python files.

The remaining workflow migrations require updating `tests/test_deployment_artifacts.py`,
`docs/rust-rewrite/inventory/python-invocations.json`, current-source admissions,
and the executable gate catalog in the same reviewed changes. F01 retains its
original fixture and inventories the historical Docker command; no source deletion
or invocation retirement is claimed by this increment.
