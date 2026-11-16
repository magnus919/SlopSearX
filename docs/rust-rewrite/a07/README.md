# A07 native biomedical adapters

The historical implementation baseline is
`1e9dd82d62456fe6f511cc5ade88bae619fac027`, freshly fetched and verified before
work. A01's prerequisite is merged PR #969 (`025ab2f2d50febcb573ba62579eecf7e2d07ef3a`)
with its accepted handoff and ancestry. The A07 checkpoint is now normally merged
with `1bad48bbe4e25e2f60d5b86668e92bbce922fdf0`, retaining F15, W11 and W02.
Historical source pins remain historical; the gate verifies original native/CI
Git blobs separately from live original provider/config/test bytes.

ClinicalTrials, Europe PMC, PubMed, PubChem and UniProt are actual native
implementations with registered factories. They consume shared F08 transport,
SearchAdmission, F06 raw ConfigScalar settings and the accepted SearchContext.
The source-compatible settings bridge preserves absent/blank endpoint defaults
and nonblank override bytes. Actual registry tests verify all five factories,
source categories and specialist cards, science selection, denied admission and
Europe PMC's disabled-by-default policy. Factory availability does not grant
selection. Registration adds no engine-specific orchestrator dispatch; historical catalog
implementation flags remain unchanged. Shared caller-deadline propagation is
described below.

The immutable evidence contains 172 distinct actual source controls. These
preserve the original 91, 25 nonempty-scalar, six numeric and five digit controls,
plus 45 reviewed shape/date controls. Native replay uses actual parsers and
compares ordered methods, URLs, parameters, request counts, adapter-owned headers,
status and complete response/result fields, including actual media. Only measured
latency is excluded as nondeterminism. The 31-case diagnostic ledger individually
records reviewed differences: native/library exception wording and UniProt's
pre-request TypeError supersession under the adapters-never-raise invariant.
Source-authored messages and upstream 429 versus local-denial diagnostics remain
separate. Python-library User-Agent identity and transport-generated headers are
explicit request-comparison exclusions, not claims of byte-identical HTTPX output.

Source iterable behavior, raw scalar configuration, PubMed missing-summary
positions/all authors, and stable UniProt promotion/species ties are covered.
Europe PMC remains opt-in and lite-only, retains provider URLs, attribution and
preprint notices, and does not claim date/paging enforcement or fetch abstracts,
full text or cursors. Actual adapter-to-rank-to-compact tests preserve attribution,
PPR notices and the 300-character snippet. Their PubMed-first URLs differ; no
same-URL attribution replacement is claimed.

PubMed's aggregate deadline covers both requests, decoding and normalization.
Other direct adapters retain per-request network timers and cooperative bounded
normalization without an invented aggregate timer. Caller context bounds all
five through admission, requests and normalization. One 32 MiB invocation budget
charges first-phase scratch, payload projection and ProducedBatch ownership.
Quota failure is terminal Error with no partial successful output. Shared Python
repr/digit/casefold behavior is reused without a Unicode/JSON library fork.

The quota-success fixture uses a 60-second test timeout, giving PubMed its unchanged
90% (54-second) aggregate budget. It verifies quota and transfer behavior; it is not
default-budget throughput or performance evidence. Production defaults, the aggregate deadline
calculation and deadline controls are unchanged.
Provider-sized CID, digit-scan and stable-rank phases yield cooperatively; actual
pending-phase cancellation/deadline tests and removed-checkpoint mutations prove
cleanup. See BOUNDED_RUNTIME.md for allocation measurements and exclusions.

Parent review accepted the shared payload/batch, repr/scalar/casefold,
factory/gate/metadata, runtime and main-preservation components. The integrated
F03 workspace gate passed on `bff188e33b43c2d4e82bc61fa3dfea48c1d7b56c`: source and
inventory checks, formatting, strict Clippy, workspace tests/build and native
readiness. The integrated A07 suite includes 71 infrastructure units, 20 biomedical
tests, seven context tests, seven factory tests, two compact consumers and four
gate rejection tests. Exact frozen reports govern their source/log identities.
Final scoped documentation-head reports and exact-head PR/CI are required delivery evidence.

CI runs the real A07 gate and retains its source-bound report. The normal main
union preserves 399 tracked Python source entries and 40 historical oracle
invocations. Twelve A07 producers are explicitly inventoried for G08 retirement;
the native A07 runtime/gate neither invokes nor requires them. Captures and their
source provenance remain immutable. G02 global/query-input/DOM/returned-response
accounting and T03/G01/T06 HTML/full-MCP integration remain genuine downstream
boundaries; see PORTAL_IMPACT.md and REMAINING_A07.md. No live provider calls,
production rollout or complete-milestone acceptance is claimed.


The PubChem independent request-budget check combines the preserved integration
case with a mandatory private unit using the real HTTP pool and one manually
advanced owner clock. The integration case keeps its 500 ms configuration,
two-stage success, two results, two requests, and unchanged second-stage 404
error case. The unit holds actual loopback responses at owner times 0, 300,
500 and 600 ms: each request spends 300 ms, the second remains pending at the
first deadline of 500 ms, and both succeed after 600 ms overall. Production
clocks, APIs and default budgets are unchanged. All 19 integration identities
and 17 prior resource controls remain mandatory alongside the new unit. The
original wall-delay fixture remains archived byte-for-byte in JSON. The
qualification below records execution; source artifacts retain their original
point-in-time proposal status.

### PubChem owner-clock qualification (22 October 2026)

The whole A07 gate passed in the candidate working tree based on
`652315253a933f7222acb94f480284991c2125f7` plus seven uncommitted candidate paths,
with 302 passing tests and controls
in aggregate: 5 + 263 + 19 + 6 + 7 + 2. This is the whole-gate count; the six
controller commands include overlapping controls. The retained A07 report has
SHA-256 `6fc0aa9d0af85880c3597ce2238d2944c5e7b9a25b79646999dce9535d0a3c5c`.
The positive run used origin/main at the same revision (0/0 divergence). Its
completion fixture honors `SLOPSEARX_TEST_TEMP_ROOT` when configured and
otherwise uses the platform temporary directory; the run used this test-only
override for fixture placement.

The independently reviewed negative control removed the second PubChem
request's deadline renewal. At the same base commit and candidate working tree, origin/main had advanced
to `8aecf43ae8b2c25946f92c688f8a4ae299c53af5` through docs-only PR #1039
(0/2 divergence). The exact owner-clock test failed at the expected boundary;
the original source bytes were restored and verified. Its 904-byte stdout is
published at
[`qualification/pubchem-owner-clock-negative.stdout`](qualification/pubchem-owner-clock-negative.stdout)
(SHA-256 `032e93c436be3a08f8c2c35a25d59f004b84f3d05750d823cd25ac53f964e160`).

Portable reproduction uses Rust 1.99.0 and the six commands in
[`qualification/pubchem-owner-clock-qualification.json`](qualification/pubchem-owner-clock-qualification.json).
Choose an operator-owned output directory for the A07 report and fixture temp
root. The recorded report hash identifies the as-run report. These results
qualify this A07 scope only; they do not establish provider behavior, full
rewrite completion, or production rollout. Exact-head delivery checks remain
required.

The historical PubChem source archive and owner-clock amendment remain
byte-for-byte preserved with their original classifications and runtime status.
The archive records the superseded wall-delay fixture as historical failed
source; the owner-clock proposal remains `UNAPPLIED_UNCOMPILED_UNRUN`. The newer
qualification record adds actual native test evidence without rewriting those
source artifacts.
