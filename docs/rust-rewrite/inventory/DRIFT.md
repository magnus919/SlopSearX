# Baseline drift and intentional differences

F01 inventory baseline: `20109f9083babd014971951e8c02565ae51c2326`.
Original planning baseline: `2a995f964757df77693747b232aa47738c0c58cc`.

## Observed changes

| Baseline interval | Observation | Parity impact |
| --- | --- | --- |
| Planning baseline → F01 baseline | Experiment report and rewrite planning documents merged; no changes in `slopsearx/` or `engines/` | No runtime capability changes; inventory uses the newer full repository revision |
| F01 baseline → `b7b79ab6` (observed while F01 was in review) | Three experiment Markdown documents changed in PR #949 | No source, dependency, workflow, executable or historical Python artifact changes; pinned inventory stays at its declared baseline |

| F01 merged `2e0026b4` → `6aa33a16` (observed during F02) | PR #951 changed 29 experiment Markdown/JSON registration and evidence files | No runtime, dependency, workflow or Python-source changes; architecture and pinned F01 source inventory remain applicable |

| `6aa33a16` → `65a55e2e` | PR #953 changed six experiment Markdown/JSON outcome files | No runtime, dependency, workflow or Python-source changes; F03 begins from F02 merge `562b6ce1` on top of this revision |

## PR #954 drift accepted during F03

Reviewed `562b6ce1` → upstream `73940f07` (PR #954), integrated through a
normal merge into F03. Historical F01 inventory and planning baseline SHAs
remain unchanged. The native F01 fixture pins reviewed current sources; it
checks freshness and does not establish completed native parity.

Two existing mandatory Python tooling modules and four existing test modules
changed, alongside experiment documentation and frozen JSON materials. No
Python files, dependencies, workflow commands or invocation sites were added
or removed. Existing disposition/invocation rows and plan owners remain valid;
no plan dependency or runtime capability changes are required.

| Changed source | Existing owner | Native parity obligation |
| --- | --- | --- |
| `scripts/coverage_band_companion.py` | O03/O08 | Accept exact corrected protocol/cohort/companion pins and v4 groups; reject cross-mixed pins and archival groups |
| `scripts/coverage_gate_calculation.py` | O08 | Accept exact corrected v4 protocol/cohort pins only with unchanged execution and quality/task-use contracts, frozen-pool stage modes and diagnostic-only useful count |
| `tests/test_coverage_band_companion.py` | O05 | Preserve both-stage acceptance, mismatched-pin rejection and group-inventory rejection |
| `tests/test_coverage_gate_calculation.py` | O05 | Preserve both-stage acceptance, unchanged gate/execution identities, mixed/unknown-pin and changed-quality rejection, and hard navigation failure |
| `tests/test_coverage_stage_orchestration.py` | O05 | Exactly representable monotonic test origin avoids rounding at the eight-hour boundary; production deadline semantics remain unchanged |
| `tests/test_coverage_task_use_materials.py` | O05 | Verify only two development queries changed, confirmation and quality contracts remain fixed, and material/candidate source hashes match |

Corrected material hashes: protocol
`cfac29e7d406dc4c7e45030fafeb6ba466923cfeb19b34e427d344507c9d9ba9`, cohorts
`ad731b5d99907242dbc7c6be0c671049e179df76d50a2148886e7e34b6b28b30`, companion
`5b802406687af0a3c1cd318a4f5dfc6250f2643e81547d2739f7521e81740ed2`.
O08 must preserve the exact accepted/rejected cases in successor verification;
O05 carries the new regressions before O09 switches mandatory gates. This
expands parity obligations without authorizing an experiment or changing gates.

## Drift procedure

1. Fetch upstream and follow AGENTS.md session synchronization rules.
2. Compare the last accepted inventory baseline against the new source revision
   with `git diff --name-status OLD..NEW` and inspect changed implementations,
   public declarations, tests, scripts and executable workflow commands.
3. Add or revise capability, Python-disposition and invocation rows; assign a
   plan owner to every addition. Record removed or changed contracts explicitly.
4. Review omitted/null/empty/error semantics and durable bytes/clock behavior.
   Do not normalize a newly observed difference into the replay oracle silently.
5. Update this ledger with the interval, source changes, decisions and affected
   issues. Modify issue scope and dependencies together when needed.
6. Validate exact source-set coverage, owner IDs, source/test references and
   dependency acyclicity before accepting the new baseline. A missing capability
   or unclassified executable blocks acceptance.

F03/F04 will carry this inventory into the native check and replay commands.
Until then, acceptance requires source-set comparison and recorded review;
there is no claim that native parity has been implemented.

## Intentional differences

None implemented or accepted by F01. Proposed hardening in the case study must
receive an explicit ADR, affected contract updates and evidence before it is
counted as an accepted parity difference. Migration of an internal cache
namespace must not implicitly change durable artifact identities.

## F04 temporary developer oracle addition

F04 adds `docs/rust-rewrite/replay/oracle/export.py` against accepted F03
revision `7b2d876307189d7a62000d1f289b3ef4b65eaff6`. This imports the pinned
Python backend to produce reproducible adapter, HTTP and MCP observations
using recorded responses and local fake engines. It is backend-coupled
developer tooling, not a client-only exception. F04 owns its source evidence;
G08 must retire the executable before final backend removal. Its invocation
is recorded separately from mandatory CI/container commands in
`python-invocations.json`. Native replay consumes JSON and never invokes it.

The source manifest grows from 363 to 364 Python paths, with backend-coupled
tooling/test paths growing from 240 to 241. Historical baseline identities
remain unchanged; this ledger records the reviewed post-baseline addition.
The three adapter and two surface observations prove replay infrastructure
coverage only; native implementation parity remains assigned to successor
issues. No live provider capture or experiment is authorized by this addition.

## F05 temporary pure-function oracle addition

F05 adds `docs/rust-rewrite/f05/oracle/export.py` against accepted F04
revision `16f03be656f34d90edd8285f9c81dfb1c4626b0a`. The exporter observes
pinned Python storage codecs, HTTP/MCP projections and canonical identities
using local inputs. It is backend-coupled developer tooling; it does not
qualify for the client-only exception. F05 owns capture provenance and G08
owns executable retirement. Native F05 checks consume portable JSON without
launching Python. No provider calls or deployment are authorized.

This branch adds one path to the accepted 364-path manifest: 365 total,
including 242 backend-coupled tooling/test paths. Historical F01 baseline
reports remain unchanged. The reference environment uses Python 3.14.7 and
Unicode 16.0.0 to match the production image's Python version; the Unicode
identity cases include mappings absent from Python 3.12's Unicode 15 tables.

## F05 explicit native input differences

The F05 implementation introduces the following differences for review with
ADR 003; final acceptance requires the F05 negative tests and reviewed PR.
Pure JSON parsing rejects input larger than 4 MiB or nested more than 100
levels, and rejects floating-point tokens that cannot be represented as a
finite f64. Arbitrary-width integer digits remain lossless. These are explicit
native input bounds, not a claim that every legacy record fits. Payload and
storage codecs apply their narrower record limits, including source-compatible
omission where applicable. Persisted-data compatibility still requires the
later store and migration qualification; these parser tests do not prove it.

Native artifact-reference decoding requires integer JSON version `1` and
rejects boolean `true`. The Python equality check accepts `true`, although
its producers write integer `1`. Negative tests must cover this rejection
and each parser bound. Existing opaque identifiers remain unchanged; this
change migrates no durable identity namespace. Public numeric values retain
integer-versus-float representation; comparator tolerance must not conceal
changes in payload values, timing fields, or canonical identity bytes.

## W01 temporary record/accounting oracle addition

W01 adds `docs/rust-rewrite/w01/oracle/export.py` against accepted main
`cfd7a0af61a22cfd040a7fcd41ad950f82005959` after F05/F07 acceptance. Its
Python3.14.7 source-bound capture uses synthetic research records, injected clock
and ID factories, and pure coverage/accounting functions. It makes no provider
or store requests. W01 owns these migration observations; G08 retires executable
Python before final backend removal. Native mandatory gates read captured JSON
and never execute the exporter. This is not a client-only exception.

The reviewed source manifest grows from 365 to 366 Python paths and from 242 to
243 backend-coupled tooling/test paths. The README capture command is classified
separately under developer oracle invocations; existing mandatory runtime/CI
commands and historical baseline reports are unchanged. The invocation summary
now reports the actual three existing F04/F05/W01 developer rows. F01 fixture
refresh is limited to the new oracle and these three edited inventory documents;
all prior runtime Python and historical baseline hashes remain unchanged.

The first W01 frozen F03 run correctly failed on the newly staged, unclassified
exporter. Its failed source-bound evidence is retained; this explicit inventory
addition closes that failure rather than weakening exact-set validation. W01's
native hardening differences are recorded separately in `../w01/parity.json`.

## F08 transport differences proposed for review

F08 starts at accepted F05 `7cc420f7ffb61127750e77bf6130e225925f9186`.
The [transport parity ledger](../f08/parity.md) and [ADR 006](../../adr/006-adapter-http-bounds.md)
record finite request/body/redirect bounds, unsupported encoding errors, stricter
redirect credential stripping and the global native idle-pool strategy. Native
source-bound socket tests qualify those cases; full adapter/provider and
platform/portal integration parity remains with the named downstream owners.
No Python source or production browser behavior changes in this slice.

## F11 temporary pure-algorithm oracle addition

F11 adds `docs/rust-rewrite/f11/oracle/export.py` against accepted F05 revision
`7cc420f7ffb61127750e77bf6130e225925f9186`. It captures only deterministic
ranking, bounded scholarly grouping and structural URL classification from
synthetic/public inputs. Native F11 CI replays committed JSON without Python,
DNS, providers or shared stores. The exporter is backend-coupled developer
tooling, owned by F11 for evidence and G08 for executable retirement; it is not
a client exception or mandatory native runtime/gate dependency.

The reviewed manifest grows from 365 to 366 Python paths and from 242 to 243
backend-coupled tooling/test paths. Historical F01 baseline identities and
validation reports remain unchanged. F01 source fixture updates cover this
new temporary file and these reviewed inventory documents only; existing
Python runtime source pins are preserved.

## Integrated W01, T01 and F10 source evidence

The W01 branch retains its research exporter and identity capture, accepted T01
retains the MCP exporter, and accepted F10 adds its quota/health exporter. These
are temporary developer-only source oracles. Native acceptance gates consume
captured JSON and do not execute the exporters or call providers; G08 owns their
retirement before backend removal. The exact inventory union contains 370 Python
source paths: 122 backend/runtime, 247 backend-coupled tooling/test and one
client-only path. Its developer-oracle invocation register contains seven rows.

F01 records the final merged inventory-document identities after the narrow
reviewed refresh and adds the accepted F10 exporter blob pin. Existing W01/T01/F10
exporter bytes, Python runtime pins and historical F01 baseline evidence remain
unchanged.

## T09 temporary browser identity oracle and native authority

T09 adds `docs/rust-rewrite/t09/capture.py` against accepted F06/F07 main
`1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`. It records 61 offline actual-source
observations with 11 exact source hashes; native gates replay the JSON and never
execute Python. Independent pinned-Python recapture matched every byte. T09 owns
the evidence and G08 executable retirement. The current union grows from 370 to
371 Python source paths and from 247 to 248 backend-coupled tooling/test paths;
the developer-oracle invocation register grows from seven to eight rows.
Existing runtime Python and prior oracle bytes/pins are unchanged.

T09 uses maintained AWS-LC signature/HMAC/random APIs from the already locked
TLS backend, server-authoritative Valkey time, exact-record session CAS and
explicit typed input bounds. Named source coercion/config/redirect corrections
and browser namespace drain/relogin consequences are recorded in
[the T09 parity ledger](../t09/PARITY.md). No G04/G07 cutover is authorized or
claimed by this module; legacy audit retention and durable workflow idempotency
remain required handoff obligations.

## F09 temporary source oracle

F09 adds one developer-only exporter for synthetic scope, policy, filter, date
and named-topic ordering observations. This is backend-coupled migration tooling,
not a client exception. Native F09 consumes pinned JSON and never executes Python.
G08 retires the exporter before backend removal. The exact inventory becomes
371 Python paths (122 runtime, 248 tooling/test, one client) and eight developer
oracle invocation rows. Existing runtime and baseline hashes are unchanged.

## T04 isolated OAuth reference

T04 adds `docs/rust-rewrite/t04/capture.py` as a temporary, offline, exact-source
migration oracle (T04/G08), not a runtime dependency or gate subprocess. It
captures pinned SDK registration normalization and source provider state locally;
no HTTP endpoint or provider is called. G08 removes it before backend retirement.
The exact Python source union has 371 paths: 122 backend/runtime, 248
backend-coupled tooling/test and one client-only. Existing runtime/oracle bytes
and all historical baseline identities remain unchanged.

## T04 and T09 accepted-main integration

The union retains both independent temporary oracles exactly once: 372 Python
source paths (122 backend/runtime, 249 backend-coupled tooling/test, one client-only)
and nine developer-oracle invocations. T09 now has 76 source observations: the
original 61 plus fifteen integer drain-setting cases. Historical section counts
above describe their original isolated additions; existing runtime/source and
prior oracle bytes remain unchanged. T04 is accepted at `403dfd3720e969fa829f426136bf0f05b0360c56`;
T09 acceptance and combined-source qualification remain separate.

## F09 and T04 integration

Both temporary oracles remain inventoried exactly once. The combined inventory
contains 372 Python sources: 122 backend/runtime, 249 backend-coupled tooling/test
and one client-only source. Historical isolated-addition counts above are retained.

## Accepted F09, T04 and T09 integration

The exact combined inventory has 373 Python source paths: 122 backend/runtime,
250 backend-coupled tooling/test and one client-only, with ten developer-oracle
invocations. F09, T04 and T09 temporary exporters remain inventoried once each;
G08 retains executable retirement ownership. Historical isolated/earlier union
counts above remain historical. All original runtime and source oracle bytes
are unchanged. F09 is accepted at `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`;
T09 remains in review and requires qualification against the combined source.

## A02 isolated package adapter observations

A02 adds two temporary developer-only exporters for independently source-bound
package registry and supplemental redirect/detail/query observations. They use
CPython 3.14.7 with httpx 0.28.1 and lxml 6.1.1 in the isolated migration lane;
mandatory native tests and gates consume fixed JSON and never invoke Python or
providers. A02 owns source comparison and G08 removes both executable exporters
before backend retirement. This is not a client/runtime exception.

The exact Python source union becomes 374 paths: 122 backend/runtime, 251
backend-coupled tooling/test and one client-only source. The developer oracle
invocation register becomes 11 rows. The F01 fixture adds the two exporter blobs
and refreshes only these three inventory document blobs; existing source pins
and all historical baseline evidence remain unchanged.

## Accepted T09 and A02 source inventory integration

The normal merge retains the accepted browser-identity oracle and both existing
W01 developer-oracle invocation rows, plus both A02 package-observation exporters.
The exact union is 375 Python source paths: 122 backend/runtime, 252
backend-coupled tooling/test and one client-only. The developer-oracle invocation
register contains 12 rows. Historical isolated-addition counts above are retained.
Only the three merged inventory-document F01 identities need documentary
refresh; no existing oracle or runtime source identity is changed.

## F12 optional routing reference

F12 adds `docs/rust-rewrite/f12/oracle/export.py`, a temporary offline source
oracle pinned to Python 3.14.7, MCP 2.3.0 and FastMCP 4.0.11. Synthetic local
transports capture exact Jev payloads, decisions, keyless/enhanced scopes and
cache identities. Native gates read immutable JSON; they never invoke Python.
F12/G08 own retirement before backend removal. At the isolated F12 commit
`e1f16cfa`, before the accepted T09 integration below, the inventory contained
373 Python sources: 122 backend/runtime, 250 backend-coupled tooling/test and one
client-only source, with ten developer-oracle invocation rows. These are historical
counts verified from that commit, not the final F12 integration totals of 374
paths and eleven invocations at `fc97fc6d`. Existing baseline source hashes and
historical evidence are unchanged.

## F12 integration with accepted T09

The combined inventory retains all accepted-main Python source rows and adds
exactly the F12 reference exporter: 374 source paths (122 backend/runtime,
251 backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. All prior source records and oracle bytes remain unchanged.
T09 is accepted at `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`; F12 remains
in review. Earlier counts and statuses above describe historical checkpoints.

## A02 integration with accepted T09 and F12

The normal integration preserves all reviewed A02 and accepted T09 records,
plus the accepted F12 routing oracle, including both W01 invocation records. The exact union is
376 Python source paths: 122 backend/runtime, 253 backend-coupled tooling/test
and one client-only source, with 13 developer-oracle invocation rows. Earlier
counts and in-review labels above describe historical checkpoints. Only the
three merged inventory-document F01 anchors need documentary refresh; runtime
and oracle source/capture identities remain unchanged.

## T05 OAuth wire migration oracle

T05 adds exactly one temporary backend-coupled source exporter and one developer
invocation entry. It captures actual pinned SDK loopback HTTP requests with a
synthetic authority, never provider calls. Native runtime/gates consume the JSON
fixture only. G08 owns exporter retirement. Existing runtime/source inventory pins
remain unchanged; F01 adds this exporter and refreshes only reviewed inventory
bookkeeping documents. T05's native all-network auth, resource binding and bounded
form refusal differences are explicit in [the parity ledger](../t05/PARITY.md).

The developer-invocation summary repairs a stale count7: eight existing rows are
unchanged and the single T05 addition produces nine actual rows.

## T05 integration with accepted F09/A01

The combined inventory retains all 372 accepted-main Python sources unchanged and
adds the one T05 migration exporter: 373 sources (122 backend/runtime, 250
backend-coupled tooling/test, one client-only). All nine accepted-main developer
oracle invocation rows remain unchanged; T05 adds the tenth. Historical counts
above describe their isolated checkpoints rather than this combined source tree.

## T05 integration with accepted T09

The combined inventory preserves all 373 accepted-main Python sources and adds
only T05: 374 sources (122 backend/runtime, 251 backend-coupled tooling/test, one
client-only). All ten accepted-main developer invocation rows remain unchanged,
including both distinct W01 commands; T05 adds the eleventh. Historical counts
above describe their isolated checkpoints, not this combined source tree.

## T05 integration with accepted F12

The combined inventory retains all 374 accepted-main source rows and adds only
T05: 375 sources (122 backend/runtime, 252 backend-coupled tooling/test, one
client-only), and twelve complete developer-oracle invocation rows. Both distinct
W01 commands remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; earlier counts and statuses describe
historical checkpoints. Runtime and reference-oracle bytes remain unchanged.

## A02 integration with accepted T05

The normal integration preserves all reviewed A02 and accepted T05, T09 and
F12 source records, including both distinct W01 invocation rows. The exact
full-record union contains 377 Python source paths: 122 backend/runtime, 254
backend-coupled tooling/test and one client-only source, with 14 developer-oracle
invocation rows. Earlier counts and statuses describe historical checkpoints.
Only the three merged inventory-document F01 anchors require documentary
refresh; accepted runtime, oracle and source capture identities remain unchanged.

## F13 temporary source-capture inventory

F13 adds one optional backend-coupled exporter,
`docs/rust-rewrite/f13/capture.py`, with two documented invocations (forward
source capture and reverse-read of actual native-created snapshot bytes). Native
F13 gates invoke no Python. G08 retires this executable after the rollback window;
immutable source observations remain. All original disposition/invocation rows
are unchanged. This branch now inventories 378 Python sources: 122 runtime,
255 backend-coupled tooling/test and one client-only source, plus 16 temporary
developer-oracle invocation rows. The normal integration of accepted F12/T05/A02 main retains all complete records
from both parents, including both existing W01 invocation rows. No runtime
Python or source-oracle identity is refreshed by this documentary union.

## A03 community adapters

A03 adds exactly one temporary developer-only exporter,
`docs/rust-rewrite/a03/capture.py`, owned by A03 with executable retirement under
G08. Its pinned Python/HTTPX observations use injected offline transport only;
mandatory native gates consume committed JSON without invoking Python. All
373 existing source rows and ten developer invocation rows remain unchanged.
The combined counts are 374 Python sources (122 backend/runtime, 251
backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. No runtime source or accepted oracle pins are refreshed.



## A03 integration with accepted F12

This combined checkpoint retains every complete source/invocation record from
A03 and accepted F12: 375 Python sources (122 backend/runtime, 252
backend-coupled tooling/test, one client-only) and twelve developer-oracle
invocations. Both W01 invocation rows remain distinct and unchanged. Source
exporter/runtime bytes remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; T09 is accepted at
`35d97ccf8c9b2d8246373b2024cc8d7a457705a3`. Earlier counts/statuses describe
historical checkpoints; A03 acceptance remains pending.


## A03 integration with accepted T05

This checkpoint preserves every complete source/invocation record from both
parents: 376 Python sources (122 backend/runtime, 253 backend-coupled
tooling/test, one client-only) and thirteen developer-oracle invocation rows,
including both distinct W01 commands. All runtime and reference-oracle bytes
remain unchanged. T05 is accepted at
`66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`; F12 and T09 remain accepted as
recorded above. Earlier counts/statuses remain historical; A03 acceptance is
pending.

## A03 integration after A02 acceptance

The combined source manifest retains all complete A02 and A03 oracle records: 378 Python source paths (122 runtime, 255 backend-coupled tooling/test, one client) and 15 developer oracle invocation records. Earlier counts above describe their historical checkpoints. No executable Python is granted a new runtime exception.

## F13 integration after A03 acceptance

All complete parent records are preserved: 379 source paths and 17 developer-oracle invocations. A03 was accepted in PR #975 at `5c1b48b9e625501ab61828299168101ac00301b3`. Earlier counts describe historical checkpoints. The F13 raw snapshot EOF exception remains narrowly bound in the native hygiene ledger.

## F14 execution and PR874 publication-date amendment

F14 adds exactly two temporary developer-only source capture programs and two invocation rows. All 379 prior source records and all 17 prior invocation records, including both distinct W01 commands, remain unchanged; the resulting exact source set is 381 (122 runtime, 258 backend-coupled tooling/test, one client), with 19 developer-oracle invocations. Neither exporter is a runtime exception or mandatory gate dependency; G08 retires executable Python while retaining immutable JSON observations.

The historical 730c corpus preserves all initial 46 objects and 24 source bindings, adding only two availability observations. A separate independently recaptured d207 corpus contains 14 date admission and eight cache mutation observations across 11 source bindings. PR874 changes eight current Python anchors (formatter, MCP tools, HTTP server, service, and their four regression modules); historical captures retain original baseline identities. The native checker admits only the exact reviewed old/new Git blobs and new source SHA bindings. This source-freshness bridge does not assert unchanged behavior: bounded requests now require explicit audited OpenAlex and exact date evidence, with the corrupt self-consistent cache-scope case explicitly hardened against current authority. F09's 15 named native expectations remain unchanged. T02/T06/portal successors own proof envelope/availability mapping and surface contract qualification.

## W00 temporary saved-definition oracle

W00 adds one offline developer exporter at
`docs/rust-rewrite/w00/capture_source.py`, owned by W00/G08, against accepted
main `66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`. It captures actual saved records,
strict validation, configured-auth fingerprints, control revisions, source claim
precision and arithmetic boundaries with private Valkey and no provider calls.
The native W00 acceptance gate executes Rust tests only and consumes immutable
reviewed JSON; G08 retires this Python exporter before backend removal.

The existing 375 source rows and 12 developer invocation rows remain unchanged; the
new counts are 376 and 13. Both distinct W01 invocation rows are preserved. No
existing runtime, baseline source or legacy exporter identity is refreshed.
Source claim identity uses actual source rehydration after cjson: a retained early
failed capture hashed raw integer 1000 instead of the runner's float 1000.0. The
corrected observation is independently reproducible and the old failure remains
evidence, rather than a changed native hash rule. The [superseded capture and
boundary failures](../w00/evidence/README.md) are committed historical evidence;
current acceptance consumes only `w00/source.json`.

## W00 integration with accepted A02/A03/F13 and bounded-date main

The normal merge into accepted main `544c830724a322d05521dac7f902dbe93142c4a1`
preserves all complete parent source and developer-invocation records: 380 Python
sources (122 runtime, 257 backend-coupled tooling/test, one client-only) and 18
developer-oracle invocations. Both W01 invocation rows remain distinct. Earlier
counts/statuses describe historical checkpoints. Reviewed W00 runtime and oracle
bytes are preserved during conflict resolution; changes to current-source oracle
provenance require the explicit audit and review below rather than a blind pin
refresh. Accepted upstream runtime/exporter anchors retain their exact values.

PR #874 changes MCP tools only in ordinary search date-filter metadata and the
science tool's explicit OpenAlex requirement for absolute dates. No saved helper,
validation, policy fingerprint or saved control function changes. The accompanying
service/server/formatter changes add bounded-date validation, canonical response
proof and HTTP/portal rendering; these are outside W00 definition/claim behavior.
W00's whole-file MCP pin therefore needs a reviewed source-provenance mapping,
backed by offline recapture of all 55 saved observations with no provider calls.

The historical W00 MCP blob remains
`061593ce45d3e2440e818703af6b50d6e6d99cf1`; the accepted current blob is
`d06ecd475b9b523addf79781d2762eddd92f4911`. The separate
[55-observation integration recapture](../w00/integration-source.json) has reviewed
SHA256 `c500069182b9975539910e29683531759e0df375e049e8daea5b8664aa10aad3`.
Every field equals the historical corpus except exactly the named MCP source-hash
field. The gate checks both pinned source generations and that exact equivalence;
it does not refresh historical outcomes or ignore a general metadata group.

## W00 dependent F14 integration preparation

The reviewed F14 commit `2427bf38e2241d1ba23637e233f97e7d0ee6d5ea`
is integrated while PR #978 awaits acceptance. This is dependent preparation,
not a claim that F14 or W00 has been accepted. The complete source-record union
contains 382 paths and 20 developer-oracle invocations. Historical source
observations and their original provenance remain unchanged. Both W01 rows,
W00's source integration mapping and F14's exact PR874 bridge are retained.
Final publication requires accepted upstream integration and qualification.

## A04 temporary source-capture oracles

A04 adds two temporary, offline-only Python 3.14.7 exporters under
`docs/rust-rewrite/a04/oracle/`: `export_cases.py` reproduces all 37 original
source captures and their per-record config/status/exception context, while
`export_matrix.py` captures supplementary Brave, Wikipedia, and Reddit edge
cases. Both use actual source adapter methods with `httpx.MockTransport`; no
provider, DNS, or paid calls occur. Mandatory native A04 gates consume only the
committed JSON packs and never execute either exporter. A04 owns the source
observations; G08 owns executable retirement before backend removal.

The current source-path manifest grows by two backend-coupled tooling files,
and the developer-oracle invocation appendix grows by two rows. Historical F01
baseline identities remain unchanged. F01 documentary source pins require the
normal reviewed refresh; no runtime Python behavior or public client exception
is added.

## A04 corrected capture inventory

The frozen A04 packet retains 37 original observations and their context, 104 supplementary source observations, and six tagged nonfinite observations documenting the accepted F05 finite-output boundary. The same two developer-only exporters now have three explicit invocation records. This checkpoint inventories 377 Python source paths and 15 developer-oracle invocations; historical example and py.txt counts remain unchanged. All earlier complete source and invocation records are preserved. Staged source paths are checked exactly before F01 documentary pins are refreshed.


## A04 fourth-review source semantics and explicit representations

The corrected packet preserves all 37 original source records/context, all 110
previous ordinary records, and all six tagged nonfinite observations. The
ordinary matrix now has 296 authentic source observations. A separate 14-record
representation fixture retains source boolean/overrange media and non-string
selftext outcomes alongside explicitly reviewed native whole-response refusals.
The existing two source exporters have four separate commands; all previous 15
full developer invocation rows are unchanged and the appendix now contains 16.
Python source paths remain exactly 377; historical example count 49 and all
historical py.txt counts/modes remain unchanged. This entry changes no accepted
runtime Python source pin or historical reference capture.


The final A04 fourth-review extension retains that 296/14 checkpoint as an exact
record prefix. It now inventories 333 ordinary source observations and 126
explicit representation observations, with source-error precedence controls.
The same fourth exporter command captures approved String/Option<String>
malformed-output refusals across all three adapters; the four commands and all
16 invocation rows remain unchanged. Source-path and historical counts remain
377 and 49 respectively. No runtime/reference source pin changes.


## A04 fifth-review source JSON boundary observations

The A04-owned offline exporter adds a separate `--source-json-output` invocation
and source_json.json evidence for byte encoding, Python numeric decoding,
Unicode scalar output boundaries, and bounded native JSON resources. Existing
37/context/333/126/6 captures and all historical/runtime source pins remain
unchanged. The existing exporter path is reused: projected Python source paths
remain377, developer oracle invocation rows become17, and historical example
reference count remains49. Source-success/native-refusal deltas and actual
Python decoder exceptions are explicit in A04/PARITY.md and the new corpus;
they do not establish arbitrary JSON or other-adapter parity. Public codec and
storage remain unchanged. This entry requires narrow own-document/exporter
anchor review; it does not authorize refreshing runtime source pins.

## A04 accepted-main integration preparation

The reviewed A04 checkpoint is integrated with accepted F14 main `576d43b03c0378ffe113a1386b1ec88eaa4323df`. All complete parent inventory records remain unchanged: the additive union has 383 Python source paths and 24 developer-oracle invocation rows, including both W01 commands. Historical A04 captures and source pins are immutable. Its shared filter-semantics observation now uses the accepted exact PR874 source-freshness bridge and separately bound 22-case date evidence; this does not change adapter runtime semantics. Parent integration review, exact-head CI and acceptance remain pending.

## A04 accepted W00 main integration

The fully qualified A04/F14 checkpoint `ca7c6209b4b5e67f5916297e632afbd93f09d758` is normally integrated with accepted W00 main `950071404c61eabc1201730362baccd01b3f2295`. The complete-record union is384 Python source paths and25 developer-oracle invocations. Both parent records and all raw captures remain exact. W00 adds saved modules, gate and evidence metadata; A04 runtime and its exact optional-subreddit cache/flight correction remain unchanged. This entry changes only combined documentary bindings; independent integration review and acceptance remain pending.

## A06 parser candidate tooling (pending package acceptance)

At preserved baseline 730c37a5, seven offline source exporters add eight distinct
developer invocations (initial and direct-regression modes are separate). One
additional temporary native resource/descriptor qualification driver is invoked
by Rust CI. The projected staged inventory is 387 Python source paths: 122 runtime,
264 backend-coupled tooling/test and one optional client, with 25 developer oracle
invocations and 64 executable invocation lines. Earlier counts remain historical.
All eight new files are A06/G08-owned; no Python backend/client exception is added.
The CI driver must gain a native successor before required Python gate retirement.
These counts describe this candidate's intended staged set, not a claimed passed
F01 gate before parent stages/reviews it. Existing runtime/source pins stay intact.

A06 normal merge of accepted main1e9dd82d preserves the complete union: 392 source paths and 33 developer oracle invocation records. Prior A06-only counts above describe the premerge checkpoint, not the integrated total.
## W11 staged-store source oracle

At baseline `1e9dd82d62456fe6f511cc5ade88bae619fac027`, add the isolated developer oracle `docs/rust-rewrite/w11/oracle/export.py` and its documented invocation. It executes source AST functions and staged-store read/retry controls against private disposable Valkey only. Mandatory W11 checks consume immutable JSON and execute native Rust. G08 retires this executable Python before backend removal; the original source snapshot remains non-executable historical evidence. Existing inventory entries and runtime boundaries are unchanged.

## F15 bounded reranking and advisory source oracles

The F15 source-bound gate adds exactly two temporary developer-only Python exporters, `f15/capture.py` and `f15/capture_headers.py`, with two corresponding invocation rows. The complete384 source records and25 invocation rows at accepted A04 main remain unchanged, including both W01 calls; the additive union is386/27. The independently recaptured150-case corpus preserves its original124 cases, and the separate12-case streamed-header corpus records actual body-read counts. Source baseline `4dfc0f5691b6a5a1c039a2837372b1977422ec27` and all13 historical blobs are immutable. No provider calls or native Python runtime dependency are introduced. G08 retires executable capture tools while retaining immutable observations. F15 package review and maintainer acceptance remain pending.

## A06 accepted F15 main union (candidate integration)

Both complete parent row sets are preserved:394 inventory records and35 developer invocation records. Current checkpoint has449 Python source paths;55 new development-donor/oracle paths remain unregistered, explicitly exact_match=false. These are pending parent-owned integration proposals, not a passed F01 gate/current-source closure.

## A06 guarded classification closure (candidate integration)

Parent-reviewed proposal `15cc59e79df807053f718ec5ac1571c1284ef33b9e11904cbf82bc19240ab25f` appends all57 missing classifications while preserving the394 prior rows and every actual invocation row. After staging the two new A06 exporters, the exact tracked source set is451 paths:122 backend/runtime,324 backend-coupled tooling/test,one client-only andfour historical licensed source inputs. The53 manual developer-tool guidance entries are a separate appendix; they are not executed CI invocation evidence and do not change64 active invocation rows or35 existing developer-oracle invocation records. All captures remain immutable; no provider call or Python runtime/client exception is introduced.

G08 removes all runnable Python tools before final backend removal. The four licensed IDNA Python originals must also leave the final backend tree and backend/helper packages: preserve immutable licensed bytes, notices and pins only as clearly non-executable historical evidence or Git history. Merely labelling these files inert does not authorize shipping executable Python backend source. F01 source bindings are refreshed against the staged exact set; parent aggregate acceptance and later A07/main additive union remain required.

## A06 pinned hostname lowercase oracle addition

Parent accepted narrow component1978676d and its guarded additive inventory proposal. The single new temporary manual exporter `a06/oracle/export_hostname_lower.py` captures Python3.14.7 Unicode16 lowercase/FinalSigma/hostname percent-zone controls offline. Its executable code retires at G08; native tables and immutable JSON retain source identities. No provider calls, runtime fallback, mandatory Python gate or fabricated CI invocation is introduced. All451 prior source rows and actual invocation arrays remain unchanged; the staged exact set becomes452 paths (325 backend-coupled tooling/test), with54 manual guidance rows and the unchanged35 developer invocation rows. Main/A05 additive reconciliation remains required; these counts do not incorporate A05's separately pending pipeline exporter.
## W11 integration with accepted F15

The additive union preserves both original inventories and all historical source pins:387 Python source paths,264 backend-coupled tooling/test paths and28 developer oracle invocations. Both W11 and F15 native gates and retained artifacts remain enabled. This records integration preparation; merged-head validation remains required.


## A07 temporary source evidence inventory

A07 biomedical candidate adds12 exact-hash temporary offline source producers,396 projected Python source paths and37 developer-oracle invocation rows against this baseline. All existing records/historical captures unchanged; native A07 gate consumes nonexecutable JSON only and never requires or executes producers. G08 owns executable retirement before backend removal. This is additive inventory, not native runtime Python permission. Projected counts describe this candidate only; recompute after normal accepted-main integration before final acceptance. No complete existing record was replaced.


## A07 normal accepted-main integration

The DCO A07 checkpoint is normally merged with accepted main1bad48b (F15/W11/W02). All complete parent and A07 inventory records are retained:399 tracked Python source paths and40 temporary developer-oracle invocation rows. Existing source observations and source Git pins remain immutable. Historical native/CI baseline blobs are checked separately from current native implementations. Final integrated gates and exact-head CI remain required.


## A08 actual source migration evidence

A08 starts from fresh accepted-main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0`.
Four exact-hash temporary source producers have actual completed offline or
private-loopback invocation receipts in `a08/oracle-runs.json`. The candidate
union is403 tracked Python source paths and44 developer-oracle invocation rows.
Every existing parent record and historical capture is preserved. Native A08
runtime and mandatory gate read immutable JSON only and never execute Python.
G08 retires all four executable producers; source captures/provenance remain.
Initial exploratory/partial attempts are not these accepted capture receipts.

A08 regression-mapping correction against candidate `9e1b4453` retains the
original 47 passing parameter instances / 44 functions and 38 original adapter
invocations, and adds five whole-adapter replays of the exact observed Oyez
URL-helper assertion inputs. The existing inventoried producer captures those
inputs and performs all five offline adapter calls; its updated exact hash and
executed receipt remain bound here and in F01. No producer count or retirement
owner changes. Genuine registration-only disposition is restricted to the five
explicit original registration nodes.
## W04 saved-runner source oracle inventory correction

W04 adds the temporary offline synthetic `docs/rust-rewrite/w04/capture_source.py` producer. Its existing local oracle inventory did not update the global F01 source set; this correction adds its exact source binding, backend-coupled tooling/test classification, W04/G08 ownership and reproduction guidance. The guidance is not an execution receipt. Existing source/invocation records remain unchanged: the additive union is388 Python source paths,265 backend-coupled tooling/test paths and29 developer-oracle invocation rows. Native runtime and mandatory gates never execute the exporter; the W04 gate hash-reads it as a temporary source artifact. G08 retires executable Python before backend removal and retains immutable JSON observations/provenance.

## W04 integration with accepted A07

Normal integration of accepted A07 main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0` preserves every complete parent record, yielding400 tracked Python source paths,277 backend-coupled tooling/test paths and41 developer-oracle invocation rows. Both A07 and W04 native gates and artifact uploads remain enabled. Historical observations and source pins are unchanged. This records integration; final combined-source validation and review remain required.

## A06 normal accepted-main source union

The preserved A06 checkpoint9fc815c8 is normally integrated with accepted main9b7d888d. Complete source records and actual invocation rows from both parents remain unchanged:466 classified source paths,49 developer-oracle invocation rows and64 active interpreter invocation rows. The54 manual developer-tool guidance entries remain a separate appendix and are not execution evidence. Only the three current inventory-document F01 bindings are refreshed to merged bytes; historical source pins and observations remain immutable. G08 executable retirement, final merged-source qualification and parent review remain required.

## A08 union with accepted W04/main

The normal merge preserves the accepted A08 history through `f3ef8910` and
main/W04 `9b7d888d`. Both complete source inventories and developer-oracle
records are retained: 404 tracked Python sources and 45 developer-oracle
invocation rows. Existing source observations, captures and producer hashes
remain unchanged. The descriptive A08 regression-producer evidence SHA now
matches its already-enforced current hash. CI snapshot and evidence paths
retain both W04 and A08 gates. Main integration does not adopt pending A05/A06
shared seams; their future union remains a separate reviewed operation.

## A06 integration with accepted A08

Preserve both complete parent histories and all source/invocation records:470 source dispositions,53 actual developer-oracle rows and54 separate manual guidance entries. Active interpreter rows remain64 here; the previously reviewed A06 native-driver retirement is a separate guarded update. Only the three conflicted current inventory-document F01 hashes are repinned; historical pins remain immutable. Global/platform and issue984 crosssurface closure remain open.

## Accepted A05 main93db historical drift record

# Baseline drift and intentional differences

F01 inventory baseline: `20109f9083babd014971951e8c02565ae51c2326`.
Original planning baseline: `2a995f964757df77693747b232aa47738c0c58cc`.

## Observed changes

| Baseline interval | Observation | Parity impact |
| --- | --- | --- |
| Planning baseline → F01 baseline | Experiment report and rewrite planning documents merged; no changes in `slopsearx/` or `engines/` | No runtime capability changes; inventory uses the newer full repository revision |
| F01 baseline → `b7b79ab6` (observed while F01 was in review) | Three experiment Markdown documents changed in PR #949 | No source, dependency, workflow, executable or historical Python artifact changes; pinned inventory stays at its declared baseline |

| F01 merged `2e0026b4` → `6aa33a16` (observed during F02) | PR #951 changed 29 experiment Markdown/JSON registration and evidence files | No runtime, dependency, workflow or Python-source changes; architecture and pinned F01 source inventory remain applicable |

| `6aa33a16` → `65a55e2e` | PR #953 changed six experiment Markdown/JSON outcome files | No runtime, dependency, workflow or Python-source changes; F03 begins from F02 merge `562b6ce1` on top of this revision |

## PR #954 drift accepted during F03

Reviewed `562b6ce1` → upstream `73940f07` (PR #954), integrated through a
normal merge into F03. Historical F01 inventory and planning baseline SHAs
remain unchanged. The native F01 fixture pins reviewed current sources; it
checks freshness and does not establish completed native parity.

Two existing mandatory Python tooling modules and four existing test modules
changed, alongside experiment documentation and frozen JSON materials. No
Python files, dependencies, workflow commands or invocation sites were added
or removed. Existing disposition/invocation rows and plan owners remain valid;
no plan dependency or runtime capability changes are required.

| Changed source | Existing owner | Native parity obligation |
| --- | --- | --- |
| `scripts/coverage_band_companion.py` | O03/O08 | Accept exact corrected protocol/cohort/companion pins and v4 groups; reject cross-mixed pins and archival groups |
| `scripts/coverage_gate_calculation.py` | O08 | Accept exact corrected v4 protocol/cohort pins only with unchanged execution and quality/task-use contracts, frozen-pool stage modes and diagnostic-only useful count |
| `tests/test_coverage_band_companion.py` | O05 | Preserve both-stage acceptance, mismatched-pin rejection and group-inventory rejection |
| `tests/test_coverage_gate_calculation.py` | O05 | Preserve both-stage acceptance, unchanged gate/execution identities, mixed/unknown-pin and changed-quality rejection, and hard navigation failure |
| `tests/test_coverage_stage_orchestration.py` | O05 | Exactly representable monotonic test origin avoids rounding at the eight-hour boundary; production deadline semantics remain unchanged |
| `tests/test_coverage_task_use_materials.py` | O05 | Verify only two development queries changed, confirmation and quality contracts remain fixed, and material/candidate source hashes match |

Corrected material hashes: protocol
`cfac29e7d406dc4c7e45030fafeb6ba466923cfeb19b34e427d344507c9d9ba9`, cohorts
`ad731b5d99907242dbc7c6be0c671049e179df76d50a2148886e7e34b6b28b30`, companion
`5b802406687af0a3c1cd318a4f5dfc6250f2643e81547d2739f7521e81740ed2`.
O08 must preserve the exact accepted/rejected cases in successor verification;
O05 carries the new regressions before O09 switches mandatory gates. This
expands parity obligations without authorizing an experiment or changing gates.

## Drift procedure

1. Fetch upstream and follow AGENTS.md session synchronization rules.
2. Compare the last accepted inventory baseline against the new source revision
   with `git diff --name-status OLD..NEW` and inspect changed implementations,
   public declarations, tests, scripts and executable workflow commands.
3. Add or revise capability, Python-disposition and invocation rows; assign a
   plan owner to every addition. Record removed or changed contracts explicitly.
4. Review omitted/null/empty/error semantics and durable bytes/clock behavior.
   Do not normalize a newly observed difference into the replay oracle silently.
5. Update this ledger with the interval, source changes, decisions and affected
   issues. Modify issue scope and dependencies together when needed.
6. Validate exact source-set coverage, owner IDs, source/test references and
   dependency acyclicity before accepting the new baseline. A missing capability
   or unclassified executable blocks acceptance.

F03/F04 will carry this inventory into the native check and replay commands.
Until then, acceptance requires source-set comparison and recorded review;
there is no claim that native parity has been implemented.

## Intentional differences

None implemented or accepted by F01. Proposed hardening in the case study must
receive an explicit ADR, affected contract updates and evidence before it is
counted as an accepted parity difference. Migration of an internal cache
namespace must not implicitly change durable artifact identities.

## F04 temporary developer oracle addition

F04 adds `docs/rust-rewrite/replay/oracle/export.py` against accepted F03
revision `7b2d876307189d7a62000d1f289b3ef4b65eaff6`. This imports the pinned
Python backend to produce reproducible adapter, HTTP and MCP observations
using recorded responses and local fake engines. It is backend-coupled
developer tooling, not a client-only exception. F04 owns its source evidence;
G08 must retire the executable before final backend removal. Its invocation
is recorded separately from mandatory CI/container commands in
`python-invocations.json`. Native replay consumes JSON and never invokes it.

The source manifest grows from 363 to 364 Python paths, with backend-coupled
tooling/test paths growing from 240 to 241. Historical baseline identities
remain unchanged; this ledger records the reviewed post-baseline addition.
The three adapter and two surface observations prove replay infrastructure
coverage only; native implementation parity remains assigned to successor
issues. No live provider capture or experiment is authorized by this addition.

## F05 temporary pure-function oracle addition

F05 adds `docs/rust-rewrite/f05/oracle/export.py` against accepted F04
revision `16f03be656f34d90edd8285f9c81dfb1c4626b0a`. The exporter observes
pinned Python storage codecs, HTTP/MCP projections and canonical identities
using local inputs. It is backend-coupled developer tooling; it does not
qualify for the client-only exception. F05 owns capture provenance and G08
owns executable retirement. Native F05 checks consume portable JSON without
launching Python. No provider calls or deployment are authorized.

This branch adds one path to the accepted 364-path manifest: 365 total,
including 242 backend-coupled tooling/test paths. Historical F01 baseline
reports remain unchanged. The reference environment uses Python 3.14.7 and
Unicode 16.0.0 to match the production image's Python version; the Unicode
identity cases include mappings absent from Python 3.12's Unicode 15 tables.

## F05 explicit native input differences

The F05 implementation introduces the following differences for review with
ADR 003; final acceptance requires the F05 negative tests and reviewed PR.
Pure JSON parsing rejects input larger than 4 MiB or nested more than 100
levels, and rejects floating-point tokens that cannot be represented as a
finite f64. Arbitrary-width integer digits remain lossless. These are explicit
native input bounds, not a claim that every legacy record fits. Payload and
storage codecs apply their narrower record limits, including source-compatible
omission where applicable. Persisted-data compatibility still requires the
later store and migration qualification; these parser tests do not prove it.

Native artifact-reference decoding requires integer JSON version `1` and
rejects boolean `true`. The Python equality check accepts `true`, although
its producers write integer `1`. Negative tests must cover this rejection
and each parser bound. Existing opaque identifiers remain unchanged; this
change migrates no durable identity namespace. Public numeric values retain
integer-versus-float representation; comparator tolerance must not conceal
changes in payload values, timing fields, or canonical identity bytes.

## W01 temporary record/accounting oracle addition

W01 adds `docs/rust-rewrite/w01/oracle/export.py` against accepted main
`cfd7a0af61a22cfd040a7fcd41ad950f82005959` after F05/F07 acceptance. Its
Python3.14.7 source-bound capture uses synthetic research records, injected clock
and ID factories, and pure coverage/accounting functions. It makes no provider
or store requests. W01 owns these migration observations; G08 retires executable
Python before final backend removal. Native mandatory gates read captured JSON
and never execute the exporter. This is not a client-only exception.

The reviewed source manifest grows from 365 to 366 Python paths and from 242 to
243 backend-coupled tooling/test paths. The README capture command is classified
separately under developer oracle invocations; existing mandatory runtime/CI
commands and historical baseline reports are unchanged. The invocation summary
now reports the actual three existing F04/F05/W01 developer rows. F01 fixture
refresh is limited to the new oracle and these three edited inventory documents;
all prior runtime Python and historical baseline hashes remain unchanged.

The first W01 frozen F03 run correctly failed on the newly staged, unclassified
exporter. Its failed source-bound evidence is retained; this explicit inventory
addition closes that failure rather than weakening exact-set validation. W01's
native hardening differences are recorded separately in `../w01/parity.json`.

## F08 transport differences proposed for review

F08 starts at accepted F05 `7cc420f7ffb61127750e77bf6130e225925f9186`.
The [transport parity ledger](../f08/parity.md) and [ADR 006](../../adr/006-adapter-http-bounds.md)
record finite request/body/redirect bounds, unsupported encoding errors, stricter
redirect credential stripping and the global native idle-pool strategy. Native
source-bound socket tests qualify those cases; full adapter/provider and
platform/portal integration parity remains with the named downstream owners.
No Python source or production browser behavior changes in this slice.

## F11 temporary pure-algorithm oracle addition

F11 adds `docs/rust-rewrite/f11/oracle/export.py` against accepted F05 revision
`7cc420f7ffb61127750e77bf6130e225925f9186`. It captures only deterministic
ranking, bounded scholarly grouping and structural URL classification from
synthetic/public inputs. Native F11 CI replays committed JSON without Python,
DNS, providers or shared stores. The exporter is backend-coupled developer
tooling, owned by F11 for evidence and G08 for executable retirement; it is not
a client exception or mandatory native runtime/gate dependency.

The reviewed manifest grows from 365 to 366 Python paths and from 242 to 243
backend-coupled tooling/test paths. Historical F01 baseline identities and
validation reports remain unchanged. F01 source fixture updates cover this
new temporary file and these reviewed inventory documents only; existing
Python runtime source pins are preserved.

## Integrated W01, T01 and F10 source evidence

The W01 branch retains its research exporter and identity capture, accepted T01
retains the MCP exporter, and accepted F10 adds its quota/health exporter. These
are temporary developer-only source oracles. Native acceptance gates consume
captured JSON and do not execute the exporters or call providers; G08 owns their
retirement before backend removal. The exact inventory union contains 370 Python
source paths: 122 backend/runtime, 247 backend-coupled tooling/test and one
client-only path. Its developer-oracle invocation register contains seven rows.

F01 records the final merged inventory-document identities after the narrow
reviewed refresh and adds the accepted F10 exporter blob pin. Existing W01/T01/F10
exporter bytes, Python runtime pins and historical F01 baseline evidence remain
unchanged.

## T09 temporary browser identity oracle and native authority

T09 adds `docs/rust-rewrite/t09/capture.py` against accepted F06/F07 main
`1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`. It records 61 offline actual-source
observations with 11 exact source hashes; native gates replay the JSON and never
execute Python. Independent pinned-Python recapture matched every byte. T09 owns
the evidence and G08 executable retirement. The current union grows from 370 to
371 Python source paths and from 247 to 248 backend-coupled tooling/test paths;
the developer-oracle invocation register grows from seven to eight rows.
Existing runtime Python and prior oracle bytes/pins are unchanged.

T09 uses maintained AWS-LC signature/HMAC/random APIs from the already locked
TLS backend, server-authoritative Valkey time, exact-record session CAS and
explicit typed input bounds. Named source coercion/config/redirect corrections
and browser namespace drain/relogin consequences are recorded in
[the T09 parity ledger](../t09/PARITY.md). No G04/G07 cutover is authorized or
claimed by this module; legacy audit retention and durable workflow idempotency
remain required handoff obligations.

## F09 temporary source oracle

F09 adds one developer-only exporter for synthetic scope, policy, filter, date
and named-topic ordering observations. This is backend-coupled migration tooling,
not a client exception. Native F09 consumes pinned JSON and never executes Python.
G08 retires the exporter before backend removal. The exact inventory becomes
371 Python paths (122 runtime, 248 tooling/test, one client) and eight developer
oracle invocation rows. Existing runtime and baseline hashes are unchanged.

## T04 isolated OAuth reference

T04 adds `docs/rust-rewrite/t04/capture.py` as a temporary, offline, exact-source
migration oracle (T04/G08), not a runtime dependency or gate subprocess. It
captures pinned SDK registration normalization and source provider state locally;
no HTTP endpoint or provider is called. G08 removes it before backend retirement.
The exact Python source union has 371 paths: 122 backend/runtime, 248
backend-coupled tooling/test and one client-only. Existing runtime/oracle bytes
and all historical baseline identities remain unchanged.

## T04 and T09 accepted-main integration

The union retains both independent temporary oracles exactly once: 372 Python
source paths (122 backend/runtime, 249 backend-coupled tooling/test, one client-only)
and nine developer-oracle invocations. T09 now has 76 source observations: the
original 61 plus fifteen integer drain-setting cases. Historical section counts
above describe their original isolated additions; existing runtime/source and
prior oracle bytes remain unchanged. T04 is accepted at `403dfd3720e969fa829f426136bf0f05b0360c56`;
T09 acceptance and combined-source qualification remain separate.

## F09 and T04 integration

Both temporary oracles remain inventoried exactly once. The combined inventory
contains 372 Python sources: 122 backend/runtime, 249 backend-coupled tooling/test
and one client-only source. Historical isolated-addition counts above are retained.

## Accepted F09, T04 and T09 integration

The exact combined inventory has 373 Python source paths: 122 backend/runtime,
250 backend-coupled tooling/test and one client-only, with ten developer-oracle
invocations. F09, T04 and T09 temporary exporters remain inventoried once each;
G08 retains executable retirement ownership. Historical isolated/earlier union
counts above remain historical. All original runtime and source oracle bytes
are unchanged. F09 is accepted at `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`;
T09 remains in review and requires qualification against the combined source.

## A02 isolated package adapter observations

A02 adds two temporary developer-only exporters for independently source-bound
package registry and supplemental redirect/detail/query observations. They use
CPython 3.14.7 with httpx 0.28.1 and lxml 6.1.1 in the isolated migration lane;
mandatory native tests and gates consume fixed JSON and never invoke Python or
providers. A02 owns source comparison and G08 removes both executable exporters
before backend retirement. This is not a client/runtime exception.

The exact Python source union becomes 374 paths: 122 backend/runtime, 251
backend-coupled tooling/test and one client-only source. The developer oracle
invocation register becomes 11 rows. The F01 fixture adds the two exporter blobs
and refreshes only these three inventory document blobs; existing source pins
and all historical baseline evidence remain unchanged.

## Accepted T09 and A02 source inventory integration

The normal merge retains the accepted browser-identity oracle and both existing
W01 developer-oracle invocation rows, plus both A02 package-observation exporters.
The exact union is 375 Python source paths: 122 backend/runtime, 252
backend-coupled tooling/test and one client-only. The developer-oracle invocation
register contains 12 rows. Historical isolated-addition counts above are retained.
Only the three merged inventory-document F01 identities need documentary
refresh; no existing oracle or runtime source identity is changed.

## F12 optional routing reference

F12 adds `docs/rust-rewrite/f12/oracle/export.py`, a temporary offline source
oracle pinned to Python 3.14.7, MCP 2.3.0 and FastMCP 4.0.11. Synthetic local
transports capture exact Jev payloads, decisions, keyless/enhanced scopes and
cache identities. Native gates read immutable JSON; they never invoke Python.
F12/G08 own retirement before backend removal. At the isolated F12 commit
`e1f16cfa`, before the accepted T09 integration below, the inventory contained
373 Python sources: 122 backend/runtime, 250 backend-coupled tooling/test and one
client-only source, with ten developer-oracle invocation rows. These are historical
counts verified from that commit, not the final F12 integration totals of 374
paths and eleven invocations at `fc97fc6d`. Existing baseline source hashes and
historical evidence are unchanged.

## F12 integration with accepted T09

The combined inventory retains all accepted-main Python source rows and adds
exactly the F12 reference exporter: 374 source paths (122 backend/runtime,
251 backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. All prior source records and oracle bytes remain unchanged.
T09 is accepted at `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`; F12 remains
in review. Earlier counts and statuses above describe historical checkpoints.

## A02 integration with accepted T09 and F12

The normal integration preserves all reviewed A02 and accepted T09 records,
plus the accepted F12 routing oracle, including both W01 invocation records. The exact union is
376 Python source paths: 122 backend/runtime, 253 backend-coupled tooling/test
and one client-only source, with 13 developer-oracle invocation rows. Earlier
counts and in-review labels above describe historical checkpoints. Only the
three merged inventory-document F01 anchors need documentary refresh; runtime
and oracle source/capture identities remain unchanged.

## T05 OAuth wire migration oracle

T05 adds exactly one temporary backend-coupled source exporter and one developer
invocation entry. It captures actual pinned SDK loopback HTTP requests with a
synthetic authority, never provider calls. Native runtime/gates consume the JSON
fixture only. G08 owns exporter retirement. Existing runtime/source inventory pins
remain unchanged; F01 adds this exporter and refreshes only reviewed inventory
bookkeeping documents. T05's native all-network auth, resource binding and bounded
form refusal differences are explicit in [the parity ledger](../t05/PARITY.md).

The developer-invocation summary repairs a stale count7: eight existing rows are
unchanged and the single T05 addition produces nine actual rows.

## T05 integration with accepted F09/A01

The combined inventory retains all 372 accepted-main Python sources unchanged and
adds the one T05 migration exporter: 373 sources (122 backend/runtime, 250
backend-coupled tooling/test, one client-only). All nine accepted-main developer
oracle invocation rows remain unchanged; T05 adds the tenth. Historical counts
above describe their isolated checkpoints rather than this combined source tree.

## T05 integration with accepted T09

The combined inventory preserves all 373 accepted-main Python sources and adds
only T05: 374 sources (122 backend/runtime, 251 backend-coupled tooling/test, one
client-only). All ten accepted-main developer invocation rows remain unchanged,
including both distinct W01 commands; T05 adds the eleventh. Historical counts
above describe their isolated checkpoints, not this combined source tree.

## T05 integration with accepted F12

The combined inventory retains all 374 accepted-main source rows and adds only
T05: 375 sources (122 backend/runtime, 252 backend-coupled tooling/test, one
client-only), and twelve complete developer-oracle invocation rows. Both distinct
W01 commands remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; earlier counts and statuses describe
historical checkpoints. Runtime and reference-oracle bytes remain unchanged.

## A02 integration with accepted T05

The normal integration preserves all reviewed A02 and accepted T05, T09 and
F12 source records, including both distinct W01 invocation rows. The exact
full-record union contains 377 Python source paths: 122 backend/runtime, 254
backend-coupled tooling/test and one client-only source, with 14 developer-oracle
invocation rows. Earlier counts and statuses describe historical checkpoints.
Only the three merged inventory-document F01 anchors require documentary
refresh; accepted runtime, oracle and source capture identities remain unchanged.

## F13 temporary source-capture inventory

F13 adds one optional backend-coupled exporter,
`docs/rust-rewrite/f13/capture.py`, with two documented invocations (forward
source capture and reverse-read of actual native-created snapshot bytes). Native
F13 gates invoke no Python. G08 retires this executable after the rollback window;
immutable source observations remain. All original disposition/invocation rows
are unchanged. This branch now inventories 378 Python sources: 122 runtime,
255 backend-coupled tooling/test and one client-only source, plus 16 temporary
developer-oracle invocation rows. The normal integration of accepted F12/T05/A02 main retains all complete records
from both parents, including both existing W01 invocation rows. No runtime
Python or source-oracle identity is refreshed by this documentary union.

## A03 community adapters

A03 adds exactly one temporary developer-only exporter,
`docs/rust-rewrite/a03/capture.py`, owned by A03 with executable retirement under
G08. Its pinned Python/HTTPX observations use injected offline transport only;
mandatory native gates consume committed JSON without invoking Python. All
373 existing source rows and ten developer invocation rows remain unchanged.
The combined counts are 374 Python sources (122 backend/runtime, 251
backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. No runtime source or accepted oracle pins are refreshed.



## A03 integration with accepted F12

This combined checkpoint retains every complete source/invocation record from
A03 and accepted F12: 375 Python sources (122 backend/runtime, 252
backend-coupled tooling/test, one client-only) and twelve developer-oracle
invocations. Both W01 invocation rows remain distinct and unchanged. Source
exporter/runtime bytes remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; T09 is accepted at
`35d97ccf8c9b2d8246373b2024cc8d7a457705a3`. Earlier counts/statuses describe
historical checkpoints; A03 acceptance remains pending.


## A03 integration with accepted T05

This checkpoint preserves every complete source/invocation record from both
parents: 376 Python sources (122 backend/runtime, 253 backend-coupled
tooling/test, one client-only) and thirteen developer-oracle invocation rows,
including both distinct W01 commands. All runtime and reference-oracle bytes
remain unchanged. T05 is accepted at
`66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`; F12 and T09 remain accepted as
recorded above. Earlier counts/statuses remain historical; A03 acceptance is
pending.

## A03 integration after A02 acceptance

The combined source manifest retains all complete A02 and A03 oracle records: 378 Python source paths (122 runtime, 255 backend-coupled tooling/test, one client) and 15 developer oracle invocation records. Earlier counts above describe their historical checkpoints. No executable Python is granted a new runtime exception.

## F13 integration after A03 acceptance

All complete parent records are preserved: 379 source paths and 17 developer-oracle invocations. A03 was accepted in PR #975 at `5c1b48b9e625501ab61828299168101ac00301b3`. Earlier counts describe historical checkpoints. The F13 raw snapshot EOF exception remains narrowly bound in the native hygiene ledger.

## F14 execution and PR874 publication-date amendment

F14 adds exactly two temporary developer-only source capture programs and two invocation rows. All 379 prior source records and all 17 prior invocation records, including both distinct W01 commands, remain unchanged; the resulting exact source set is 381 (122 runtime, 258 backend-coupled tooling/test, one client), with 19 developer-oracle invocations. Neither exporter is a runtime exception or mandatory gate dependency; G08 retires executable Python while retaining immutable JSON observations.

The historical 730c corpus preserves all initial 46 objects and 24 source bindings, adding only two availability observations. A separate independently recaptured d207 corpus contains 14 date admission and eight cache mutation observations across 11 source bindings. PR874 changes eight current Python anchors (formatter, MCP tools, HTTP server, service, and their four regression modules); historical captures retain original baseline identities. The native checker admits only the exact reviewed old/new Git blobs and new source SHA bindings. This source-freshness bridge does not assert unchanged behavior: bounded requests now require explicit audited OpenAlex and exact date evidence, with the corrupt self-consistent cache-scope case explicitly hardened against current authority. F09's 15 named native expectations remain unchanged. T02/T06/portal successors own proof envelope/availability mapping and surface contract qualification.

## W00 temporary saved-definition oracle

W00 adds one offline developer exporter at
`docs/rust-rewrite/w00/capture_source.py`, owned by W00/G08, against accepted
main `66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`. It captures actual saved records,
strict validation, configured-auth fingerprints, control revisions, source claim
precision and arithmetic boundaries with private Valkey and no provider calls.
The native W00 acceptance gate executes Rust tests only and consumes immutable
reviewed JSON; G08 retires this Python exporter before backend removal.

The existing 375 source rows and 12 developer invocation rows remain unchanged; the
new counts are 376 and 13. Both distinct W01 invocation rows are preserved. No
existing runtime, baseline source or legacy exporter identity is refreshed.
Source claim identity uses actual source rehydration after cjson: a retained early
failed capture hashed raw integer 1000 instead of the runner's float 1000.0. The
corrected observation is independently reproducible and the old failure remains
evidence, rather than a changed native hash rule. The [superseded capture and
boundary failures](../w00/evidence/README.md) are committed historical evidence;
current acceptance consumes only `w00/source.json`.

## W00 integration with accepted A02/A03/F13 and bounded-date main

The normal merge into accepted main `544c830724a322d05521dac7f902dbe93142c4a1`
preserves all complete parent source and developer-invocation records: 380 Python
sources (122 runtime, 257 backend-coupled tooling/test, one client-only) and 18
developer-oracle invocations. Both W01 invocation rows remain distinct. Earlier
counts/statuses describe historical checkpoints. Reviewed W00 runtime and oracle
bytes are preserved during conflict resolution; changes to current-source oracle
provenance require the explicit audit and review below rather than a blind pin
refresh. Accepted upstream runtime/exporter anchors retain their exact values.

PR #874 changes MCP tools only in ordinary search date-filter metadata and the
science tool's explicit OpenAlex requirement for absolute dates. No saved helper,
validation, policy fingerprint or saved control function changes. The accompanying
service/server/formatter changes add bounded-date validation, canonical response
proof and HTTP/portal rendering; these are outside W00 definition/claim behavior.
W00's whole-file MCP pin therefore needs a reviewed source-provenance mapping,
backed by offline recapture of all 55 saved observations with no provider calls.

The historical W00 MCP blob remains
`061593ce45d3e2440e818703af6b50d6e6d99cf1`; the accepted current blob is
`d06ecd475b9b523addf79781d2762eddd92f4911`. The separate
[55-observation integration recapture](../w00/integration-source.json) has reviewed
SHA256 `c500069182b9975539910e29683531759e0df375e049e8daea5b8664aa10aad3`.
Every field equals the historical corpus except exactly the named MCP source-hash
field. The gate checks both pinned source generations and that exact equivalence;
it does not refresh historical outcomes or ignore a general metadata group.

## W00 dependent F14 integration preparation

The reviewed F14 commit `2427bf38e2241d1ba23637e233f97e7d0ee6d5ea`
is integrated while PR #978 awaits acceptance. This is dependent preparation,
not a claim that F14 or W00 has been accepted. The complete source-record union
contains 382 paths and 20 developer-oracle invocations. Historical source
observations and their original provenance remain unchanged. Both W01 rows,
W00's source integration mapping and F14's exact PR874 bridge are retained.
Final publication requires accepted upstream integration and qualification.

## A04 temporary source-capture oracles

A04 adds two temporary, offline-only Python 3.14.7 exporters under
`docs/rust-rewrite/a04/oracle/`: `export_cases.py` reproduces all 37 original
source captures and their per-record config/status/exception context, while
`export_matrix.py` captures supplementary Brave, Wikipedia, and Reddit edge
cases. Both use actual source adapter methods with `httpx.MockTransport`; no
provider, DNS, or paid calls occur. Mandatory native A04 gates consume only the
committed JSON packs and never execute either exporter. A04 owns the source
observations; G08 owns executable retirement before backend removal.

The current source-path manifest grows by two backend-coupled tooling files,
and the developer-oracle invocation appendix grows by two rows. Historical F01
baseline identities remain unchanged. F01 documentary source pins require the
normal reviewed refresh; no runtime Python behavior or public client exception
is added.

## A04 corrected capture inventory

The frozen A04 packet retains 37 original observations and their context, 104 supplementary source observations, and six tagged nonfinite observations documenting the accepted F05 finite-output boundary. The same two developer-only exporters now have three explicit invocation records. This checkpoint inventories 377 Python source paths and 15 developer-oracle invocations; historical example and py.txt counts remain unchanged. All earlier complete source and invocation records are preserved. Staged source paths are checked exactly before F01 documentary pins are refreshed.


## A04 fourth-review source semantics and explicit representations

The corrected packet preserves all 37 original source records/context, all 110
previous ordinary records, and all six tagged nonfinite observations. The
ordinary matrix now has 296 authentic source observations. A separate 14-record
representation fixture retains source boolean/overrange media and non-string
selftext outcomes alongside explicitly reviewed native whole-response refusals.
The existing two source exporters have four separate commands; all previous 15
full developer invocation rows are unchanged and the appendix now contains 16.
Python source paths remain exactly 377; historical example count 49 and all
historical py.txt counts/modes remain unchanged. This entry changes no accepted
runtime Python source pin or historical reference capture.


The final A04 fourth-review extension retains that 296/14 checkpoint as an exact
record prefix. It now inventories 333 ordinary source observations and 126
explicit representation observations, with source-error precedence controls.
The same fourth exporter command captures approved String/Option<String>
malformed-output refusals across all three adapters; the four commands and all
16 invocation rows remain unchanged. Source-path and historical counts remain
377 and 49 respectively. No runtime/reference source pin changes.


## A04 fifth-review source JSON boundary observations

The A04-owned offline exporter adds a separate `--source-json-output` invocation
and source_json.json evidence for byte encoding, Python numeric decoding,
Unicode scalar output boundaries, and bounded native JSON resources. Existing
37/context/333/126/6 captures and all historical/runtime source pins remain
unchanged. The existing exporter path is reused: projected Python source paths
remain377, developer oracle invocation rows become17, and historical example
reference count remains49. Source-success/native-refusal deltas and actual
Python decoder exceptions are explicit in A04/PARITY.md and the new corpus;
they do not establish arbitrary JSON or other-adapter parity. Public codec and
storage remain unchanged. This entry requires narrow own-document/exporter
anchor review; it does not authorize refreshing runtime source pins.

## A04 accepted-main integration preparation

The reviewed A04 checkpoint is integrated with accepted F14 main `576d43b03c0378ffe113a1386b1ec88eaa4323df`. All complete parent inventory records remain unchanged: the additive union has 383 Python source paths and 24 developer-oracle invocation rows, including both W01 commands. Historical A04 captures and source pins are immutable. Its shared filter-semantics observation now uses the accepted exact PR874 source-freshness bridge and separately bound 22-case date evidence; this does not change adapter runtime semantics. Parent integration review, exact-head CI and acceptance remain pending.

## A04 accepted W00 main integration

The fully qualified A04/F14 checkpoint `ca7c6209b4b5e67f5916297e632afbd93f09d758` is normally integrated with accepted W00 main `950071404c61eabc1201730362baccd01b3f2295`. The complete-record union is384 Python source paths and25 developer-oracle invocations. Both parent records and all raw captures remain exact. W00 adds saved modules, gate and evidence metadata; A04 runtime and its exact optional-subreddit cache/flight correction remain unchanged. This entry changes only combined documentary bindings; independent integration review and acceptance remain pending.

## A05 HTML adapter source and temporary oracle

A05 starts at `01876639fb8d87123f12293c02821981cff3f8b9`, after accepted A01
PR #969. DuckDuckGo/Google Python source and named regression identities remain
pinned in `a05/corpus.json`; native code uses accepted registry/HTTP/quota seams.
One backend-coupled offline exporter is added under `a05/oracle/export.py`, owned
by A05/G08. It is never called by runtime, CI or native gates; G08 retires the
executable and retains the historical JSON. Disposition/source counts grow by
one; developer-only oracle invocation inventory grows by one. No plan edges or
mandatory Python gates are removed.

A05's complete outcome/request replay preserves the source 5 ms bootstrap bug,
text versus lite/image redirect decoding, malformed-empty HTML failures and
Python ASCII/Latin-1 codecs. `a05/README.md` records two precise reviewed
differences: F08 typed network-error diagnostics omit arbitrary HTTPX exception
text, and accepted F08 host-only cookies do not leak from parent to subdomain.
Domain-cookie continuity remains supported. Live provider compatibility is
unverified; no deployment or provider request is authorized by this evidence.

A05 independent review appended 43 whole-source cases in `a05/corpus-review.json`
while retaining the original 115-case file byte-for-byte. Corrections preserve
context-inclusive lxml selectors, declarations/PI/open-comment empty-document
classification, Python quote_plus bytes and UTF-32 decoding. No further parity
exceptions were approved. The existing exporter optionally writes both pinned
corpora; its invocation inventory row changes without adding Python sources or
commands to native gates. The already-locked html5ever 0.39.0 package becomes a
direct ssx-infra dependency for tokenization; Tokio test-util is enabled only as
a dev dependency for the controlled bootstrap clock. Lock versions/checksums
are unchanged. Exact source identity/pin changes require parent review.

## A05 accepted-main integration checkpoint (qualification pending)

Local A05 checkpoint c8b9516f merges accepted main
1e9dd82d62456fe6f511cc5ade88bae619fac027. This documentary union preserves all
complete parent source/invocation records: 385 historical source rows and 26
historical developer-oracle invocation rows. Those counts describe the merged
historical inventories, not the complete current A05 source set. Additional A05
codec/provenance/timer migration oracles are not registered yet and require full
F01/G08 inventory and retirement reconciliation. No runtime or existing source
pin is refreshed. Accepted-main F01 document pins remain unchanged and are
expected red after this documentary union until exact parent-reviewed refresh.
All accepted main workflow/module/gate entries are retained; A05 adds its scoped
step and artifact without claiming package, provider, resource or CI acceptance.
## W11 staged-store source oracle

At baseline `1e9dd82d62456fe6f511cc5ade88bae619fac027`, add the isolated developer oracle `docs/rust-rewrite/w11/oracle/export.py` and its documented invocation. It executes source AST functions and staged-store read/retry controls against private disposable Valkey only. Mandatory W11 checks consume immutable JSON and execute native Rust. G08 retires this executable Python before backend removal; the original source snapshot remains non-executable historical evidence. Existing inventory entries and runtime boundaries are unchanged.

## F15 bounded reranking and advisory source oracles

The F15 source-bound gate adds exactly two temporary developer-only Python exporters, `f15/capture.py` and `f15/capture_headers.py`, with two corresponding invocation rows. The complete384 source records and25 invocation rows at accepted A04 main remain unchanged, including both W01 calls; the additive union is386/27. The independently recaptured150-case corpus preserves its original124 cases, and the separate12-case streamed-header corpus records actual body-read counts. Source baseline `4dfc0f5691b6a5a1c039a2837372b1977422ec27` and all13 historical blobs are immutable. No provider calls or native Python runtime dependency are introduced. G08 retires executable capture tools while retaining immutable observations. F15 package review and maintainer acceptance remain pending.

## W11 integration with accepted F15

The additive union preserves both original inventories and all historical source pins:387 Python source paths,264 backend-coupled tooling/test paths and28 developer oracle invocations. Both W11 and F15 native gates and retained artifacts remain enabled. This records integration preparation; merged-head validation remains required.


## A07 temporary source evidence inventory

A07 biomedical candidate adds12 exact-hash temporary offline source producers,396 projected Python source paths and37 developer-oracle invocation rows against this baseline. All existing records/historical captures unchanged; native A07 gate consumes nonexecutable JSON only and never requires or executes producers. G08 owns executable retirement before backend removal. This is additive inventory, not native runtime Python permission. Projected counts describe this candidate only; recompute after normal accepted-main integration before final acceptance. No complete existing record was replaced.


## A07 normal accepted-main integration

The DCO A07 checkpoint is normally merged with accepted main1bad48b (F15/W11/W02). All complete parent and A07 inventory records are retained:399 tracked Python source paths and40 temporary developer-oracle invocation rows. Existing source observations and source Git pins remain immutable. Historical native/CI baseline blobs are checked separately from current native implementations. Final integrated gates and exact-head CI remain required.

## A05 normal integration with accepted A07 main (qualification pending)

Local DCO checkpoint `c9d0e18b9024cf9decdb4710aaa2252e23511976` is normally
integrated with accepted main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0`.
The complete historical-record union retains400 source rows and41 developer
oracle invocation rows, including every complete parent record. These are not
current-source completeness counts: later A05 producers remain unregistered
until the explicit final F01/G08 reconciliation. Existing historical pins and
captures remain unchanged; document pins are not refreshed to manufacture a
passing gate. All main registrations, gates and artifacts remain present, with
A05 added separately. Integrated runtime, platform and exact-head qualification
remain pending.

## A05 final Python producer reconciliation (review proposal)

The exact current tracked set is456 Python source paths, including56 newly
classified A05/adopted provenance paths. All400 prior complete records and all41
actual developer-oracle invocation records remain unchanged. Four upstream
licensed Python originals are historical evidence and must leave the runnable
backend/helper tree at G08;52 tools are temporary manual source or derivation
producers. A separate52-entry AST-derived invocation-guidance appendix describes
actual arguments and output sites without claiming CI execution or recapture.
The native A05 gate consumes retained JSON/binary observations only.

Current documentary fixture bindings and the current A05 exporter binding are
proposed separately from historical source identities; previous fixture blobs
remain recorded in the immutable reconciliation receipt. No adapter-source or
raw-capture pin changes. Full G08 removal and final package/platform qualification
remain pending.

## A05 authored producer maintenance and immutable hook policy

The complete456 original classification records and41 historical developer
invocations remain unchanged. Current AST-equivalent maintenance identities for
53 authored producers (52 new tools plus the previously registered exporter)
are separate in `a05/producer-identity-amendments.json`; original source Git
blobs and full classification rows remain retained there. Historical capture
and generation producer hashes describe the original executions and are not
repinned to imply recapture. The only AST normalization splits multi-alias
imports into consecutive singleton imports in identical execution order.

Exact40 immutable source/provenance/fixture paths are excluded only from
modifying text hooks; four licensed upstream Python originals additionally
avoid Ruff modification. Existing validators remain enabled. Authored producers
retain lint/format checks; intentional bootstrap/import/API names use reviewed
line-local annotations. The unrelated baseline saved-method test is preserved.

## A08 actual source migration evidence

A08 starts from fresh accepted-main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0`.
Four exact-hash temporary source producers have actual completed offline or
private-loopback invocation receipts in `a08/oracle-runs.json`. The candidate
union is403 tracked Python source paths and44 developer-oracle invocation rows.
Every existing parent record and historical capture is preserved. Native A08
runtime and mandatory gate read immutable JSON only and never execute Python.
G08 retires all four executable producers; source captures/provenance remain.
Initial exploratory/partial attempts are not these accepted capture receipts.

A08 regression-mapping correction against candidate `9e1b4453` retains the
original 47 passing parameter instances / 44 functions and 38 original adapter
invocations, and adds five whole-adapter replays of the exact observed Oyez
URL-helper assertion inputs. The existing inventoried producer captures those
inputs and performs all five offline adapter calls; its updated exact hash and
executed receipt remain bound here and in F01. No producer count or retirement
owner changes. Genuine registration-only disposition is restricted to the five
explicit original registration nodes.
## W04 saved-runner source oracle inventory correction

W04 adds the temporary offline synthetic `docs/rust-rewrite/w04/capture_source.py` producer. Its existing local oracle inventory did not update the global F01 source set; this correction adds its exact source binding, backend-coupled tooling/test classification, W04/G08 ownership and reproduction guidance. The guidance is not an execution receipt. Existing source/invocation records remain unchanged: the additive union is388 Python source paths,265 backend-coupled tooling/test paths and29 developer-oracle invocation rows. Native runtime and mandatory gates never execute the exporter; the W04 gate hash-reads it as a temporary source artifact. G08 retires executable Python before backend removal and retains immutable JSON observations/provenance.

## W04 integration with accepted A07

Normal integration of accepted A07 main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0` preserves every complete parent record, yielding400 tracked Python source paths,277 backend-coupled tooling/test paths and41 developer-oracle invocation rows. Both A07 and W04 native gates and artifact uploads remain enabled. Historical observations and source pins are unchanged. This records integration; final combined-source validation and review remain required.

## A05 normal integration with accepted W04 main

The qualified local A05 checkpoint `fac23a9c9cb54cfe03ed9e84f6eb0872a3b6ccc8`
is normally merged with accepted W04 main `9b7d888da1ee3f7e2ed045c2185ea16f419381e9`.
All complete parent records are preserved:457 classified Python paths and42
historical developer-oracle recipes, with52 separate A05 manual guidance rows.
Current AST-equivalent producer identity amendments remain separate from original
capture/generation identities. Both A05 and W04 workflow steps/artifacts survive;
final integrated native gates and exact-head platform qualification remain pending.
## A08 union with accepted W04/main

The normal merge preserves the accepted A08 history through `f3ef8910` and
main/W04 `9b7d888d`. Both complete source inventories and developer-oracle
records are retained: 404 tracked Python sources and 45 developer-oracle
invocation rows. Existing source observations, captures and producer hashes
remain unchanged. The descriptive A08 regression-producer evidence SHA now
matches its already-enforced current hash. CI snapshot and evidence paths
retain both W04 and A08 gates. Main integration does not adopt pending A05/A06
shared seams; their future union remains a separate reviewed operation.

## A06 native qualification successor after accepted A05/main93db union

The former required `scripts/rust-a06-native-qualification.py` CI invocation is replaced by `cargo xtask check A06`. Its exact original interpreter row is retained verbatim in `retired_interpreter_invocations`; the executable remains a G08 retirement obligation. This removes one required invocation, not Python from whole CI. The native successor executes the original Expat source controls, same GNU resource-probe scope, exact negative exit checks and owned hanging-child kill/wait controls; no global backend/parser RSS guarantee is inferred.

The complete union retains 476 source records, 63 active interpreter rows, 56 actual developer-oracle rows and 60 manual guidance rows. The two new actual A06 rows reference the completed private-loopback timeout and resolved-config source captures in `a06/oracle-runs.json`; guidance remains distinct from executed evidence. Original historical producer identities and the accepted A05 current AST-equivalent identity amendment remain unchanged. All prior drift histories are preserved. Final source-bound current-main native gates and hosted architecture qualification remain required before lane acceptance; issue984 crosssurface proof remains open.

## Accepted A09/W06 main0a074 historical drift record

# Baseline drift and intentional differences

F01 inventory baseline: `20109f9083babd014971951e8c02565ae51c2326`.
Original planning baseline: `2a995f964757df77693747b232aa47738c0c58cc`.

## Observed changes

| Baseline interval | Observation | Parity impact |
| --- | --- | --- |
| Planning baseline → F01 baseline | Experiment report and rewrite planning documents merged; no changes in `slopsearx/` or `engines/` | No runtime capability changes; inventory uses the newer full repository revision |
| F01 baseline → `b7b79ab6` (observed while F01 was in review) | Three experiment Markdown documents changed in PR #949 | No source, dependency, workflow, executable or historical Python artifact changes; pinned inventory stays at its declared baseline |

| F01 merged `2e0026b4` → `6aa33a16` (observed during F02) | PR #951 changed 29 experiment Markdown/JSON registration and evidence files | No runtime, dependency, workflow or Python-source changes; architecture and pinned F01 source inventory remain applicable |

| `6aa33a16` → `65a55e2e` | PR #953 changed six experiment Markdown/JSON outcome files | No runtime, dependency, workflow or Python-source changes; F03 begins from F02 merge `562b6ce1` on top of this revision |

## PR #954 drift accepted during F03

Reviewed `562b6ce1` → upstream `73940f07` (PR #954), integrated through a
normal merge into F03. Historical F01 inventory and planning baseline SHAs
remain unchanged. The native F01 fixture pins reviewed current sources; it
checks freshness and does not establish completed native parity.

Two existing mandatory Python tooling modules and four existing test modules
changed, alongside experiment documentation and frozen JSON materials. No
Python files, dependencies, workflow commands or invocation sites were added
or removed. Existing disposition/invocation rows and plan owners remain valid;
no plan dependency or runtime capability changes are required.

| Changed source | Existing owner | Native parity obligation |
| --- | --- | --- |
| `scripts/coverage_band_companion.py` | O03/O08 | Accept exact corrected protocol/cohort/companion pins and v4 groups; reject cross-mixed pins and archival groups |
| `scripts/coverage_gate_calculation.py` | O08 | Accept exact corrected v4 protocol/cohort pins only with unchanged execution and quality/task-use contracts, frozen-pool stage modes and diagnostic-only useful count |
| `tests/test_coverage_band_companion.py` | O05 | Preserve both-stage acceptance, mismatched-pin rejection and group-inventory rejection |
| `tests/test_coverage_gate_calculation.py` | O05 | Preserve both-stage acceptance, unchanged gate/execution identities, mixed/unknown-pin and changed-quality rejection, and hard navigation failure |
| `tests/test_coverage_stage_orchestration.py` | O05 | Exactly representable monotonic test origin avoids rounding at the eight-hour boundary; production deadline semantics remain unchanged |
| `tests/test_coverage_task_use_materials.py` | O05 | Verify only two development queries changed, confirmation and quality contracts remain fixed, and material/candidate source hashes match |

Corrected material hashes: protocol
`cfac29e7d406dc4c7e45030fafeb6ba466923cfeb19b34e427d344507c9d9ba9`, cohorts
`ad731b5d99907242dbc7c6be0c671049e179df76d50a2148886e7e34b6b28b30`, companion
`5b802406687af0a3c1cd318a4f5dfc6250f2643e81547d2739f7521e81740ed2`.
O08 must preserve the exact accepted/rejected cases in successor verification;
O05 carries the new regressions before O09 switches mandatory gates. This
expands parity obligations without authorizing an experiment or changing gates.

## Drift procedure

1. Fetch upstream and follow AGENTS.md session synchronization rules.
2. Compare the last accepted inventory baseline against the new source revision
   with `git diff --name-status OLD..NEW` and inspect changed implementations,
   public declarations, tests, scripts and executable workflow commands.
3. Add or revise capability, Python-disposition and invocation rows; assign a
   plan owner to every addition. Record removed or changed contracts explicitly.
4. Review omitted/null/empty/error semantics and durable bytes/clock behavior.
   Do not normalize a newly observed difference into the replay oracle silently.
5. Update this ledger with the interval, source changes, decisions and affected
   issues. Modify issue scope and dependencies together when needed.
6. Validate exact source-set coverage, owner IDs, source/test references and
   dependency acyclicity before accepting the new baseline. A missing capability
   or unclassified executable blocks acceptance.

F03/F04 will carry this inventory into the native check and replay commands.
Until then, acceptance requires source-set comparison and recorded review;
there is no claim that native parity has been implemented.

## Intentional differences

None implemented or accepted by F01. Proposed hardening in the case study must
receive an explicit ADR, affected contract updates and evidence before it is
counted as an accepted parity difference. Migration of an internal cache
namespace must not implicitly change durable artifact identities.

## F04 temporary developer oracle addition

F04 adds `docs/rust-rewrite/replay/oracle/export.py` against accepted F03
revision `7b2d876307189d7a62000d1f289b3ef4b65eaff6`. This imports the pinned
Python backend to produce reproducible adapter, HTTP and MCP observations
using recorded responses and local fake engines. It is backend-coupled
developer tooling, not a client-only exception. F04 owns its source evidence;
G08 must retire the executable before final backend removal. Its invocation
is recorded separately from mandatory CI/container commands in
`python-invocations.json`. Native replay consumes JSON and never invokes it.

The source manifest grows from 363 to 364 Python paths, with backend-coupled
tooling/test paths growing from 240 to 241. Historical baseline identities
remain unchanged; this ledger records the reviewed post-baseline addition.
The three adapter and two surface observations prove replay infrastructure
coverage only; native implementation parity remains assigned to successor
issues. No live provider capture or experiment is authorized by this addition.

## F05 temporary pure-function oracle addition

F05 adds `docs/rust-rewrite/f05/oracle/export.py` against accepted F04
revision `16f03be656f34d90edd8285f9c81dfb1c4626b0a`. The exporter observes
pinned Python storage codecs, HTTP/MCP projections and canonical identities
using local inputs. It is backend-coupled developer tooling; it does not
qualify for the client-only exception. F05 owns capture provenance and G08
owns executable retirement. Native F05 checks consume portable JSON without
launching Python. No provider calls or deployment are authorized.

This branch adds one path to the accepted 364-path manifest: 365 total,
including 242 backend-coupled tooling/test paths. Historical F01 baseline
reports remain unchanged. The reference environment uses Python 3.14.7 and
Unicode 16.0.0 to match the production image's Python version; the Unicode
identity cases include mappings absent from Python 3.12's Unicode 15 tables.

## F05 explicit native input differences

The F05 implementation introduces the following differences for review with
ADR 003; final acceptance requires the F05 negative tests and reviewed PR.
Pure JSON parsing rejects input larger than 4 MiB or nested more than 100
levels, and rejects floating-point tokens that cannot be represented as a
finite f64. Arbitrary-width integer digits remain lossless. These are explicit
native input bounds, not a claim that every legacy record fits. Payload and
storage codecs apply their narrower record limits, including source-compatible
omission where applicable. Persisted-data compatibility still requires the
later store and migration qualification; these parser tests do not prove it.

Native artifact-reference decoding requires integer JSON version `1` and
rejects boolean `true`. The Python equality check accepts `true`, although
its producers write integer `1`. Negative tests must cover this rejection
and each parser bound. Existing opaque identifiers remain unchanged; this
change migrates no durable identity namespace. Public numeric values retain
integer-versus-float representation; comparator tolerance must not conceal
changes in payload values, timing fields, or canonical identity bytes.

## W01 temporary record/accounting oracle addition

W01 adds `docs/rust-rewrite/w01/oracle/export.py` against accepted main
`cfd7a0af61a22cfd040a7fcd41ad950f82005959` after F05/F07 acceptance. Its
Python3.14.7 source-bound capture uses synthetic research records, injected clock
and ID factories, and pure coverage/accounting functions. It makes no provider
or store requests. W01 owns these migration observations; G08 retires executable
Python before final backend removal. Native mandatory gates read captured JSON
and never execute the exporter. This is not a client-only exception.

The reviewed source manifest grows from 365 to 366 Python paths and from 242 to
243 backend-coupled tooling/test paths. The README capture command is classified
separately under developer oracle invocations; existing mandatory runtime/CI
commands and historical baseline reports are unchanged. The invocation summary
now reports the actual three existing F04/F05/W01 developer rows. F01 fixture
refresh is limited to the new oracle and these three edited inventory documents;
all prior runtime Python and historical baseline hashes remain unchanged.

The first W01 frozen F03 run correctly failed on the newly staged, unclassified
exporter. Its failed source-bound evidence is retained; this explicit inventory
addition closes that failure rather than weakening exact-set validation. W01's
native hardening differences are recorded separately in `../w01/parity.json`.

## F08 transport differences proposed for review

F08 starts at accepted F05 `7cc420f7ffb61127750e77bf6130e225925f9186`.
The [transport parity ledger](../f08/parity.md) and [ADR 006](../../adr/006-adapter-http-bounds.md)
record finite request/body/redirect bounds, unsupported encoding errors, stricter
redirect credential stripping and the global native idle-pool strategy. Native
source-bound socket tests qualify those cases; full adapter/provider and
platform/portal integration parity remains with the named downstream owners.
No Python source or production browser behavior changes in this slice.

## F11 temporary pure-algorithm oracle addition

F11 adds `docs/rust-rewrite/f11/oracle/export.py` against accepted F05 revision
`7cc420f7ffb61127750e77bf6130e225925f9186`. It captures only deterministic
ranking, bounded scholarly grouping and structural URL classification from
synthetic/public inputs. Native F11 CI replays committed JSON without Python,
DNS, providers or shared stores. The exporter is backend-coupled developer
tooling, owned by F11 for evidence and G08 for executable retirement; it is not
a client exception or mandatory native runtime/gate dependency.

The reviewed manifest grows from 365 to 366 Python paths and from 242 to 243
backend-coupled tooling/test paths. Historical F01 baseline identities and
validation reports remain unchanged. F01 source fixture updates cover this
new temporary file and these reviewed inventory documents only; existing
Python runtime source pins are preserved.

## Integrated W01, T01 and F10 source evidence

The W01 branch retains its research exporter and identity capture, accepted T01
retains the MCP exporter, and accepted F10 adds its quota/health exporter. These
are temporary developer-only source oracles. Native acceptance gates consume
captured JSON and do not execute the exporters or call providers; G08 owns their
retirement before backend removal. The exact inventory union contains 370 Python
source paths: 122 backend/runtime, 247 backend-coupled tooling/test and one
client-only path. Its developer-oracle invocation register contains seven rows.

F01 records the final merged inventory-document identities after the narrow
reviewed refresh and adds the accepted F10 exporter blob pin. Existing W01/T01/F10
exporter bytes, Python runtime pins and historical F01 baseline evidence remain
unchanged.

## T09 temporary browser identity oracle and native authority

T09 adds `docs/rust-rewrite/t09/capture.py` against accepted F06/F07 main
`1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`. It records 61 offline actual-source
observations with 11 exact source hashes; native gates replay the JSON and never
execute Python. Independent pinned-Python recapture matched every byte. T09 owns
the evidence and G08 executable retirement. The current union grows from 370 to
371 Python source paths and from 247 to 248 backend-coupled tooling/test paths;
the developer-oracle invocation register grows from seven to eight rows.
Existing runtime Python and prior oracle bytes/pins are unchanged.

T09 uses maintained AWS-LC signature/HMAC/random APIs from the already locked
TLS backend, server-authoritative Valkey time, exact-record session CAS and
explicit typed input bounds. Named source coercion/config/redirect corrections
and browser namespace drain/relogin consequences are recorded in
[the T09 parity ledger](../t09/PARITY.md). No G04/G07 cutover is authorized or
claimed by this module; legacy audit retention and durable workflow idempotency
remain required handoff obligations.

## F09 temporary source oracle

F09 adds one developer-only exporter for synthetic scope, policy, filter, date
and named-topic ordering observations. This is backend-coupled migration tooling,
not a client exception. Native F09 consumes pinned JSON and never executes Python.
G08 retires the exporter before backend removal. The exact inventory becomes
371 Python paths (122 runtime, 248 tooling/test, one client) and eight developer
oracle invocation rows. Existing runtime and baseline hashes are unchanged.

## T04 isolated OAuth reference

T04 adds `docs/rust-rewrite/t04/capture.py` as a temporary, offline, exact-source
migration oracle (T04/G08), not a runtime dependency or gate subprocess. It
captures pinned SDK registration normalization and source provider state locally;
no HTTP endpoint or provider is called. G08 removes it before backend retirement.
The exact Python source union has 371 paths: 122 backend/runtime, 248
backend-coupled tooling/test and one client-only. Existing runtime/oracle bytes
and all historical baseline identities remain unchanged.

## T04 and T09 accepted-main integration

The union retains both independent temporary oracles exactly once: 372 Python
source paths (122 backend/runtime, 249 backend-coupled tooling/test, one client-only)
and nine developer-oracle invocations. T09 now has 76 source observations: the
original 61 plus fifteen integer drain-setting cases. Historical section counts
above describe their original isolated additions; existing runtime/source and
prior oracle bytes remain unchanged. T04 is accepted at `403dfd3720e969fa829f426136bf0f05b0360c56`;
T09 acceptance and combined-source qualification remain separate.

## F09 and T04 integration

Both temporary oracles remain inventoried exactly once. The combined inventory
contains 372 Python sources: 122 backend/runtime, 249 backend-coupled tooling/test
and one client-only source. Historical isolated-addition counts above are retained.

## Accepted F09, T04 and T09 integration

The exact combined inventory has 373 Python source paths: 122 backend/runtime,
250 backend-coupled tooling/test and one client-only, with ten developer-oracle
invocations. F09, T04 and T09 temporary exporters remain inventoried once each;
G08 retains executable retirement ownership. Historical isolated/earlier union
counts above remain historical. All original runtime and source oracle bytes
are unchanged. F09 is accepted at `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`;
T09 remains in review and requires qualification against the combined source.

## A02 isolated package adapter observations

A02 adds two temporary developer-only exporters for independently source-bound
package registry and supplemental redirect/detail/query observations. They use
CPython 3.14.7 with httpx 0.28.1 and lxml 6.1.1 in the isolated migration lane;
mandatory native tests and gates consume fixed JSON and never invoke Python or
providers. A02 owns source comparison and G08 removes both executable exporters
before backend retirement. This is not a client/runtime exception.

The exact Python source union becomes 374 paths: 122 backend/runtime, 251
backend-coupled tooling/test and one client-only source. The developer oracle
invocation register becomes 11 rows. The F01 fixture adds the two exporter blobs
and refreshes only these three inventory document blobs; existing source pins
and all historical baseline evidence remain unchanged.

## Accepted T09 and A02 source inventory integration

The normal merge retains the accepted browser-identity oracle and both existing
W01 developer-oracle invocation rows, plus both A02 package-observation exporters.
The exact union is 375 Python source paths: 122 backend/runtime, 252
backend-coupled tooling/test and one client-only. The developer-oracle invocation
register contains 12 rows. Historical isolated-addition counts above are retained.
Only the three merged inventory-document F01 identities need documentary
refresh; no existing oracle or runtime source identity is changed.

## F12 optional routing reference

F12 adds `docs/rust-rewrite/f12/oracle/export.py`, a temporary offline source
oracle pinned to Python 3.14.7, MCP 2.3.0 and FastMCP 4.0.11. Synthetic local
transports capture exact Jev payloads, decisions, keyless/enhanced scopes and
cache identities. Native gates read immutable JSON; they never invoke Python.
F12/G08 own retirement before backend removal. At the isolated F12 commit
`e1f16cfa`, before the accepted T09 integration below, the inventory contained
373 Python sources: 122 backend/runtime, 250 backend-coupled tooling/test and one
client-only source, with ten developer-oracle invocation rows. These are historical
counts verified from that commit, not the final F12 integration totals of 374
paths and eleven invocations at `fc97fc6d`. Existing baseline source hashes and
historical evidence are unchanged.

## F12 integration with accepted T09

The combined inventory retains all accepted-main Python source rows and adds
exactly the F12 reference exporter: 374 source paths (122 backend/runtime,
251 backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. All prior source records and oracle bytes remain unchanged.
T09 is accepted at `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`; F12 remains
in review. Earlier counts and statuses above describe historical checkpoints.

## A02 integration with accepted T09 and F12

The normal integration preserves all reviewed A02 and accepted T09 records,
plus the accepted F12 routing oracle, including both W01 invocation records. The exact union is
376 Python source paths: 122 backend/runtime, 253 backend-coupled tooling/test
and one client-only source, with 13 developer-oracle invocation rows. Earlier
counts and in-review labels above describe historical checkpoints. Only the
three merged inventory-document F01 anchors need documentary refresh; runtime
and oracle source/capture identities remain unchanged.

## T05 OAuth wire migration oracle

T05 adds exactly one temporary backend-coupled source exporter and one developer
invocation entry. It captures actual pinned SDK loopback HTTP requests with a
synthetic authority, never provider calls. Native runtime/gates consume the JSON
fixture only. G08 owns exporter retirement. Existing runtime/source inventory pins
remain unchanged; F01 adds this exporter and refreshes only reviewed inventory
bookkeeping documents. T05's native all-network auth, resource binding and bounded
form refusal differences are explicit in [the parity ledger](../t05/PARITY.md).

The developer-invocation summary repairs a stale count7: eight existing rows are
unchanged and the single T05 addition produces nine actual rows.

## T05 integration with accepted F09/A01

The combined inventory retains all 372 accepted-main Python sources unchanged and
adds the one T05 migration exporter: 373 sources (122 backend/runtime, 250
backend-coupled tooling/test, one client-only). All nine accepted-main developer
oracle invocation rows remain unchanged; T05 adds the tenth. Historical counts
above describe their isolated checkpoints rather than this combined source tree.

## T05 integration with accepted T09

The combined inventory preserves all 373 accepted-main Python sources and adds
only T05: 374 sources (122 backend/runtime, 251 backend-coupled tooling/test, one
client-only). All ten accepted-main developer invocation rows remain unchanged,
including both distinct W01 commands; T05 adds the eleventh. Historical counts
above describe their isolated checkpoints, not this combined source tree.

## T05 integration with accepted F12

The combined inventory retains all 374 accepted-main source rows and adds only
T05: 375 sources (122 backend/runtime, 252 backend-coupled tooling/test, one
client-only), and twelve complete developer-oracle invocation rows. Both distinct
W01 commands remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; earlier counts and statuses describe
historical checkpoints. Runtime and reference-oracle bytes remain unchanged.

## A02 integration with accepted T05

The normal integration preserves all reviewed A02 and accepted T05, T09 and
F12 source records, including both distinct W01 invocation rows. The exact
full-record union contains 377 Python source paths: 122 backend/runtime, 254
backend-coupled tooling/test and one client-only source, with 14 developer-oracle
invocation rows. Earlier counts and statuses describe historical checkpoints.
Only the three merged inventory-document F01 anchors require documentary
refresh; accepted runtime, oracle and source capture identities remain unchanged.

## F13 temporary source-capture inventory

F13 adds one optional backend-coupled exporter,
`docs/rust-rewrite/f13/capture.py`, with two documented invocations (forward
source capture and reverse-read of actual native-created snapshot bytes). Native
F13 gates invoke no Python. G08 retires this executable after the rollback window;
immutable source observations remain. All original disposition/invocation rows
are unchanged. This branch now inventories 378 Python sources: 122 runtime,
255 backend-coupled tooling/test and one client-only source, plus 16 temporary
developer-oracle invocation rows. The normal integration of accepted F12/T05/A02 main retains all complete records
from both parents, including both existing W01 invocation rows. No runtime
Python or source-oracle identity is refreshed by this documentary union.

## A03 community adapters

A03 adds exactly one temporary developer-only exporter,
`docs/rust-rewrite/a03/capture.py`, owned by A03 with executable retirement under
G08. Its pinned Python/HTTPX observations use injected offline transport only;
mandatory native gates consume committed JSON without invoking Python. All
373 existing source rows and ten developer invocation rows remain unchanged.
The combined counts are 374 Python sources (122 backend/runtime, 251
backend-coupled tooling/test, one client-only) and eleven developer-oracle
invocations. No runtime source or accepted oracle pins are refreshed.



## A03 integration with accepted F12

This combined checkpoint retains every complete source/invocation record from
A03 and accepted F12: 375 Python sources (122 backend/runtime, 252
backend-coupled tooling/test, one client-only) and twelve developer-oracle
invocations. Both W01 invocation rows remain distinct and unchanged. Source
exporter/runtime bytes remain unchanged. F12 is accepted at
`d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; T09 is accepted at
`35d97ccf8c9b2d8246373b2024cc8d7a457705a3`. Earlier counts/statuses describe
historical checkpoints; A03 acceptance remains pending.


## A03 integration with accepted T05

This checkpoint preserves every complete source/invocation record from both
parents: 376 Python sources (122 backend/runtime, 253 backend-coupled
tooling/test, one client-only) and thirteen developer-oracle invocation rows,
including both distinct W01 commands. All runtime and reference-oracle bytes
remain unchanged. T05 is accepted at
`66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`; F12 and T09 remain accepted as
recorded above. Earlier counts/statuses remain historical; A03 acceptance is
pending.

## A03 integration after A02 acceptance

The combined source manifest retains all complete A02 and A03 oracle records: 378 Python source paths (122 runtime, 255 backend-coupled tooling/test, one client) and 15 developer oracle invocation records. Earlier counts above describe their historical checkpoints. No executable Python is granted a new runtime exception.

## F13 integration after A03 acceptance

All complete parent records are preserved: 379 source paths and 17 developer-oracle invocations. A03 was accepted in PR #975 at `5c1b48b9e625501ab61828299168101ac00301b3`. Earlier counts describe historical checkpoints. The F13 raw snapshot EOF exception remains narrowly bound in the native hygiene ledger.

## F14 execution and PR874 publication-date amendment

F14 adds exactly two temporary developer-only source capture programs and two invocation rows. All 379 prior source records and all 17 prior invocation records, including both distinct W01 commands, remain unchanged; the resulting exact source set is 381 (122 runtime, 258 backend-coupled tooling/test, one client), with 19 developer-oracle invocations. Neither exporter is a runtime exception or mandatory gate dependency; G08 retires executable Python while retaining immutable JSON observations.

The historical 730c corpus preserves all initial 46 objects and 24 source bindings, adding only two availability observations. A separate independently recaptured d207 corpus contains 14 date admission and eight cache mutation observations across 11 source bindings. PR874 changes eight current Python anchors (formatter, MCP tools, HTTP server, service, and their four regression modules); historical captures retain original baseline identities. The native checker admits only the exact reviewed old/new Git blobs and new source SHA bindings. This source-freshness bridge does not assert unchanged behavior: bounded requests now require explicit audited OpenAlex and exact date evidence, with the corrupt self-consistent cache-scope case explicitly hardened against current authority. F09's 15 named native expectations remain unchanged. T02/T06/portal successors own proof envelope/availability mapping and surface contract qualification.

## W00 temporary saved-definition oracle

W00 adds one offline developer exporter at
`docs/rust-rewrite/w00/capture_source.py`, owned by W00/G08, against accepted
main `66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`. It captures actual saved records,
strict validation, configured-auth fingerprints, control revisions, source claim
precision and arithmetic boundaries with private Valkey and no provider calls.
The native W00 acceptance gate executes Rust tests only and consumes immutable
reviewed JSON; G08 retires this Python exporter before backend removal.

The existing 375 source rows and 12 developer invocation rows remain unchanged; the
new counts are 376 and 13. Both distinct W01 invocation rows are preserved. No
existing runtime, baseline source or legacy exporter identity is refreshed.
Source claim identity uses actual source rehydration after cjson: a retained early
failed capture hashed raw integer 1000 instead of the runner's float 1000.0. The
corrected observation is independently reproducible and the old failure remains
evidence, rather than a changed native hash rule. The [superseded capture and
boundary failures](../w00/evidence/README.md) are committed historical evidence;
current acceptance consumes only `w00/source.json`.

## W00 integration with accepted A02/A03/F13 and bounded-date main

The normal merge into accepted main `544c830724a322d05521dac7f902dbe93142c4a1`
preserves all complete parent source and developer-invocation records: 380 Python
sources (122 runtime, 257 backend-coupled tooling/test, one client-only) and 18
developer-oracle invocations. Both W01 invocation rows remain distinct. Earlier
counts/statuses describe historical checkpoints. Reviewed W00 runtime and oracle
bytes are preserved during conflict resolution; changes to current-source oracle
provenance require the explicit audit and review below rather than a blind pin
refresh. Accepted upstream runtime/exporter anchors retain their exact values.

PR #874 changes MCP tools only in ordinary search date-filter metadata and the
science tool's explicit OpenAlex requirement for absolute dates. No saved helper,
validation, policy fingerprint or saved control function changes. The accompanying
service/server/formatter changes add bounded-date validation, canonical response
proof and HTTP/portal rendering; these are outside W00 definition/claim behavior.
W00's whole-file MCP pin therefore needs a reviewed source-provenance mapping,
backed by offline recapture of all 55 saved observations with no provider calls.

The historical W00 MCP blob remains
`061593ce45d3e2440e818703af6b50d6e6d99cf1`; the accepted current blob is
`d06ecd475b9b523addf79781d2762eddd92f4911`. The separate
[55-observation integration recapture](../w00/integration-source.json) has reviewed
SHA256 `c500069182b9975539910e29683531759e0df375e049e8daea5b8664aa10aad3`.
Every field equals the historical corpus except exactly the named MCP source-hash
field. The gate checks both pinned source generations and that exact equivalence;
it does not refresh historical outcomes or ignore a general metadata group.

## W00 dependent F14 integration preparation

The reviewed F14 commit `2427bf38e2241d1ba23637e233f97e7d0ee6d5ea`
is integrated while PR #978 awaits acceptance. This is dependent preparation,
not a claim that F14 or W00 has been accepted. The complete source-record union
contains 382 paths and 20 developer-oracle invocations. Historical source
observations and their original provenance remain unchanged. Both W01 rows,
W00's source integration mapping and F14's exact PR874 bridge are retained.
Final publication requires accepted upstream integration and qualification.

## A04 temporary source-capture oracles

A04 adds two temporary, offline-only Python 3.14.7 exporters under
`docs/rust-rewrite/a04/oracle/`: `export_cases.py` reproduces all 37 original
source captures and their per-record config/status/exception context, while
`export_matrix.py` captures supplementary Brave, Wikipedia, and Reddit edge
cases. Both use actual source adapter methods with `httpx.MockTransport`; no
provider, DNS, or paid calls occur. Mandatory native A04 gates consume only the
committed JSON packs and never execute either exporter. A04 owns the source
observations; G08 owns executable retirement before backend removal.

The current source-path manifest grows by two backend-coupled tooling files,
and the developer-oracle invocation appendix grows by two rows. Historical F01
baseline identities remain unchanged. F01 documentary source pins require the
normal reviewed refresh; no runtime Python behavior or public client exception
is added.

## A04 corrected capture inventory

The frozen A04 packet retains 37 original observations and their context, 104 supplementary source observations, and six tagged nonfinite observations documenting the accepted F05 finite-output boundary. The same two developer-only exporters now have three explicit invocation records. This checkpoint inventories 377 Python source paths and 15 developer-oracle invocations; historical example and py.txt counts remain unchanged. All earlier complete source and invocation records are preserved. Staged source paths are checked exactly before F01 documentary pins are refreshed.


## A04 fourth-review source semantics and explicit representations

The corrected packet preserves all 37 original source records/context, all 110
previous ordinary records, and all six tagged nonfinite observations. The
ordinary matrix now has 296 authentic source observations. A separate 14-record
representation fixture retains source boolean/overrange media and non-string
selftext outcomes alongside explicitly reviewed native whole-response refusals.
The existing two source exporters have four separate commands; all previous 15
full developer invocation rows are unchanged and the appendix now contains 16.
Python source paths remain exactly 377; historical example count 49 and all
historical py.txt counts/modes remain unchanged. This entry changes no accepted
runtime Python source pin or historical reference capture.


The final A04 fourth-review extension retains that 296/14 checkpoint as an exact
record prefix. It now inventories 333 ordinary source observations and 126
explicit representation observations, with source-error precedence controls.
The same fourth exporter command captures approved String/Option<String>
malformed-output refusals across all three adapters; the four commands and all
16 invocation rows remain unchanged. Source-path and historical counts remain
377 and 49 respectively. No runtime/reference source pin changes.


## A04 fifth-review source JSON boundary observations

The A04-owned offline exporter adds a separate `--source-json-output` invocation
and source_json.json evidence for byte encoding, Python numeric decoding,
Unicode scalar output boundaries, and bounded native JSON resources. Existing
37/context/333/126/6 captures and all historical/runtime source pins remain
unchanged. The existing exporter path is reused: projected Python source paths
remain377, developer oracle invocation rows become17, and historical example
reference count remains49. Source-success/native-refusal deltas and actual
Python decoder exceptions are explicit in A04/PARITY.md and the new corpus;
they do not establish arbitrary JSON or other-adapter parity. Public codec and
storage remain unchanged. This entry requires narrow own-document/exporter
anchor review; it does not authorize refreshing runtime source pins.

## A04 accepted-main integration preparation

The reviewed A04 checkpoint is integrated with accepted F14 main `576d43b03c0378ffe113a1386b1ec88eaa4323df`. All complete parent inventory records remain unchanged: the additive union has 383 Python source paths and 24 developer-oracle invocation rows, including both W01 commands. Historical A04 captures and source pins are immutable. Its shared filter-semantics observation now uses the accepted exact PR874 source-freshness bridge and separately bound 22-case date evidence; this does not change adapter runtime semantics. Parent integration review, exact-head CI and acceptance remain pending.

## A04 accepted W00 main integration

The fully qualified A04/F14 checkpoint `ca7c6209b4b5e67f5916297e632afbd93f09d758` is normally integrated with accepted W00 main `950071404c61eabc1201730362baccd01b3f2295`. The complete-record union is384 Python source paths and25 developer-oracle invocations. Both parent records and all raw captures remain exact. W00 adds saved modules, gate and evidence metadata; A04 runtime and its exact optional-subreddit cache/flight correction remain unchanged. This entry changes only combined documentary bindings; independent integration review and acceptance remain pending.

## A05 HTML adapter source and temporary oracle

A05 starts at `01876639fb8d87123f12293c02821981cff3f8b9`, after accepted A01
PR #969. DuckDuckGo/Google Python source and named regression identities remain
pinned in `a05/corpus.json`; native code uses accepted registry/HTTP/quota seams.
One backend-coupled offline exporter is added under `a05/oracle/export.py`, owned
by A05/G08. It is never called by runtime, CI or native gates; G08 retires the
executable and retains the historical JSON. Disposition/source counts grow by
one; developer-only oracle invocation inventory grows by one. No plan edges or
mandatory Python gates are removed.

A05's complete outcome/request replay preserves the source 5 ms bootstrap bug,
text versus lite/image redirect decoding, malformed-empty HTML failures and
Python ASCII/Latin-1 codecs. `a05/README.md` records two precise reviewed
differences: F08 typed network-error diagnostics omit arbitrary HTTPX exception
text, and accepted F08 host-only cookies do not leak from parent to subdomain.
Domain-cookie continuity remains supported. Live provider compatibility is
unverified; no deployment or provider request is authorized by this evidence.

A05 independent review appended 43 whole-source cases in `a05/corpus-review.json`
while retaining the original 115-case file byte-for-byte. Corrections preserve
context-inclusive lxml selectors, declarations/PI/open-comment empty-document
classification, Python quote_plus bytes and UTF-32 decoding. No further parity
exceptions were approved. The existing exporter optionally writes both pinned
corpora; its invocation inventory row changes without adding Python sources or
commands to native gates. The already-locked html5ever 0.39.0 package becomes a
direct ssx-infra dependency for tokenization; Tokio test-util is enabled only as
a dev dependency for the controlled bootstrap clock. Lock versions/checksums
are unchanged. Exact source identity/pin changes require parent review.

## A05 accepted-main integration checkpoint (qualification pending)

Local A05 checkpoint c8b9516f merges accepted main
1e9dd82d62456fe6f511cc5ade88bae619fac027. This documentary union preserves all
complete parent source/invocation records: 385 historical source rows and 26
historical developer-oracle invocation rows. Those counts describe the merged
historical inventories, not the complete current A05 source set. Additional A05
codec/provenance/timer migration oracles are not registered yet and require full
F01/G08 inventory and retirement reconciliation. No runtime or existing source
pin is refreshed. Accepted-main F01 document pins remain unchanged and are
expected red after this documentary union until exact parent-reviewed refresh.
All accepted main workflow/module/gate entries are retained; A05 adds its scoped
step and artifact without claiming package, provider, resource or CI acceptance.
## W11 staged-store source oracle

At baseline `1e9dd82d62456fe6f511cc5ade88bae619fac027`, add the isolated developer oracle `docs/rust-rewrite/w11/oracle/export.py` and its documented invocation. It executes source AST functions and staged-store read/retry controls against private disposable Valkey only. Mandatory W11 checks consume immutable JSON and execute native Rust. G08 retires this executable Python before backend removal; the original source snapshot remains non-executable historical evidence. Existing inventory entries and runtime boundaries are unchanged.

## F15 bounded reranking and advisory source oracles

The F15 source-bound gate adds exactly two temporary developer-only Python exporters, `f15/capture.py` and `f15/capture_headers.py`, with two corresponding invocation rows. The complete384 source records and25 invocation rows at accepted A04 main remain unchanged, including both W01 calls; the additive union is386/27. The independently recaptured150-case corpus preserves its original124 cases, and the separate12-case streamed-header corpus records actual body-read counts. Source baseline `4dfc0f5691b6a5a1c039a2837372b1977422ec27` and all13 historical blobs are immutable. No provider calls or native Python runtime dependency are introduced. G08 retires executable capture tools while retaining immutable observations. F15 package review and maintainer acceptance remain pending.

## W11 integration with accepted F15

The additive union preserves both original inventories and all historical source pins:387 Python source paths,264 backend-coupled tooling/test paths and28 developer oracle invocations. Both W11 and F15 native gates and retained artifacts remain enabled. This records integration preparation; merged-head validation remains required.


## A07 temporary source evidence inventory

A07 biomedical candidate adds12 exact-hash temporary offline source producers,396 projected Python source paths and37 developer-oracle invocation rows against this baseline. All existing records/historical captures unchanged; native A07 gate consumes nonexecutable JSON only and never requires or executes producers. G08 owns executable retirement before backend removal. This is additive inventory, not native runtime Python permission. Projected counts describe this candidate only; recompute after normal accepted-main integration before final acceptance. No complete existing record was replaced.


## A07 normal accepted-main integration

The DCO A07 checkpoint is normally merged with accepted main1bad48b (F15/W11/W02). All complete parent and A07 inventory records are retained:399 tracked Python source paths and40 temporary developer-oracle invocation rows. Existing source observations and source Git pins remain immutable. Historical native/CI baseline blobs are checked separately from current native implementations. Final integrated gates and exact-head CI remain required.

## A05 normal integration with accepted A07 main (qualification pending)

Local DCO checkpoint `c9d0e18b9024cf9decdb4710aaa2252e23511976` is normally
integrated with accepted main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0`.
The complete historical-record union retains400 source rows and41 developer
oracle invocation rows, including every complete parent record. These are not
current-source completeness counts: later A05 producers remain unregistered
until the explicit final F01/G08 reconciliation. Existing historical pins and
captures remain unchanged; document pins are not refreshed to manufacture a
passing gate. All main registrations, gates and artifacts remain present, with
A05 added separately. Integrated runtime, platform and exact-head qualification
remain pending.

## A05 final Python producer reconciliation (review proposal)

The exact current tracked set is456 Python source paths, including56 newly
classified A05/adopted provenance paths. All400 prior complete records and all41
actual developer-oracle invocation records remain unchanged. Four upstream
licensed Python originals are historical evidence and must leave the runnable
backend/helper tree at G08;52 tools are temporary manual source or derivation
producers. A separate52-entry AST-derived invocation-guidance appendix describes
actual arguments and output sites without claiming CI execution or recapture.
The native A05 gate consumes retained JSON/binary observations only.

Current documentary fixture bindings and the current A05 exporter binding are
proposed separately from historical source identities; previous fixture blobs
remain recorded in the immutable reconciliation receipt. No adapter-source or
raw-capture pin changes. Full G08 removal and final package/platform qualification
remain pending.

## A05 authored producer maintenance and immutable hook policy

The complete456 original classification records and41 historical developer
invocations remain unchanged. Current AST-equivalent maintenance identities for
53 authored producers (52 new tools plus the previously registered exporter)
are separate in `a05/producer-identity-amendments.json`; original source Git
blobs and full classification rows remain retained there. Historical capture
and generation producer hashes describe the original executions and are not
repinned to imply recapture. The only AST normalization splits multi-alias
imports into consecutive singleton imports in identical execution order.

Exact40 immutable source/provenance/fixture paths are excluded only from
modifying text hooks; four licensed upstream Python originals additionally
avoid Ruff modification. Existing validators remain enabled. Authored producers
retain lint/format checks; intentional bootstrap/import/API names use reviewed
line-local annotations. The unrelated baseline saved-method test is preserved.

## A08 actual source migration evidence

A08 starts from fresh accepted-main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0`.
Four exact-hash temporary source producers have actual completed offline or
private-loopback invocation receipts in `a08/oracle-runs.json`. The candidate
union is403 tracked Python source paths and44 developer-oracle invocation rows.
Every existing parent record and historical capture is preserved. Native A08
runtime and mandatory gate read immutable JSON only and never execute Python.
G08 retires all four executable producers; source captures/provenance remain.
Initial exploratory/partial attempts are not these accepted capture receipts.

A08 regression-mapping correction against candidate `9e1b4453` retains the
original 47 passing parameter instances / 44 functions and 38 original adapter
invocations, and adds five whole-adapter replays of the exact observed Oyez
URL-helper assertion inputs. The existing inventoried producer captures those
inputs and performs all five offline adapter calls; its updated exact hash and
executed receipt remain bound here and in F01. No producer count or retirement
owner changes. Genuine registration-only disposition is restricted to the five
explicit original registration nodes.
## W04 saved-runner source oracle inventory correction

W04 adds the temporary offline synthetic `docs/rust-rewrite/w04/capture_source.py` producer. Its existing local oracle inventory did not update the global F01 source set; this correction adds its exact source binding, backend-coupled tooling/test classification, W04/G08 ownership and reproduction guidance. The guidance is not an execution receipt. Existing source/invocation records remain unchanged: the additive union is388 Python source paths,265 backend-coupled tooling/test paths and29 developer-oracle invocation rows. Native runtime and mandatory gates never execute the exporter; the W04 gate hash-reads it as a temporary source artifact. G08 retires executable Python before backend removal and retains immutable JSON observations/provenance.

## W04 integration with accepted A07

Normal integration of accepted A07 main `2f78b9d02cfe62c9829fcfb38b85cd632a889de0` preserves every complete parent record, yielding400 tracked Python source paths,277 backend-coupled tooling/test paths and41 developer-oracle invocation rows. Both A07 and W04 native gates and artifact uploads remain enabled. Historical observations and source pins are unchanged. This records integration; final combined-source validation and review remain required.

## W06 manual staged source oracle registration

W06 adds the manual offline `docs/rust-rewrite/w06/capture_source.py` producer, explicitly backend-coupled tooling with W06/G08 ownership. The exact set is401 classified Python sources (278 backend-coupled tooling/test) and42 developer-oracle rows. Actual normal and optimized CPython3.14.7 capture receipts bind85 complete source cases and identical output bytes; a wrong source guard under optimization exits nonzero before imports. Native runtime and required gates never execute the producer. This is migration evidence, not client-only Python or runtime parity. G08 retires executable Python while preserving immutable JSON/provenance. Existing inventory records remain unchanged; only the new source and three mutable global document pins are added/refreshed. Final W06 runtime/gate/mutation/platform acceptance remains open.

### W06 supplemental source-language capture

An isolated manual temporary producer `/private/tmp/w06-language-source-capture.py` captures five nonstring language inputs through the original staged runner and shared service. All five complete with a permissive fixture engine; no late failure or provider compatibility is inferred. Its exact executable hash, actual optimized-interpreter receipt and frozen non-executable `.source.txt` copy are recorded in the external temporary-oracle appendix. Developer-oracle rows become43; tracked Python sources remain401 and active interpreter invocations remain63. W06/G08 own removal of the temporary executable. Native typed-string rejection is an explicit reviewed difference. The source text copy is historical provenance, never executable runtime or a required gate.

W06 retained-policy supplement records thirteen actual offline original policy outcomes using `/private/tmp/w06-retained-policy-source-capture.py`; frozen non-executable source and repeat-identical -O receipts are under `w06/evidence/`. This temporary manual backend-coupled oracle belongs to W06/G08 and is never a native runtime or mandatory gate dependency.

W06 saved-report retained authorization supplement records thirteen actual original outcomes with source guards and identical normal/-O repeats; temporary manual producer `/private/tmp/w06-saved-source-authority-capture.py` is W06/G08-owned, never a native runtime or mandatory gate dependency. Frozen non-executable source/JSON/receipts are under `w06/evidence/`.

W06 public/error/replay supplement adds125actualsource cases (121parity inputs plus4explicitrunner referencecontrols), with14preimport source guards, immutable original85guard, identical normal/optimized repeats and optimized wrong-source rejection. Temporary manual producer `/private/tmp/w06-review-public-source-capture.py` belongs to W06/G08 and is never executed by native runtime or required gates. Native reference-control differences remain explicit.

W06 R04 truthiness supplement adds16actualoriginalPlan boundaries for hugeintegers/signedzero/finitefloats andconsent, withguardedidenticalnormal/optimized repeats. Temporary manual `/private/tmp/w06-huge-integer-truthiness-capture.py` belongs to W06/G08; frozen non-executable source and JSON evidence remain under w06/evidence, never a native runtime or mandatory gate dependency.
## A05 normal integration with accepted W04 main

The qualified local A05 checkpoint `fac23a9c9cb54cfe03ed9e84f6eb0872a3b6ccc8`
is normally merged with accepted W04 main `9b7d888da1ee3f7e2ed045c2185ea16f419381e9`.
All complete parent records are preserved:457 classified Python paths and42
historical developer-oracle recipes, with52 separate A05 manual guidance rows.
Current AST-equivalent producer identity amendments remain separate from original
capture/generation identities. Both A05 and W04 workflow steps/artifacts survive;
final integrated native gates and exact-head platform qualification remain pending.
## A08 union with accepted W04/main

The normal merge preserves the accepted A08 history through `f3ef8910` and
main/W04 `9b7d888d`. Both complete source inventories and developer-oracle
records are retained: 404 tracked Python sources and 45 developer-oracle
invocation rows. Existing source observations, captures and producer hashes
remain unchanged. The descriptive A08 regression-producer evidence SHA now
matches its already-enforced current hash. CI snapshot and evidence paths
retain both W04 and A08 gates. Main integration does not adopt pending A05/A06
shared seams; their future union remains a separate reviewed operation.

## W05 original pure event oracle

The optional W05 manual capture producer is explicitly inventoried as backend-coupled tooling/test with W05/G08 ownership. It guards opt-in, pinned interpreter, immutable output and all network connections before backend imports. Actual success and the earlier failed socket-class guard attempt are retained in w05/oracle-runs.json. Native runtime and mandatory gates never invoke Python; G08 retires the executable while retaining frozen source observations. Existing parent records and historical producer identities remain intact.
W06/A08 integration proposal retains both accepted drift histories and all
source/developer-oracle rows:405 tracked Python paths,282 backend-coupled
tooling/test paths,51 actual developer-oracle invocation rows and63 active
interpreter invocation rows. Full native F01 qualification remains pending
the approved integrated tree.

W06/A05 integration proposal preserves full-record parent multisets:462
tracked Python source paths and52 actual developer-oracle rows, with63 active
interpreter invocation rows unchanged. The pre-A05303-case qualification on
4cc5858c remains historical evidence; integrated requalification is pending.

## T06 native search/discovery union with W06 main0a074390

The native search/discovery surface and frozen observations use original source
revision `6c44976444340cc8f343fef61e2e8d723b189c19`, preserved through the normal
union with accepted W06 main `0a0743904264b0054e14cb19cdd78af7d31eb60c`.
All W06 and earlier complete inventory records remain unchanged. Twelve temporary
backend-coupled tooling/test Python files are added: eleven finite manual
reference producers and the current write-once guarded launcher, enumerated in
`docs/rust-rewrite/t06/G08.md`. The original eight producer/capture byte streams
are preserved; three prompt boundary producers and the current launcher are
linted. Generated bytecode is neither delivered nor a backend dependency.

The exact Python source set is478, including the existing extensionless `ssx`;
classification counts are backend/runtime122, backend-coupled tooling/test351,
client-only1 and historical evidence4. No runtime or client-only source was
added. The one documented guarded-launcher invocation raises the developer
oracle appendix from56 to57. Mandatory native gates read frozen JSON/source
bytes without executing Python; all twelve executable manual files retire under
G08 before backend removal. See T06's source-bound native gate, parity ledger
and exactly two accepted malformed-outer-params framework diagnostic rows.

## W05 normal union with accepted W06 main

The normal integration proposal preserves W05 checkpoint d82c0191 and accepted
W06 main0a074390, including both complete source and actual invocation records:
467 classified Python paths and57 developer-oracle invocation rows. Existing
observations and historical/current producer identities remain unchanged. Both
W05/W06 native gates and evidence uploads survive. The composed Cargo graph
retains W06 infra dev-depending on MCP and W05 MCP normally depending on infra;
actual combined compilation and native qualification remain required.

## T06 normal union with accepted W05 main01eb22

Both complete parent inventories and invocation records are retained: 479 Python
source paths and 58 manual developer oracle rows. No new Python runtime or
mandatory native gate dependency is introduced. Native qualification is pending.

## A06 / accepted W05 main01eb22 integration

The normal union of the scholarly candidate with accepted main `01eb22f63c8284d7773a70948386d1800d54ba70` preserves both histories and complete source/invocation records. The prospective inventory contains 482 Python source paths, 67 actual developer-oracle invocation rows, and 60 manual developer-tool guidance rows; guidance is not execution evidence. Stronger G08 retirement requirements remain, with the original A05 exporter introduction attribution retained. No captured source outcomes are rewritten.


## A11 source-bound native threat-intelligence candidate

Four adapter records gain only an additive native_successor: abuseipdb, greynoise,
otx and urlhaus. All479 existing Python-source records,58 prior manual-oracle
records and728 historical non-runtime examples retain every original field.
Five manual original producers and nine immutable source/verifier archives are
appended. Historical executed SHA, current formatted/styled SHA and exact
execution receipts remain separate; no current producer is recaptured to change
its identity. Native engine/domain execution and the mandatory A11 gate use Rust
only. G08 retires the executable original oracles before backend removal.

A11 binds321 complete original observations,32 original test functions and41
actual parameterized calls to compiled native cases; four extra synthetic YAML
entry controls disclose accepted-F06 validation differences. The proposed gate
has414 individually named cases across13 tests; exact-artifact qualification and
normal accepted-main integration are still required. Closed F05 refusals and
request/header/diagnostic/helper differences are explicit, not blanket output
normalization. Actual private wire proof covers its13 named rows. W09 owns full
production/portal composition and G03 owns global resource/performance evidence.

## A11 Unicode credential correction

The existing A11 candidate and its historical414-case evidence remain intact.
A separate original-only four-engine supplement proves real HTTPX ASCII header
normalization rejects the fixed synthetic Unicode credential before Transport.
The native pre-fix control observed four OK responses with one private HTTP
connection each; the corrected factories return four complete Error responses
with zero HTTP connections and a fixed safe diagnostic. The exact four output
bindings are separate from the unchanged370-entry replay difference ledger.

The candidate adds five case objects to the unchanged414 and one named native
test to the unchanged13:419 cases across14 tests. Final expanded qualification
and normal-main integration remain required. One optional manual producer and
one immutable executed-source archive are appended, giving six A11 producers;
all484 preceding source records and63 actual developer-oracle rows retain every
field and order. Current and executed credential producer bytes are identical.
No F06 contract, global HTTP header policy, provider call, native interpreter
dependency or mandatory Python execution is introduced. G08 owns executable
retirement; W09 and G03 composition/resource obligations remain unchanged.

## O01 native observability source evidence

The accepted T06/main8b48 composition retains all479 Python source paths. O01
adds seven external temporary producer identities as non-executable historical
source.txt copies and four receipt-backed actual invocation rows (58 to62).
The original53 v1 runner's wrong49 collection check remains a failure alongside
its53 passing source cases; v2 independently closes53 cases across42 functions.
SDK/state/render captures without separate process receipts remain capture-only
provenance, not invented invocation rows. Native O01 reads frozen JSON and
executes no Python. G08 retires the external runnable producers and legacy
backend/tests; immutable non-executable evidence may remain. Two actual HTTP
route rows and production bootstrap composition remain downstream T02/W09.

O01 also retains the inherited26-case exporter's co-staged historical identity
as an eighth external, capture-only producer. The old checkout and staged bytes
are untouched. No execution attestation or actual invocation row is invented;
all62 actual rows and479 source paths remain unchanged. G08 owns retirement
of that runnable old-checkout exporter as well as the seven current-goal tools.
