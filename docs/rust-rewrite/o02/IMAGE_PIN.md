# O02 native image-pin and base-digest tools

## What landed

`crates/xtask/src/o02_image_pin.rs` implements the two operational helpers that
the release jobs used to run as Python, exposed through the existing reviewed
`ci-helper` seam:

| command | replaces |
| --- | --- |
| `cargo xtask ci-helper image-pin-promote [--check\|--write] [--root DIR]` | `scripts/promote_image_digest.py` |
| `cargo xtask ci-helper image-pin-verify-base [--dockerfile PATH]` | `scripts/verify_docker_base_digest.py` |

Behaviour is intentionally identical to the Python originals:

- the same pinned image (`ghcr.io/magnus919/slopsearx`), the same three pin files
  (`docker-compose.yml`, `k8s/deployment.yaml`, `tests/test_deployment_artifacts.py`),
  and the same fail-closed rule that the three files must carry exactly one
  unique pinned digest;
- the same allowlisted hosts (`auth.docker.io`, `registry-1.docker.io`,
  `ghcr.io`), the same accepted manifest media types, a 30-second timeout, a
  1 MiB response cap, three attempts and 1s/2s backoff for transport errors and
  HTTP 429/500/502/503/504;
- the same exit codes — `0` current, `1` drift under `--check`, `10` pins
  rewritten, `2` pin-state or registry error;
- only HTTPS targets are permitted; the transport rejects anything else before
  spawning `curl`. The tool never builds, pushes or promotes an image.

The transport is `curl`, which the release runners already provide. This is the
"Rust or portable shell client" path the O02 issue allows; the parsing,
rewriting, retry classification and exit-code logic are native Rust and covered
by 12 unit tests, including injected-transport retry, permanent-failure and
unusable-digest controls.

## Evidence

Local, against the real registries:

- `image-pin-verify-base` — exit `0`; both `library/rust:1.99.0-bookworm` and
  `library/python:3.14.7-slim-trixie` reported identical pinned and registry
  digests.
- `image-pin-promote --check` — exit `1`, reporting the pinned
  `sha256:91194d14…` against registry `sha256:5b795849…`; drift detection and
  the exact/exclusive pin extraction both exercised.

`cargo test -p xtask o02_image_pin` — 12 passed.
`cargo clippy -p xtask --all-targets -- -D warnings` — clean.

## CI wiring and the one behaviour fix

`image-promotion.yml` now runs the native `--check` as a parity cross-check
before the Python `--write`, so the native implementation is exercised against
the real registry on every scheduled run. `docker.yml` still invokes the Python
base-digest verifier; the native command is available and unit-tested but not
yet wired there (see the blocker below).

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

`tests/test_deployment_artifacts.py` also asserts that `docker.yml` contains the
Python verifier command and that `image-promotion.yml` contains
`scripts/promote_image_digest.py --write`.

So deleting either script, editing that test, or changing an inventoried
workflow line is not a routine edit: it needs a new reviewed sealed amendment
covering the prior/current bytes. That is deliberately out of scope here. Until
it lands, the Python files stay tracked and the Python writer stays
authoritative; the native tool is a proven, exercised replacement rather than a
claim that O02 is complete.

## Remaining O02 scope

- Wire `image-pin-verify-base` into `docker.yml` and drop the Python verifier.
- Replace `scripts/portal_release_smoke.py` (two `docker.yml` uses).
- Replace `scripts/configure_ci_dockerhub_mirror.py` (`ci.yml`, `docker.yml`).
- Replace `scripts/upstream_contracts.py` (`.github/workflows/upstream-contracts.yml`).
- Land the sealed F01 amendment that retires the superseded Python sources and
  updates `python-disposition.json` / `python-invocations.json`, then delete the
  scripts and their Python unit tests.

The two `docker.yml` items are additionally blocked by
`tests/test_deployment_artifacts.py`, which asserts that the workflow contains
the Python verifier command and the promotion write command. All four items also
invalidate `docs/rust-rewrite/inventory/python-invocations.json`, whose exact
bytes F01 authenticates and which must therefore be updated in the same reviewed
change. The sealed amendment is consequently the gating dependency for the whole
remainder, not just for deleting the superseded files.
