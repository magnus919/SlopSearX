# Native acceptance evidence

Run `cargo xtask check F01`, `cargo xtask check F02`, or
`cargo xtask check F03 --output target/rust-evidence/F03.json` from the workspace.
Before dispatching acceptance checks, every invocation verifies that the actual
`rustc --version --verbose` header and release match the accepted F02 Rust 1.99.0
pin in both `architecture/pins.json` and `rust-toolchain.toml`. Missing, malformed,
duplicate or mismatched release identity fails, including compiler overrides via
`RUSTUP_TOOLCHAIN`.
Every invocation emits one JSON object on stdout and exits nonzero on failure.
Without `--output`, stdout is the artifact; the optional path is explicit and a
write error fails acceptance. Output destinations are resolved through existing
symlink ancestors and rejected if they overlap tracked files, owned foundation
source or repository metadata, including Git directories outside linked worktrees.
Inside the worktree, only explicitly ignored
artifact destinations are accepted; paths outside the worktree are also allowed.
Validation occurs before checks and again before writing, including failure
reports. Atomic replacement preserves source bytes behind hardlinked outputs.
Cargo diagnostics go to stderr. No Python helper
participates.

F01 compares the complete tracked Python source path set to the disposition
manifest and requires nonempty classifications/dispositions. Its checked-in
fixture binds every inventoried Python source and inventory document to its Git
blob hash. F02 requires the merged accepted F02 revision as an ancestor and binds
all three ADRs, architecture documents and pins to their exact bytes. These
checks revalidate source inventory and decisions; they do not rerun historical
Python regression tests or certify native implementation of those contracts.
Changes fail as stale until the owning lane reviews and updates both the
inventory/decision and its fixture. Git blob identifiers describe file bytes;
they are not signatures or an independent security attestation.

F03 requires F01 and F02 checks, `cargo fmt --all --check`, strict Clippy across
all targets, locked tests, a locked workspace build, and real TCP readiness from
the freshly built `target/debug/ssx-server`. The runner starts it on ephemeral
loopback with an empty PATH, validates the startup descriptor and exact
`GET /healthz` response, and terminates/joins it on either outcome. Startup and
socket reads have finite timeouts. The foundation response explicitly says
`foundation`. This is not a search, MCP, worker, gateway, production TLS or
feature-parity claim. CI separately runs the complete workspace test suite,
supply-chain gates and the no-interpreter runtime smoke.

Nested tests exclude xtask to avoid recursively running the acceptance runner.
The runner itself is covered by the outer `cargo test --locked --workspace` gate;
its negative tests reject unknown and unimplemented IDs, missing/malformed/empty
fixtures, duplicate sources and plan IDs, path escape, stale bytes and incomplete
source coverage. Cargo releases its build lock before launching the xtask
executable; nested Cargo commands run sequentially.

All other known IDs in `plan.json` fail explicitly as unimplemented. Merely adding
a plan row, counting fixtures, writing a `passed` field or supplying arbitrary
commands cannot make a lane pass: a reviewed native check implementation and its
owned fixtures must be added. F04 owns portable replay and deliberate behavioral
difference detection; this foundation runner does not substitute for it.

Evidence schema version 1 records `plan_id`, `status` (`passed` or `failed`),
`error`, `scope`, `output_path`, start/end Unix milliseconds, `toolchain`, `source`, `identity_commands` and
`commands`. Source identity includes HEAD, tracked diff, working-tree status and
Git blob hashes for tracked/untracked Rust workspace, CI workflow, supply-chain policy, smoke script and rewrite decision files.
Each command records program, argument vector, exit code, stdout and stderr;
readiness additionally records its compiled executable Git blob hash (verified unchanged after smoke), empty PATH, startup, request and response.
The runner rejects a source snapshot changed during execution. Reports describe the checkout tested, including uncommitted files, rather than
asserting that an earlier HEAD alone contains the implementation. Retain CI
artifacts with the corresponding exact revision and check results. A failed
report is evidence of failure, never an acceptance artifact.
