# Native healthcheck qualification — partial O02

Read the [case study](../CASE_STUDY.md), [healthcheck contract](HEALTHCHECK.md) and [O02 #919](https://github.com/magnus919/SlopSearX/issues/919). Local acceptance passed on 2026-11-01. Hosted amd64/ARM64 image acceptance remains pending. This document adds no runtime change and does not close O02.

The qualified source is `fff50a1634b283a1c5f49b7d0a5cc5fae06716ee`. Health package bytes were unchanged from `1339abea4a723a508b822482895f0930d7396094`; their successful native results retain that producer revision. The successor corrected a local name shadowing the historical custody-reader function and its exact mutable binding. The original compiler failure remains retained; no historical oracle or lock span was rewritten.

| Actual command | Producer revision | Exit / outcome |
| --- | --- | --- |
| `cargo test --locked --offline -p ssx-healthcheck --message-format=json-render-diagnostics -- --test-threads=1` | `1339abea` | 0; 3 passed, 0 failed, 0 ignored |
| `cargo build --locked --offline -p ssx-healthcheck --bin ssx-healthcheck --message-format=json-render-diagnostics` | `1339abea` | 0; normal non-test executable selected from compiler output |
| `cargo clippy --locked --offline -p ssx-healthcheck --all-targets -- -D warnings` | `1339abea` | 0 |
| `cargo fmt --all --check` | `fff50a16` | 0 |
| `cargo test --locked --offline -p xtask --bin xtask --no-run --message-format=json-render-diagnostics` | `fff50a16` | 0 |
| Selected xtask test image: `a13::tests::compiled_inventory_is_closed --exact --nocapture --test-threads=1` | `fff50a16` | 0; 1 passed, 0 failed, 0 ignored |

The three real-loopback tests cover HTTP 200 without reading absent/invalid bodies; non-200, redirects, malformed/truncated/oversized headers; refused connection and slow-header deadline. They exercise owned ephemeral TCP endpoints. They do not execute the shipping main on fixed port 8080 or establish container behavior.

The A13 owning reader reaches the existing shared-source controls and `native_health_lock_prior`: actual accepted lock reconstruction plus old/missing, duplicate and modified-package rejection. All 245 current T08A source-binding rows were separately verified; that static admission is not 245 runtime tests. No unchanged T08/T07 runtime or CLI compatibility corpus was replayed for this helper increment.

The current O02 retirement-control test requires both `TMPDIR` and `SLOPSEARX_TEST_TEMP_ROOT` to name the same absolute, existing, non-symlink directory. The Foundation workspace-test job creates that directory under `${RUNNER_TEMP}` and removes it on exit. For local runs, configure both variables under the persistent Rust build root as described in [`RUST_BUILD_FOOTPRINT.md`](../../RUST_BUILD_FOOTPRINT.md); the control does not fall back to the system temporary directory. It creates an exclusive child directory and reports both control failures and cleanup failures.

The actual normal native-host debug executable is 471,744 bytes, compiler `profile.test=false`, with no package features. Its SHA-256 is `e024671fb140d7074d51e2046a96d8368b9c42e78e148de9347eaa2a6cdd6438`; its terminal rehash matched. This identifies the local normal image, not the future Linux release/container images. The selected current xtask test image SHA-256 is `9c26c7bd901c144e328b5a6f7b189644404611d7644f60016849c73324d1218d`.

The retained native evidence root is `/Volumes/tank01/magnus/rust-builds/slopsearx/o02-healthcheck-native-20261101/`. `FINAL-HANDOFF.json` is 12,871 bytes, SHA-256 `4b03ed1c75452a99c07caf19ebe888247b0206e87d7e8933defe365d6956e942`. It binds commands, compiler artifacts, raw stdout/stderr, source snapshots, the original compile failure, retained-input comparison and cleanup. All 24 linked artifacts were independently checked against their recorded hashes/lengths before this document was authored. Shared source/target/cache leases were released and owned temporary files removed.

Source checks: 24 deployment-artifact tests passed with `--no-cov`; the earlier subset invocation also passed all 24 tests but failed the inherited whole-backend 80% coverage threshold. Its original output is retained, and no full-backend coverage claim is made. Scoped hooks passed. Full-file hooks report the known unrelated Ruff-format baseline in `tests/test_coverage_saved_method_materials.py`; the AST-equivalent rewrite was restored exactly. DCO commits ran the installed AST graph hook. The final documentation successor is checked separately; native results retain the producer revisions above.

Local Docker/Podman are unavailable. Fixed-port shipping-main execution, interpreter-absent runtime execution and the actual amd64/ARM64 Docker image witness remain unrun locally. Existing Docker CI must build/test the helper and run its actual shipped binary against the server with hostile proxy settings, then require silent exit 1 in an otherwise empty container. Exact published-head CI/review acceptance remains required before merge.

The image still contains the Python backend and its uvicorn entrypoint. Base verification remains an interim Python tool; digest promotion, portal smoke, mirror configuration and upstream probes remain O02 work. Final native packaging and backend retirement remain O04/G08 responsibilities. No full O02 gate, completed Python-free server, production activation, live-provider result, performance improvement or retrieval-quality improvement is claimed.
