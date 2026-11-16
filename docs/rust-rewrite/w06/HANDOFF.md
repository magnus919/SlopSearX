# W06 cold-agent handoff

Start with the current AGENTS.md, CASE_STUDY.md, issue #912, the prerequisite
W11/W02/W04/F15 handoffs, then this directory's README/PARITY and the actual
implementation. Baseline for these immutable source captures is
`9b7d888da1ee3f7e2ed045c2185ea16f419381e9`. The shared W06 worktree preserves
that branch history; a new session must fetch and reconcile current upstream
before using it.

Independent review now accepts the composed corrected 14-file source map.
The accepted assessment is retained in
`evidence/independent-composed-source-assessment.json` (SHA256
`6284ee20d81831afdd2aa856eea54f96bba4207d885614c1e96dce4986560a2a`).
The final R04 review is in `evidence/independent-r04-final-review.json` (SHA256
`718375bac6877fcdb81b8d7201e4d3c045f56b7157d1530f7a4e13ba42da6c00`).
These receipts compose the original full review, public/saved-authority
corrections and exact-integer truthiness correction. They establish source
review acceptance for the listed hashes, not final combined gate or delivery
acceptance. Parent owns exact-head CI/review and final delivery acceptance.

Current local qualification includes A09 main `6c449764`: normal merge
`0b35624d` passed W06/W04/W11/W02/A05/A08, including all 303 W06 completed
cases. Reviewed A09 artifact pin correction `ba671ae2` then passed A09/F01/O06.
See `evidence/post-a09-qualification303.json` for exact report hashes, source
commits and check limits. No runtime cases or immutable source captures changed.
Published-head CI/review remains the parent's acceptance boundary.

Historical local qualification passed on post-A05 merge `54cabc79` (parents
`9de1abd3` and accepted main `93db215d`). W06 executed all 37 semantic tests
with zero ignored and verified 303 distinct case/test receipts. Fresh
F01/O06/W04/W11/W02/A05/A08 gates also passed. The parent-reviewed dependency
and module union updates only Cargo.lock, ssx-infra/Cargo.toml and
ssx-infra/src/lib.rs in the owned 14 map; the remaining 11 files are byte-exact,
including the W06 planning, runner and service implementations. Full lock
records preserve accepted dependency versions/checksums and both parents'
required edges. See `evidence/post-a05-qualification303.json` for identities.

Workspace strict Clippy, formatting, three gate unit checks, direct Ruff and
scoped hooks passed after that merge. The AST graph was refreshed. The PR
relative diff check passes; raw parent-merge diff whitespace warnings come
from accepted immutable A05 source assets and were not normalized. Local
checks do not establish fresh Linux ARM/AMD CI success; the earlier pre-A05
ARM A07 tiny-timeout failure remains a separate receipt and is not retried
or resolved by this packet.

`evidence/integrated-qualification303.json` remains the exact pre-A05 proof
on `4cc5858c`. The earlier 151 positives and mutation negatives also remain
immutable historical evidence; neither is relabeled as current 303 negative
qualification. Giant runtime reports stay outside Git.

The approved `pyproject.toml` exception suppresses only E501 for the frozen
exporter's long metadata string, preserving producer bytes. Direct Ruff and
scoped hooks pass. The original all-files hook run failed on that line and
an unrelated existing formatter wrapping; the latter was inspected and
restored exactly. Import-linter and mypy passed all-files, then had no matching
changed runtime Python in the scoped run. The graph was refreshed by AST after
main integration. No provider or production qualification is implied.

The immutable original source corpus has 85 cases. Separate retained-policy
and saved-source-authority packets add 13 envelopes each; the public-input
packet adds 121 executed parity rows and four source-only runner controls;
exact huge-number truthiness adds 16 rows. The current manifest requires 248
source and 55 native/rendering/hardening receipts, exactly once each, across
six application and 31 infrastructure semantic tests plus one process probe.
The helper's actual killed-owner lifecycle is asserted by its parent test.
The manifest pins all 14 reviewed runtime/test files. It excludes the four
reference controls from executed case bindings.

The language packet establishes all five reviewed nonstring-input differences
using actual original SearchService/StagedSearchRunner and permissive fixture
engines. All six manual producers verify pinned source hashes before backend
imports, including under `-O`, and have W06/G08 retirement ownership. Their
non-executable source copies, immutable outputs and actual capture receipts
remain under `evidence/`; executable supplements live under `/private/tmp`.
Native runtime and the mandatory gate never invoke a producer or Python.
`oracle-inventory.json` records all six manual producers. Parent owns their
global inventory registration.

| Artifact | SHA256 |
| --- | --- |
| source.json | 98116bc1010da4390d2b4e48775d6b3c8e1df63a41568a344013f4746c2fcbd3 |
| capture_source.py | e7426b72d8a70a1668a9e5a123f3e821ab34b81c926b8f5f824dbf6be40f2594 |
| regression-map.json | 1f8033728cef0aaf5b687f88c8ec414ed5e9bdb1133d91c9b5053972a0938af5 |
| language-input-source-outcomes.json | 0f4931fb02c53f706b56bdcd6c4d9d76e28b72da63ecad9c42980aa001e3ccd4 |
| language-source-producer.source.txt | 46cf840c30c1d36b0a69bcd20d71b96b693a54069cd721ef86f06ca59f179722 |
| retained-policy-source-outcomes.json | 108ecf64651cf95adcdaab200577e88cdc662e5ebaaabc1dfdf9e35758a9bd61 |
| retained-policy-source-producer.source.txt | 301fcf030f0a582923ca15635894bd637f21481b942318a3809987d267800e3c |
| saved-source-authority-outcomes.json | 7b3f1375ee478af9fbb7aaa8e975390ee5c5427e8beab976b41955a54922f079 |
| review-public-source-outcomes.json | 0d43f086c5c98a9bee7bd5339dc80d1ea7801147d7fa6e4be4c71263a5c86a1e |
| huge-integer-truthiness-source-outcomes.json | 4bce3df0415a2c49b3df348bf997462864af20efa984d5ca574650ee024c8487 |
| qualification.json (post-A05) | 18c907fcbe81cea06eef1cb94366059abae63d0baca791936c41449cb010c607 |
| evidence/qualification-component151.json (historical) | 474b79b5c4430b743baf465450840816ddd9eb9188a46b4bf36af3c880ab8a0d |

`crates/xtask/src/w06.rs` pins those bytes, checks the original source Git
blobs/SHA256 guards, enumerates the exact native test inventory, executes
every semantic test and verifies each case receipt with its expected test
identity. Every receipt follows assertions and owned fixture cleanup. The
subprocess probe has an explicit helper role; the actual process is started,
observed, killed and recovered by its parent semantic test. The gate also
runs the owned locked check, formatting and strict Clippy commands. Run with
the pinned toolchain/private Valkey and the tank01 build directory documented
in README; do not run concurrent Cargo builds into that target.

The regression map contains original signatures, parameters, decorators and
body hashes. Helper functions are not promoted into semantic evidence, and
downstream workflow-composition functions remain assigned to W08/W09. The
W11 corpus supplements the runner proofs; it does not replace real canonical
acquisition/fallback/recovery tests. Canonical plan bytes, full record fields,
literal clean-empty decisions and complete saved-source error envelopes are
compared directly. Fresh public admission binds one opaque operation ID and adds only the
separately accepted F09 pagination-enforcement entry to the source expectation.
Seeded replay records retain their exact shape. Original runner comparisons
remove the memory fallback's cleared owner token; durable W11 Lua retains its
acknowledged token. All other full record/envelope fields remain significant.

Preserve the rejected capture packets and capture-clock diff. Preserve the
first native cache qualification failure: fractional fixture timestamps were
rounded by Valkey cjson and changed the timeout-sensitive cache partition by
one millisecond. The corrected fixture uses an exactly representable integer
clock and retains the original cache assertions and production cache key.
The pre-retained-policy-delta gate is historical, not final acceptance for
the corrected retained-authority seam.

`evidence/native-gate-qualification.json` seals the two component positives,
all five actual gate mutation negatives and their full-log identities. The
post-restoration positive contains 151 actual receipts with 151 unique IDs,
103 executed commands and a separate summary; locked check, formatting and
strict Clippy passed. Actual ignored/missing/wrong-receipt commands themselves
returned zero while the gate rejected them. The original source/corpus/native
test bytes were restored exactly before the second positive. Full artifacts
remain at the tank01 paths recorded in that receipt. These facts describe the
historical component before the accepted corrections. Preserve those reports
and all five original mutation logs. They do not replace qualification of the
corrected source. Any final mutation rerun must use a unique new output
directory or suffix, never the old fixed negative paths.

The gate rejects changes to the frozen core source/test files before executing
native tests. A deliberate ignored/missing-test mutation will therefore hit
that source-identity fence unless the parent prepares a separately reviewed
mutation-only manifest/gate pin; that pin must never become the restored
positive qualification. Preserve both the mutation and exact restored source
identities. Never weaken the real gate to make a negative reach a later stage.

After a main-branch union, any changed shared owned-file identity (for example
Cargo.lock) requires explicit reviewed manifest reconciliation before the full
gate. Source capture identities remain historical and immutable. Graphify was
updated during implementation; the parent owns its final refresh after union.

W08 consumes saved-report-derived query/provenance and staged snapshots as
bounded evidence. W09 owns actual MCP schema/registry/prompts, live grant
composition and concrete O01 event/metric export. W10 owns protected portal
composition and browser acceptance. Recovery callbacks are acknowledgment
observations rather than a durable outbox. Dynamic authority has no atomic
policy fence with a SAVE already sent to Valkey; a definite commit can win
that race. Unknown external/write completion remains unknown and charged.
Snapshot orphaning is possible; stale reference publication is fenced.

This packet does not authorize providers, production state, deployment or
cutover, establish performance/relevance benefit, or close the final Rust-only
release obligations. Delivery/merge/CI identity belongs to the parent W06
acceptance record and must be verified on its exact final head.
At G08, retire the temporary executable producers and explicitly migrate
current-source guard verification to the approved historical-source identity
inventory. Do not silently skip these guards when removing backend Python.
