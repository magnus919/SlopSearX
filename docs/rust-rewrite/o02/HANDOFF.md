# O02 operations handoff

[O02 #919](https://github.com/magnus919/SlopSearX/issues/919) delivers runtime
and release commands for [O04 packaging](https://github.com/magnus919/SlopSearX/issues/921).
The evidence below is bound to merged revision
`ec3540eb6b3914fe4e4b14a11aa2558ad3a32c0a` except where a different revision is named.

| Operation | Command and result |
| --- | --- |
| Build and startup | The [T02 workflow](../../../.github/workflows/rust-t02-image.yml) built the Linux amd64 native HTTP fixture from [Dockerfile.t02](../../../crates/ssx-http/Dockerfile.t02). The loopback smoke client completed 22 cases. The runtime inventory contained neither Python executables nor libpython. |
| Health | The installed `/usr/local/bin/ssx-healthcheck` exited 1 before startup, 0 at readiness, and 1 after shutdown. It stayed silent and used the loopback route with hostile proxy variables present. [Health contract](HEALTHCHECK.md) |
| Activation and rollback | `bash scripts/rust-native-rollback-rehearsal.sh "$prior" "$candidate"` activated the prior immutable image ID, started and rejected an unhealthy candidate, and reactivated the prior ID. Health exits were 0, 1, and 0. The job removed all three containers. [Merged rehearsal artifact](https://github.com/magnus919/SlopSearX/actions/runs/44561281384/artifacts/14550121587) |
| Upstream contracts | `cargo xtask upstream-contracts --output upstream-report.json` replayed three captured cases through the native adapters on `32ed64b5` with zero network requests. The live workflow requires explicit opt-in. The [transition receipt](UPSTREAM_CONTRACTS_TRANSITION.json) records the probe and output contract. |
| Digest comparison and promotion | The first [scheduled native run](https://github.com/magnus919/SlopSearX/actions/runs/44544619545) ran recovery, check, and write on `dfa5f5f5`. It opened [PR #1158](https://github.com/magnus919/SlopSearX/pull/1158) with five digest replacements across the four allowed files. Each matched the tested OCI index from `dc0cdb4d`. [Command contract](IMAGE_PIN.md) |
| OCI index | The [publication from `dc0cdb4d`](https://github.com/magnus919/SlopSearX/actions/runs/44530953158) verified the index before publishing. Direct registry read-back matched both tested architecture digests. The commit tag and `latest` both served `sha256:98255e295e5a9a497ff0e0a25b10c649f1f305011c0d7b077c79f0f1da956573` at verification. |
| Base images, portal smoke, and mirror configuration | Docker CI ran `cargo xtask ci-helper image-pin-verify-base` before build and `cargo xtask ci-helper portal-release-smoke` for both themes. The CI workflow exercised `scripts/configure_ci_dockerhub_mirror.sh` in the [monitoring-rules job](https://github.com/magnus919/SlopSearX/actions/runs/44561281801/job/135690934803). [Docker workflow](../../../.github/workflows/docker.yml) |

The pinned Rust toolchain and offline Cargo cache ran all 100 cases in
`cargo xtask check O02 --output "$O02_EVIDENCE_ROOT/O02.json"` on the merged
revision. The gate includes mismatched-digest rejection, promotion transaction
failure and recovery, and failed readiness. [Qualification PR](https://github.com/magnus919/SlopSearX/pull/1155)
· [Rollback PR](https://github.com/magnus919/SlopSearX/pull/1156)

The retained `merged-O02.json` receipt records a clean checkout of that revision,
2,472 source blobs, and four successful test commands. They passed 3 healthcheck
tests, 95 O02 tests, and 2 image-publication controls. The receipt SHA-256 is
`5e8178727da2863add0e5341c6531a9f0f4f6166533b8032a7b47ded0b3d3c4e`.
The [healthcheck qualification](HEALTHCHECK_QUALIFICATION.md) links the separate
hosted amd64 and ARM64 shipping-image checks.

The healthcheck now uses a fixed loopback connection, bypasses proxy settings
and redirects, and bounds headers and elapsed time. The base verifier applies
curl's downloaded-body limit, subject to the curl version's enforcement of
unknown-size bodies. The upstream check defaults to offline replay. These
operational differences are recorded in the [health contract](HEALTHCHECK.md),
[image-pin contract](IMAGE_PIN.md), and [upstream transition](UPSTREAM_CONTRACTS_TRANSITION.json).

O04 consumes the native commands and repeats build, activation, readiness, and
rollback with the assembled release images. The current shipping image starts
the Python server. The Python-free witness here is the scoped native HTTP
fixture. [G08](https://github.com/magnus919/SlopSearX/issues/946) owns historical
backend Python source retirement. [G07](https://github.com/magnus919/SlopSearX/issues/945)
owns production activation.
