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

Base verifier policy remains:
- the same allowlisted hosts (`auth.docker.io`, `registry-1.docker.io`,
  `ghcr.io`), the same accepted manifest media types, a 30-second timeout, a
  1 MiB response cap, three attempts and 1s/2s backoff for transport errors and
  HTTP 429/500/502/503/504;
- `--check` uses `0` for current, `1` for drift and `2` for registry/pin errors.
- only HTTPS targets are permitted; the transport rejects anything else before
  spawning `curl`. The tool never builds, pushes or promotes an image.

The transport is `curl`, which the release runners already provide. This is the
"Rust or portable shell client" path the O02 issue allows; parsing, read-only
verification, registry retry classification and base-check behavior are native
Rust. Twenty-four unit tests include both-base match/drift verification, failure
classification and non-mutation, injected-transport retry, permanent failure,
unusable digests, explicit promotion modes, bounded Dockerfile and manifest
reads, active-field decoys, strict oracle assignment validation, and drift refusal
without modifying the manifests or test oracle. A same-digest native write is a
successful no-op; digest transitions stay on the existing Python promotion path
until source custody is amended.

## Evidence

Historical local production checks (from the image-promotion increment, not
repeated in this bounded wiring slice):

- `image-pin-verify-base` — exit `0`; both `library/rust:1.99.0-bookworm` and
  `library/python:3.14.7-slim-trixie` matched their recorded registry digests.
- `image-pin-promote --check` — exit `1` on a recorded mismatch, exercising
  drift detection and exclusive pin extraction. No credentials or promotion
  write were used here.

Current candidate checks are offline/unit-only:
`SLOPSEARX_TEST_TEMP_ROOT=$SLOPSEARX_BUILD_RUN/tmp cargo test --locked --offline -p xtask o02_image_pin -- --test-threads=8` — 24 passed, including active-field decoys, bounded inputs, oracle duplicates, missing pins and promotion drift refusal.
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
authenticated byte-exactly by F01 source custody:

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
control. The Python test module and its historical source bytes remain unchanged.

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
