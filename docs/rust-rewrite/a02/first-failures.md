# First A02 failures (retained)

Initial `cargo check --locked -p ssx-infra` failed because scraper Html (non-Send) lived across detail await. Fixed by retaining owned matched names only before awaiting.

Initial test compilation found EngineStatus has no as_str and AdapterHttp::shutdown is synchronous. Fixed test API usage.

First executable parity run: 2 tests, 1 passed, 1 failed, zero ignored. `pypi_exact` timed out with an overly short 30 ms loopback test budget (Python oracle expected ok). Changed ordinary exchanges to 2000 ms and explicit timeout fixtures to 100 ms with a 500 ms server stall. No corpus expectation or source identity changed.

Second executable parity run caught source semantic discrepancy: Docker Hub results={} yields ok/empty because sorted(enumerate({})) is empty, whereas native rejected non-array. Fixed native to preserve empty mapping/string iterable behavior, not changed fixture expectations.

Optional oracle pre-commit initially found import/style errors; corrected without changing captured bytes and reproduced both corpus digests exactly. Full prescribed pre-commit then had only the pre-existing Ruff-format change in tests/test_coverage_saved_method_materials.py; that incidental diff was restored, and every hook passed for all other tracked files.

First `cargo xtask check A02` passed: five integration tests exercising all 135 unique observed cases plus admission/cancellation/timeout semantics, one default-route unit test and four registry completeness tests, zero ignored. Later final qualification is frozen against the staged source and records native source identities.

Independent review retained four source-backed defects: PyPI HTML byte decoding,
whitespace/comment-only document error classification, Docker Python-only strip
characters, and explicit-empty PyPI/RubyGems base URL fallback. Separate 19-case
raw-byte corpus and config observations were captured before native corrections;
the initial 135 observations remain byte-exact. Decoder dependency adds only an
explicit edge to existing locked encoding_rs 0.8.42, with verified checksum and
Apache/MIT/BSD license metadata. Invalid explicit URLs fail before caller-pool
access; a closed-pool negative test prevents any accidental public request.

First frozen F03 gate failed at F01 source inventory before executing build/tests:
`source fixture coverage is incomplete or unexpected`. Two optional A02 oracle
exporters were present in the local retirement inventory but not global F01.
The narrow correction adds exactly two A02/G08 source/invocation records and a
drift note, updating only their new exporter and changed inventory-document
blob pins. Existing Python/runtime/historical pins remain unchanged. The failed
`target/rust-evidence/F03-a02.json` is retained separately before requalification.

Final transport audit caught fresh per-detail sessions losing fallback cookies.
A separately source-captured cookie case proves direct-session isolation and
index->detail->detail cookie updates. The approved F08 Session::set_deadline
changes only the next operation deadline; native tests prove request budget,
cancellation and shutdown guards survive it. Existing 135 corpus cases remain
unchanged; review corpus now has 20 cases for 155 total.

Parent/independent review then confirmed three actual native failures in common
meta declarations (raw red output retained as meta-first-failure.txt). Generic
meta content/name attributes do not declare an encoding; content charset needs
http-equiv=Content-Type, and an unknown declaration must not suppress a later
recognized one. Separate three-case source corpus was captured before this
finite correction. Initial 155 observations remain unchanged; total is 158.
The coherent prior freeze passed A02/F08/F03/A01/F06/O06 before correction; those
reports are retained and final qualification runs against the new freeze.
