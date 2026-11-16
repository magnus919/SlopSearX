# W07 Valkey receipt persistence increment

This is the storage-only increment for [#913](https://github.com/magnus919/SlopSearX/issues/913).
It implements the existing `ReceiptRepository` port as `ValkeyReceipts` and
supplies unpredictable production receipt IDs. It follows the
[rewrite case study](../CASE_STUDY.md) and [dependency index](../ISSUES.md).
Baseline: `2c7de9c3628c5e5bb61255dff45e019e94019e68`.
F07 (`cfd7a0af61a22cfd040a7fcd41ad950f82005959`, PR #961) and F13
(`730c37a5`, PR #976) are merged ancestors. The application increment
(PR #1029) already provides submission/read/manifest semantics and lineage ports.

The repository borrows F07's `Arc<Valkey>` and its bounded command execution.
The composition owner closes the store. No command retries after dispatch;
transport loss during a write remains `CompletionUnknown`, and an unexpected
script failure remains `ScriptErrorMayHaveWritten`. Cancellation is checked
before dispatch and between reads; a dispatched mutation is awaited. A store
outage returns `Unavailable`, never an empty/missing successful result.

The fixed Lua transition preserves tenant/result-hash namespaces and the four
horizon/index/receipt/idempotency key roles. A first accepted write fixes a
86,400-second logical result-bundle horizon, with a 300-second physical margin.
A same-key/same-digest submission replays; a different digest conflicts. The
index contains at most 20 receipts and each complete encoded receipt is at most
32,768 bytes. New-horizon cleanup is bounded by that same index limit. Read
results preserve the source index count even when a receipt value is absent.
The authenticated tenant is a separate trusted input; body attribution is an
inert caller claim. `observations_verified` and discovery verification fields
are preserved without relabeling them. No supplied URL/reference is fetched.

Malformed snapshot/result/artifact semantics remain owned by the existing
application source reader and domain reference constructors. This repository
bounds result/key/digest inputs, rejects malformed retained objects/references,
and never derives tenant authority from them. Its storage-input cap is not an
assertion that an arbitrary caller string is a server-issued result handle.

## Reviewed differences and source reuse

The predecessor is the preserved dirty worktree on
`codex/w07-valkey-receipts-current` at `1317e3b9e5e5538075c6e79ef32e807bc3d46270`.
The current increment explicitly carries forward its repository/Lua/private
fixture and original eight test bodies, with three additional controls and two
small replay guards. `receipt-repository-source.json` records exact predecessor
and current file hashes. No old worktree, seal, or historical result was edited.
Upstream already contains the predecessor's CI binary/hash/timeout support, so
this increment does not modify the workflow.

The intentional differences from the original Python implementation are:

- Known corrupt state, unsafe cleanup, size/capacity refusal, and ID collisions
  are preflighted before mutation. Lua is not a rollback transaction; unexpected
  execution failures retain their indeterminate classification.
- An existing receipt key or index member cannot be overwritten on ID collision.
  Valid idempotent replay still precedes unrelated index validation.
- Direct replay rejects oversized key/digest inputs before reading, matching the
  submission boundary. A non-string retained digest is malformed rather than an
  idempotency conflict, matching the Lua path.

The [parity ledger](receipt-repository-parity.json) records review/qualification
status. Focused qualification on the exact reviewed runtime source passed:
11 real-Valkey receipt tests, 18 application receipt unit tests, and 13 lineage
unit tests (42 tests total), plus formatting and strict Clippy for all targets
of `ssx-infra` and `ssx-application`. Each test scope's complete inventory matched
its passing names with zero failed or ignored cases. This is storage-only
acceptance; no full-workspace build/test, W07 gate, or production acceptance is
claimed.

The qualified source seal is
`64ed3c1e7743c5bf4e16d3ea3f6afcbff1ae93d9f67b462ca4051cc02bc8f335`;
focused raw/terminal/retirement evidence is sealed by
`2478d9ce0a735db5bb4c4c9c2836868b075079f7905970caee4d7900a88e8b4c`.
The first run passed formatting, then Clippy exited 101 because its PATH omitted
the installed CMake tool. That outcome is retained. The corrected run restored
the PATH from the successful A12 environment, passed commands 2-8 with actual
terminal exit 0, and preserved the first formatting pass. Both runs proved
native-session retirement and removed only their exact owned empty temp roots.
No production Rust/Lua/test bytes changed during or after qualification; the
subsequent documentation update records these results.

## Focused acceptance and qualification handoff

The `w07_receipts` integration target uses a separate real Valkey process per
fixture. Its binary path and SHA-256 are explicit inputs:
`SLOPSEARX_TEST_VALKEY_SERVER` and `SLOPSEARX_TEST_VALKEY_SERVER_SHA256`.
There is no PATH or production endpoint fallback. The fixture pins a regular
executable, creates a private 0700 directory below explicit `TMPDIR`, disables
TCP and persistence, bounds the Unix socket path, and owns/joins the original
child before deleting scratch. On macOS scratch must be on the registered Tank
build root. The fixture is Unix-specific; Linux AMD64/ARM64 CI and local macOS
are the qualification targets. Windows acceptance is not claimed.

The distinct-digest competing-writer control uses two independent Valkey owners
and connections against one private server, releases eight writers together,
and verifies that closing one owner leaves the other usable. Its deterministic
ID ranges are disjoint. The eleven controls cover competing identical/different
submissions, bounded
indexes and no orphan keys, tenant isolation, fixed logical horizon and physical
TTLs, exact horizon expiry, complete-record size, immutable collision handling,
malformed state/reference preflight, missing receipt cardinality, cancellation,
closed/unavailable stores, and a real server-paused indeterminate write with one
ID and no automatic retry. Tests only inspect or mutate their private server;
no global/shared-store flush occurs.

The focused commands above have passed. Complete the remaining workspace,
inherited-gate, hook and published-head checks under separate scheduler
ownership as applicable; the full command inventory is:

```sh
cargo test --locked -p ssx-infra --test w07_receipts
cargo test --locked -p ssx-application --lib receipts::
cargo test --locked -p ssx-application --lib lineage::
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
cargo test --locked --workspace
cargo xtask check F07 --output <registered-evidence>/F07.json
cargo xtask check W00 --output <registered-evidence>/W00.json
cargo xtask check W11 --output <registered-evidence>/W11.json
```

Use only the scheduler-approved target/cache and registered scratch/evidence
paths. Retain exact source/tool/binary identities, command exits and negative
results. Pre-commit and the required CI/independent review remain publication
gates. The scoped W07 gate is still absent and must be implemented with the
full issue's transport/source closure; this increment does not replace it with
a passing stub.

Portal impact: no runtime factory or HTTP/browser boundary adopts this repository
yet, so this increment changes no active portal contract. Later composition must
run the applicable portal gate. #913 stays open for concrete lineage/manifest
composition, MCP tools/resources/auth/catalog adoption, supported output and
cancellation/ownership qualification, a real W07 gate, and downstream handoff.
No performance or production acceptance is claimed.
