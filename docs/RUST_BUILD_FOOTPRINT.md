# Rust Build Footprint

Use this checklist for local Rust development, qualification, and PR delivery.
It records where build material goes, who owns it, and when it can be removed.

## Before building

- Keep build artifacts outside the checkout under a configured persistent
  build root. On this Mac, use
  `/Volumes/tank01/magnus/rust-builds/slopsearx/<issue-or-pr>/<run>/`; on
  another host, configure an equivalent persistent root. Place Cargo targets
  and caches, source copies, overlays, archives, logs, receipts, handoffs, and
  temporary and tool-cache paths under that root. Set `CARGO_TARGET_DIR`,
  `CARGO_HOME`, `TMPDIR`, `TMP`, `TEMP`, and `XDG_CACHE_HOME` explicitly. Set
  `PRE_COMMIT_HOME` for hook environments, Ruff's `RUFF_CACHE_DIR` (or
  `--cache-dir`), and mypy's `--cache-dir` or `cache_dir` configuration to
  owned paths under the build root. Record shared caches with their owner,
  size, reuse reason, and review/prune date. Do not fall back to `/private/tmp`,
  `/tmp`, or `.codex` worktree locations without explicit user consent. CI may
  use its designated ephemeral job directory when the job cleans it up.
- **Unix-domain socket fixtures on macOS:** the test fixture enforces a
  `sockaddr_un` path shorter than 104 bytes. macOS's per-user default `TMPDIR`
  may be too long once the fixture UUID directory and `s.sock` are appended.
  For socket-backed test commands, have the bounded run own the short Tank
  temporary base `/Volumes/tank01/magnus/rust-builds/slopsearx/tmp` and pass it
  explicitly as `SLOPSEARX_TEST_TEMP_ROOT`, `TMPDIR`, `TMP`, and `TEMP`; the
  fixture-specific override takes precedence over the process temporary
  directory. Do not fall back to the macOS default, `/private/tmp`, or `/tmp`.
  The reviewed T06 helper's `/ssx-t06-<32-hex>/s.sock` suffix makes
  the complete path 96 UTF-8 bytes under that base, below the fixture's bound.
  The base is exclusive to one task/run: the owner creates it only when absent,
  rejects symlinks or pre-existing paths, records its inode and mode, and holds
  the owner lease through test-process retirement. Let the fixture remove its
  UUID child; remove the base only after process-group absence and matching
  inode ownership are verified. If either proof is unavailable, retain it and
  report the cleanup failure.

  Example targeted local invocation, run inside the bounded task runner that
  provides the exclusive short base. Set `SLOPSEARX_BUILD_RUN` to the registered
  absolute run directory under the persistent build root first:

  ```sh
  : "${SLOPSEARX_BUILD_RUN:?Set the registered absolute run directory}"
  SLOPSEARX_TEST_TEMP_ROOT=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TMPDIR=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TMP=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TEMP=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    cargo test --locked --target-dir "$SLOPSEARX_BUILD_RUN/target" \
      -p ssx-mcp --test t06_discovery
  SLOPSEARX_TEST_TEMP_ROOT=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TMPDIR=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TMP=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    TEMP=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp \
    cargo test --locked --target-dir "$SLOPSEARX_BUILD_RUN/target" \
      -p ssx-mcp --test t06_transport
  ```

- Record the owner, issue or PR, each absolute artifact path, artifact class,
  current bytes, and its retain/delete condition in the persistent task ledger
  under `<build-root>/storage-ledger/`. Mark shared or live paths with owner,
  size, reuse reason, and review/prune date. Mark evidence with its retention
  reason and whether it is canonical or unique.
- Record `df` availability and `du` usage for the run directory before work.
  Estimate expected growth from a comparable prior build or the inputs and
  toolchain planned for this run; state assumptions when the estimate is rough.
- Reuse an exclusive target or approved shared cache when source and toolchain
  match and no other process is using it. Record the owner, size, reuse reason,
  and review/prune date for shared caches. Avoid duplicate targets and
  concurrent writers. Do not clean or repurpose existing paths before
  checking their owner and references.
- After moving a checkout, verify the affected package's dep-info and any
  compile-time `CARGO_MANIFEST_DIR` or runtime fixture-root paths resolve under
  the current workspace. A cached test binary can still refer to the former
  absolute root even when the new checkout contains all fixture files. If stale
  roots are confirmed, invalidate/rebuild only the affected owned package
  artifact(s) in the same approved target while holding the normal exclusive source, target, and Cargo-cache leases.
  Measure target usage before and after, and retain the original failure and
  provenance. Preserve Cargo registry/download caches; do not use blanket
  `cargo clean`, make a second target, or restore/copy the checkout to its old
  absolute path.

## After each merged PR

Complete these steps as part of PR acceptance:

1. Inspect the task ledger and the PR's receipts/handoffs for every path the
   run created or reused. Check process and open-file use before cleanup.
2. Preserve the minimum evidence needed to understand and review the result:
   baseline SHA, patch, lockfile identity, toolchain, commands and exit codes,
   and unique raw outputs. Keep compact unique evidence under the persistent
   build root. A receipt path alone is not an indefinite hold: after reviewing
   references, record the artifact's relocation or retirement mapping.
3. Treat targets, caches, temporary directories, full source copies, and
   compiled test binaries as reproducible build artifacts that can ordinarily
   be removed after the PR merges. Retain them only for an explicit active
   investigation or release obligation, with an owner, reason, and review date.
   A shared cache may remain when its owner, size, reuse justification, and
   review/prune date are recorded. Remove confirmed duplicate or superseded
   task artifacts. Never use broad recursive cleanup against existing user or
   shared data. Archive an app-managed worktree with the Codex app tool; inspect
   branch and dirty state before considering removal of an ordinary worktree.
4. Measure the cleaned task directory with `du` and check `df` again. Record
   what was removed, logical bytes reduced, available-space change, paths kept
   and why, and any process/reference that prevented cleanup. Report logical
   and filesystem measurements separately; shared or asynchronously reclaimed
   blocks may make them differ.

If owner or live use is unclear, preserve the artifact until the reference
audit resolves it, then record the retirement or relocation. Do not delete
canonical source or unique raw evidence required by the record, or another
owner's work.

## Running the whole Rust suite locally

The socket guidance above covers targeted browser and HTTP fixtures. Running the
entire `xtask` suite needs the same temporary base, and three properties of it
each fail differently when violated:

```sh
# Existing, owned, absolute, non-symlink base, registered by the bounded runner.
# Keep it short and outside the checkout — see the socket note above.
export SLOPSEARX_TEST_TEMP_ROOT=/Volumes/tank01/magnus/rust-builds/slopsearx/tmp
export TMPDIR="$SLOPSEARX_TEST_TEMP_ROOT"
cargo test -p xtask
```

- **`TMPDIR` and `SLOPSEARX_TEST_TEMP_ROOT` must be set and must resolve to the
  same directory.** The O02 health-retirement fixture compares them and rejects a
  mismatch with `TMPDIR and SLOPSEARX_TEST_TEMP_ROOT must resolve to the same
  directory`; unset, the same fixture reports
  `SLOPSEARX_TEST_TEMP_ROOT must be configured`.
- **The base must sit outside the checkout.** The review-harness bootstrap test
  creates its trusted harness destination under the process temporary directory.
  Its destination validator rejects a parent inside the target source tree
  (`harness must be outside target source`).
- **The base must be short.** The socket fixtures build `sockaddr_un`-limited
  paths beneath it, as the note above explains.

CI configures temporary paths per step; local whole-suite runs need equivalent
setup. A recorded run at `fbf275ab` reported 170 passed and 6 failed without the
fixture-root configuration. With both variables naming a short external base,
it reported 171 passed and 5 failed. These counts describe that host and revision,
not an expected failure allowance.

### Investigating permission-mode control failures

Five custody tests still failed in that local macOS run with permission-mode
control errors; the corresponding Linux CI checks passed. Local probes requested
`04644` and `04755` but read back `0644` and `0755`, respectively. Neither result
retained `S_ISUID`: the evidence does not establish an execute-bit-dependent rule,
a universal macOS behavior, or a portable replacement mode.

If a negative control expects an altered mode to be rejected, first inspect the
mode actually retained by the host. An accepted-mode readback means the intended
negative input was not established; it is not evidence that the validator accepts
setuid files. The host/filesystem/security-policy cause and a portable fix remain
unresolved in [#1111](https://github.com/magnus919/SlopSearX/issues/1111). Do not
weaken custody checks or update sealed source records merely to obtain a pass.
