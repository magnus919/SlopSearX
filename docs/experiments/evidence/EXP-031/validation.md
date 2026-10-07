# Implementation validation and promotion blocker

Candidate local signed commit: f3e9faa7fd9b00a0ebefb4a83651adbfc9ce2c31.
The inert candidate.patch.json preserves source/tests/docs against baseline.
No implementation PR or push; required checks are not fully green.

- Relevant final slice: pytest --no-cov -q tests/test_filter_semantics.py
  tests/test_mcp_enforcement.py tests/test_mcp_tools.py tests/test_mcp_harness.py
  tests/test_formatter.py tests/test_server.py: 281 passed, four warnings.
  Includes unsupported, consuming, enforced and partially enforced guards.
- ruff check on changed Python files and pinned pre-commit ruff-format: passed.
- Installed-runtime mypy slopsearx/ engines/: 114 source files, no issues.
- graphify update . completed after source/test edits (AST only).
- Full pytest with coverage first ran sandboxed: 34 failed, 2138 passed,
  56 skipped, six errors; loopback/browser permission failures made that run
  unsuitable as a product verdict. Original exact-warning assertions also
  needed the intended new wording.
- Full suite with loopback/browser permissions: 2176 passed, 56 skipped,
  two failed, coverage 85.10% (required 80%). One failure was the old
  time_range-warning assertion, corrected and covered by the final slice.
  The other is tests/test_workflow_portal.py:543, expecting two occurrences
  of “Submit to confirm.” but seeing one. It also fails on unchanged baseline
  source in the documentation worktree: one failed, one warning.
- pre-commit run --all-files was run. Historical evidence whitespace and
  unrelated staged-search formatting were flagged/modified; those files were
  restored byte-for-byte from HEAD, preserving their evidence checksums.
  Its isolated mypy hook reported gateway.py:209 unused type-ignore and
  oauth.py:104 subclassing Any. Same two errors reproduce with unchanged
  baseline tools.py using pre-commit run mypy --files slopsearx/mcp/tools.py.
  The installed-runtime mypy passes, so the isolated dependency environment
  differs; the hook failure is still recorded, not waived as green.

No baseline browser/hook source/config was changed to force promotion. Resume
implementation delivery after those required checks can pass. The semantic
finding is supported on the fixed contract corpus; delivery remains blocked.

Raw logs and the unified patch that contained trailing space padding are stored
as base64 JSON containers to preserve their exact bytes without introducing
whitespace-hook failures. artifact-encoding.json maps original filenames to
containers and original checksums. Decode the data field with base64.b64decode
and verify original_sha256 before use; this is representation only, not a new
trial or a change to the observations.
