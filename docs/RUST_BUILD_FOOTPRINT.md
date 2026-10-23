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
