# O02 native image-pin and base-digest tools

## What landed

The native `ci-helper` commands are:

| command | behavior |
| --- | --- |
| `cargo xtask ci-helper image-pin-promote --check [--root DIR]` | compare only; 0 current, 1 drift, 2 error; pending transactions fail without mutation |
| `cargo xtask ci-helper image-pin-promote --write [--root DIR]` | validate/stage/write digest pins with rollback; 0 current, 10 updated, 2 error |
| `cargo xtask ci-helper image-pin-promote --recover [--root DIR]` | explicit offline transaction rollback/cleanup; 0 clean/recovered, 2 error |
| `cargo xtask ci-helper image-pin-verify-base [--dockerfile PATH]` | verify both Docker base digests |

The [native writer and source-custody transition](image-promotion-write-v1/README.md)
defines the exact digest-slot successor of the existing literal test oracle,
semantic active-field validation, durable originals, compensating rollback,
interrupted-transaction recovery, current source bindings and offline controls.
The scheduled workflow invokes native recover/check/write; the historical Python
helper/source inventories remain preserved for G08. No promotion workflow or
registry write was executed while implementing this transition.

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
Rust. Focused offline tests cover native parsing, registry behavior and the new
transaction/custody failure controls. See the additive transition for the exact
controls and registered run evidence. The non-custodied parser remains a sealed
literal-only grammar; a real source successor must preserve every non-digest byte.

## Evidence and trust boundaries

The CI/release environment must provide a trusted `curl` in PATH and trusted
curl configuration/environment. The transport inherits HOME/CURL_HOME, proxy
variables and executable lookup. This transition does not isolate those inputs.

The writer updates four operands together: `docker-compose.yml`,
`k8s/deployment.yaml`, the literal digest in
`tests/test_deployment_artifacts.py`, and exactly two active YAML examples in
`spec.md`. It uses a durable journal, per-file atomic replacement, compensating
rollback and explicit offline recovery. Checks refuse pending recovery state
without mutation. The scheduled job uses native commands and records only these
four paths in its generated PR; no workflow dispatch, registry write or
production deployment was performed for local validation.

The historical Python helpers remain tracked until G08 retirement. The deployment
test is a bounded current-source successor: one exact assertion replacement
requires native workflow wiring, and a unique digest slot may vary; every other
byte remains fixed. Existing historical F01/W03 and builder-v2 proofs reconstruct
the original authenticated source. The native parser still rejects arbitrary
Python instead of interpreting it. Current source bindings and their finite audit
are part of the additive transition; historical inventories remain unchanged.

Historical live registry observations in earlier image/base-verifier increments
were not repeated for this writer. Current validation is offline: injected
registry transport, semantic pin parsing, exact-source projection, rollback at
every replacement boundary, interrupted/failed recovery, journal ownership and
static workflow/catalog plus shell exit-code controls. Registered command receipts
retain exact source/head, toolchain, commands, exits and measured build footprint.
These checks do not establish registry availability or production readiness.

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
- `.github/workflows/upstream-contracts.yml` now invokes the native
  `cargo xtask upstream-contracts --live` runner only when a manual dispatch
  sets `run_live: true` or the weekly schedule is explicitly enabled with
  `LIVE_UPSTREAM_CONTRACTS=true`. The command's default local mode is offline
  fixture replay through the actual Rust adapters. The historical Python
  script and test remain frozen and byte-identical until separately reviewed
  G08 source retirement; this completes only the upstream-probe invocation
  portion of O02.
- Complete O02's evidence and ownership criteria; source retirement is a separate
  G08 concern, and this verifier wiring does not delete tracked Python files.

The portal-smoke transition updates the current workflow source and executable
gate catalog together, and records both current native invocations in
`PORTAL_SMOKE_TRANSITION.json`. It leaves the F01 invocation, Python-disposition,
and tooling inventories unchanged because they are historical evidence. The
Python source and source-bound deployment test remain until a separately
reviewed G08 retirement amendment. This increment does not complete O02.

## Native publication verification

The manifest publication job builds the repository-pinned Rust verifier. It
checks the Buildx dry-run index before pushing tags, then checks the actual
index behind each tag. Both checks require one Linux amd64 descriptor and one
Linux arm64 descriptor, with the exact child digests emitted by the tested
architecture jobs. The scope is index contents and tested child identity.
