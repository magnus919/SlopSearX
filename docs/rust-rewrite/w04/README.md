# W04 — saved execution and atomic reports

Baseline: `1bad48bbe4e25e2f60d5b86668e92bbce922fdf0` (fresh upstream main,
W00/F15 and W02 accepted). Issue: #909. No providers, production state,
scientific calls or deployment were used. Native mandatory execution and
qualification do not invoke Python.

`ssx_application::saved::observe` captures full canonical first-page source
facts, then bounds the observation. `compare_observations` preserves source
identity/event bytes, sorted reasons, source provenance and baseline retention.
The committed 21-case synthetic offline corpus was generated against the exact
source files whose SHA256 identities it records. Native replay checks report
AsciiSpaced SHA256 and byte length, decoded reports/baselines and observation
facts, including multiengine provenance and null/empty publication dates.

`SavedRunner` uses the shared fail-closed search service with `prefer_fresh`,
no presentation slice, an owned cancellation token and a monotonic execution
timeout. It renews authority during acquisition and publication, cancels and joins owned
acquisition after lease loss/shutdown, and releases claims after failure.
Dropping an outer `run_one` signals its detached cleanup task; cleanup can
continue after the caller disappears. An external request may already have
been accepted. No exactly-once external dispatch or rollback of unknown writes
is claimed. The shared search service is never shut down to cancel one run.
`run_due` repairs/discovers a bounded batch and owns at most two local runs;
`run` polls bounded tenant pages until joined shutdown. Valkey remains the
schedule/lease authority; persisted wall time is separate from monotonic time.

`SavedStore::commit` takes an opaque immutable W00 claim. Exact Rust validation
and raw-record CAS fence current revision, tenant, policy fingerprint and active
state; Lua checks live lease ownership and authoritative server time. All key
kinds, report eviction keys and TTL limits are preflighted before any write.
The single script publishes report, report index, definition/next due and
optional replacement baseline, removes the lease and deletes evicted reports.
An incomparable observation retains the previous baseline and its independent
TTL. Shared index TTLs are extended rather than shortened; this is the reviewed
intentional hardening in PARITY.md. Outbox publication remains W05/W09.

Report reads revalidate both the live definition and the retained report's
original engine scope. A second exact definition snapshot rejects concurrent
mutation during the read. `SavedAuthority` rereads the injected authority before
dispatch and publication; a mutable real authority fixture proves midflight
revocation. `StaticSavedAuthority` is concrete immutable native composition.
It does not implement production hot reload. W09 must bind dynamic deployments
to their live grants/config lifecycle and preserve both revalidation boundaries.

## Verification

Use the pinned Rust1.99 and private Valkey8.1.10 environment. All local build
artifacts are on the user-selected tank01 build directory with incremental
compilation disabled:

```sh
export CARGO_TARGET_DIR=/Volumes/tank01/magnus/rust-builds/slopsearx/w04
export CARGO_INCREMENTAL=0
export SLOPSEARX_TEST_VALKEY_SERVER=/private/tmp/ssx-f07-valkey-pinned/bin/valkey-server
cargo xtask check W04 --output "$CARGO_TARGET_DIR/evidence/W04.json"
cargo fmt --all --check
cargo clippy --locked -p ssx-application -p ssx-infra -p xtask --all-targets -- -D warnings
cargo check --locked -p ssx-application -p ssx-infra -p xtask --all-targets
```

The gate rejects source/exporter drift and changed exact case/test inventories;
it executes real private Valkey, held loopback sockets and a killed native
worker subprocess. Test names locate assertions; counts alone do not establish
parity. See PARITY.md for source-test mapping and unclosed downstream obligations.
`capture_source.py` is explicitly inventoried in oracle-inventory.json for G08
retirement. It is never imported/invoked by native code, CI or the W04 gate.

Portal impact: the new native runner/storage APIs are not yet wired to HTTP,
MCP or portal routes. Existing wire/HTML contracts are unchanged. W09/W10 must
qualify the composed saved workflow and portal behavior before cutover. No
transport/production parity or metric-gauge parity is claimed by W04.
