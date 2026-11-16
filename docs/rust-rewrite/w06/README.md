# W06 — bounded staged execution

Source baseline: `9b7d888da1ee3f7e2ed045c2185ea16f419381e9`, fetched and
verified equal to upstream main before this work. Issue: #912. W11, W02,
W04 and F15 are accepted prerequisites. This packet uses synthetic offline
inputs, loopback acquisition, and disposable private Valkey. No provider or
production requests are authorized or represented by it.

Current local integration includes accepted A09 main `6c449764` through normal
merge `0b35624d`. W06 verified all 303 exact completed cases; W04/W11/W02/A05/A08
also passed. A09/F01/O06 passed after the reviewed artifact-only correction at
`ba671ae2`. Strict workspace Clippy and formatting passed. The
[post-A09 receipt](evidence/post-a09-qualification303.json) preserves the separate
source commits and check limits. The [post-A05 receipt](evidence/post-a05-qualification303.json),
earlier `4cc5858c` receipt and 151-case mutation packet remain historical evidence.
Fresh published-head CI/review and parent delivery acceptance remain required.
Read [HANDOFF.md](HANDOFF.md).

`source.json` records complete source plan/error envelopes and identity bytes,
claimed records, actual runner responses and dispatch requests, complete final
records/canonical bytes, snapshot creation inputs, rendered views, replay
equivalence, and saved-report source resolution. Every captured runner request
uses the actual Python runner. The temporary exporter imports only the pinned
source after checking source SHA256 guards, including under `python -O`.
It is manual migration tooling, inventoried for G08 removal; native execution,
CI and the mandatory W06 gate never invoke it.

The capture clock and opaque IDs are fixed explicitly. UUID integers are
shifted left 80 so snapshot IDs, which use the first 12 hexadecimal characters,
remain distinct. Query IDs and injected response latencies are fixed inputs.
Original runner record comparisons remove fixture-only
`owner_token`: the Python in-memory fallback clears it, while durable Lua and
the accepted W11 record retain the acknowledged token. Lease authority remains
the actual fence in either implementation. Missing/null fields, engine provenance, error
envelopes and canonical plan bytes remain meaningful. Fresh public-admission
comparisons bind one opaque operation ID and add only the separately accepted
F09 pagination-enforcement entry to the source expectation; seeded replay
records retain their exact captured shape.

`regression-map.json` lists original functions, parameters, decorators and body
hashes, identifies helper and downstream-only code, and names assertion
obligations. It is navigation, not execution evidence. `qualification.json`
binds every source case and each supplemental native case to a concrete test.
The gate verifies exact listed native tests, executes the tests, rejects
ignored/missing tests and requires each expected case receipt exactly once
after its assertions and cleanup. A total count cannot satisfy the gate.
The frozen binding requires 248 source cases: the immutable original 85,
13 retained-policy, 13 saved-source-authority, 121 public-error/replay and 16
exact-number truthiness cases. It also requires 55 native/rendering/hardening
cases across six application and 31 infrastructure semantic tests, plus one
explicit subprocess probe (38 listed tests). The probe's actual killed-owner
lifecycle is asserted by its parent semantic test. Four additional original
runner scenarios are explicitly source-only reference controls; their native
boundary pairings are documented without false execution receipts. The gate
also pins the reviewed 14 runtime/test source identities and all six producer
identities, and checks the union of original source guards against baseline
Git blobs. See [the public-input handoff](evidence/review-public-handoff.md).

Two rejected provisional captures remain immutable under `evidence/`: the
first used an invalid composite saved-report ID, and the second used UUIDs
whose snapshot prefixes collided. Neither is qualification evidence. Explicit
exporter branch checks now reject the first fixture failure.

Use the pinned Rust1.99.0 toolchain and Valkey8.1.10. Build artifacts belong on
the selected tank01 build directory; incremental compilation is disabled.

```sh
export RUSTUP_HOME=/private/tmp/slopsearx-rust-tools/rustup
export CARGO_HOME=/private/tmp/slopsearx-rust-tools/cargo
export CARGO_TARGET_DIR=/Volumes/tank01/magnus/rust-builds/slopsearx/w06-gate
export CARGO_INCREMENTAL=0
export SLOPSEARX_TEST_VALKEY_SERVER=/private/tmp/ssx-f07-valkey-pinned/bin/valkey-server
cargo +1.99.0 xtask check W06 --output "$CARGO_TARGET_DIR/evidence/W06-final-union-303.json"
cargo +1.99.0 fmt --all --check
cargo +1.99.0 clippy --locked -p ssx-application -p ssx-infra -p ssx-mcp -p xtask --all-targets -- -D warnings
```

Do not run concurrent workspace builds into this target. Gate evidence retains
command stdout/stderr and failure status. See [PARITY.md](PARITY.md) for
boundaries, reviewed differences and downstream obligations. A successful
local packet does not itself establish transport, portal, production or final
Rust-only release parity.

Direct `ruff check .` and scoped W06 hooks pass. The immutable qualified
exporter's long metadata string has an exact-file E501-only Ruff exception;
all its other selected rules remain checked. The earlier all-files run also
reformatted one unrelated existing test; that file was restored byte for byte.
The receipt preserves both original failures and makes no all-files-green claim.
