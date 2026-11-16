# Native workspace foundation (F03)

The ten `ssx-*` crates implement the ADR 003 ownership graph. `xtask` is a separate native acceptance runner. Rust 1.99.0 and every candidate direct dependency from `architecture/pins.json` resolve unchanged into `Cargo.lock`. Always use `--locked`; the lockfile owner must regenerate it deliberately when manifests change. This is a compiled foundation, not search, MCP, storage, worker or client feature parity.

`ssx-domain` owns internal values; `ssx-wire` owns public DTOs. Native `ssx` and `ssx-gateway` depend on wire DTOs and protocol/client libraries without importing backend domain or application crates. `ssx-application` owns explicit runtime ports. Infrastructure implements those ports; HTTP receives the same injected runtime used by its deterministic tests. A later MCP handler can receive this bundle without changing transport wiring. The probe-only engine/store methods are foundation seams, not complete future adapter or durable-store APIs.

The fake engine returns classified outcomes. The in-memory fixture store preserves found/missing/expired/unavailable distinctions and never acts as production shared authority. `FixedClock` separates persisted Unix seconds from process-local monotonic elapsed time. Metrics and audit have no-op/recording sinks; the supervisor sink records typed started/completed/failed/cancelled events. F14 owns real task spawn, cancellation, joining and shutdown; O01 owns production exporters. No test fixture contacts providers or Valkey.

Build and verify from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
cargo xtask check F03
```

The focused tests exercise state injection through the real Axum router, 200/503 readiness mapping, a 501 search route, unavailable and missing store outcomes, classified engine failure, deterministic clocks and typed sink events. The runner additionally exercises the actual compiled process with no Python executable on PATH. F04 owns fixture replay and nonzero differential evidence; no placeholder reports feature acceptance.

Run `target/debug/ssx-server --listen 127.0.0.1:0`. The process emits a JSON line containing `address` and `mode: foundation`, then serves `GET /healthz` with `{"status":"ok","mode":"foundation"}`. This only reports the injected foundation probes. The listener rejects non-loopback addresses. Every other application route returns 501. SIGINT requests graceful listener shutdown. The binaries `ssx`, `ssx-worker` and `ssx-gateway` expose `--help`; every operational command currently exits nonzero with an explicit unimplemented error.

The foundation introduces no Python production changes and does not replace an existing endpoint, image or deployed listener. Portal impact is therefore limited to a separate native foundation route. Native search and portal contracts remain pending their implementation gates; existing Python portal CI remains required.

Dependency licenses/advisories are checked by the native CI supply-chain gate; the foundation jobs separately inspect the locked TLS feature graph for platform verification and unexpected native-TLS/OpenSSL backends. The compile graph includes candidate parsing, Valkey and MCP libraries to expose compatibility early; their compilation does not prove protocol, root-store or storage behavior. T01/O02/O04 own real protocol/TLS/platform qualification. F03 evidence describes the available host only; Linux amd64/arm64 CI and later release-platform qualification are separate evidence.
