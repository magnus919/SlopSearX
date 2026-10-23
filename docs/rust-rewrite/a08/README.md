# A08 native regulatory, financial and legal adapters

Implementation starts from freshly fetched `origin/main`
`2f78b9d02cfe62c9829fcfb38b85cd632a889de0`, with HEAD/main divergence `0 0`.
A01's accepted factory contract and A07's accepted SearchContext, SourceJSON,
owned-budget and payload primitives are used directly. A08 implements actual
EDGAR, FRED, openFDA and Oyez operations; no Python implementation, sidecar,
remote fallback, provider invocation or production deployment is part of the
native runtime or qualification gate.

`register_regulatory` attaches all four actual factories while leaving enabled,
auth, category and sensitive-selection policy with the caller. It shares the
caller-owned F08 transport and SearchAdmission. `RegulatorySettings::from_resolved`
uses F06's exact scalar variants, source startup blank-base normalization and
snapshot credentials. A direct Oyez `search_url` remains a typed adapter-only
setting: source YAML/F06 reject that unknown field; environment extras do not
silently become a new startup setting. FRED's query authentication stays in the
provider-mandated query field and Secret Debug stays redacted.

The frozen source corpus has 344 ordered controls. Instrumenting the actual
original offline tests also produced 38 adapter invocations from 44 function
nodes / 47 parameter instances, all passed. Five exact original Oyez URL-helper
assertion inputs are also captured during that run and replayed through the
actual source adapter, retaining original parameter identities and provider
term/docket metadata. Those helper nodes bind the actual native replay. Only
the five exact original registration nodes may use registration-only mapping.
The native replay executes 385 actual adapter controls and explicitly supersedes
two malformed bool/int FRED direct
keys with real F06 loader rejection before adapter instantiation. Each replay
asserts ordered method/URL/query/body, adapter-owned headers, request count, and
complete outcome/result fields. Measured latency and transport/library default
User-Agent headers are excluded; EDGAR's authored User-Agent and all declared
Accept/content-type headers are asserted. Exact per-case library User-Agent
differences remain frozen in `header-differences.json`. Prior source-bound
native loopback observations of absence for FDA/FRED/Oyez remain historical.
The A06 versioned default User-Agent is a candidate successor; validation of
the combined source is pending. Its explicit effective-current header artifact
(`source-amendments/default-user-agent-effective-v1.json`) retains the frozen
row identities and source values, while EDGAR's authored override remains
required. No full transport-header parity is claimed. Only integer versus f64 encoding of a
numeric result.score is normalized under the accepted FiniteF64 boundary;
booleans, strings and nonfinite scores are not silently coerced.

The expanded URL controls run both unresolved and validated-identity fallback
EDGAR paths and whole Oyez result normalization. Params begin at the first
semicolon in the final segment; earlier semicolons stay in the path. URL parsing
uses Python3.14's C0-prefix and CR/LF/tab view while EDGAR retains validated
original supplied bytes. Empty query/fragment, authority/port, slash and malformed
bracket controls retain source behavior. Balanced brackets, IPv4/IPvFuture and
all 19 qualified Unicode16 NFKC delimiter controls exercise actual source and
native adapters. The additive domain diagnostic seam reuses the existing
qualified boolean validator and table; previous media callers are unchanged. Direct Oyez numeric NaN and infinities
return the exact source JSON diagnostic before dispatch; actual F06 YAML rejects
these nonfinite scalars. Text `"NaN"` remains a dispatched JSON string. These
controls introduce no additional outcome differences.

The exact 72-row differences ledger retains source and native outcomes side by
side: 48 native/library diagnostic wording differences, 14 finite/scalar domain
refusals, eight MockTransport nonpositive-timer supersessions, and two typed
configuration supersessions. The static native transport errors carry no old
HTTPX exception URL context, so no fabricated HTTPX prose is returned. Authored
source diagnostics, 429 and timeout classifications remain distinct. FRED's
synthetic full-URL, surrounding-text, repeated/encoded-key and malformed-URL
controls prove that native returned errors do not expose the synthetic key.
Parent review of these exact cases remains part of delivery acceptance.

Twelve actual source private-loopback controls distinguish network timers from
MockTransport timing artifacts. Native factory tests execute each engine's
50 ms network timeout and observe socket EOF, reject nonpositive timers before
HTTP, and drop pending search futures with real socket EOF. Denied, unavailable
and pending admission stop before HTTP; caller context covers admission,
transport, decoding and cooperative normalization. FDA direct calls retain
independent per-request timers across the exact and optional broad requests;
no new aggregate source timer is invented.

FDA exact-query variants use an engine-local Unicode16 table from pinned
Python3.14.7 and the parent-reviewed A06 Cased/CaseIgnorable property capture
`db70a80f98f2758da47030752c826a42ee97aac12e482a7145a11f5efe15d1df`.
All 11,468 actual whole-string variant controls execute natively, including
FinalSigma across combining/ignorable characters and title boundaries. This
reuses exact operational properties rather than Rust's changing Unicode tables
or approximate category rules. Runtime reads only `case16-data.json`; the larger
whole-string control pack is test evidence.

Payload construction uses the narrow generic envelope accepted in commit
`3dd43bef8c71ff89bd3dbce6b72bf8a3aada586b`: financial/economic_series and
biomedical/drug_label retain full fields and ordered provenance. Existing
publication callers preserve bytes, charge ownership and previous 15 tests.
Payload-only surrogate sanitation is distinct from explicit refusal of
non-scalar public display fields. Full FRED notes and FDA indications survive
the shorter display prefixes.

Run `cargo xtask check A08 --output /absolute/designated-target/A08.json` with
the pinned Rust toolchain. It checks exact source/artifact pins and ordered case
IDs, the 44/47 mapping, exact package/target/qualified-test identities, actual
execution with no missing or ignored tests, and gate rejection controls. The
ordinary locked build, fmt and strict Clippy also run before review. CI executes
the real gate and retains its report. Local build/evidence output is confined
to `/Volumes/tank01/magnus/rust-builds/slopsearx/a08`, incremental disabled.

Four temporary source producers are inventoried globally with G08 retirement,
and `oracle-runs.json` records actual completed commands, exact producer/output
hashes and retained log hashes. Captures use source Python3.14.7/Unicode16 and
synthetic credentials, mocked transport or owned loopback. The initial partial
coverage attempt passed all 47 tests but failed the unrelated repository-wide
80 percent threshold; final capture explicitly disabled global coverage and
retained its successful actual receipt. Earlier exploratory captures are not
acceptance evidence. Native commands never execute these producers.

See [PORTAL_IMPACT.md](PORTAL_IMPACT.md) and [BOUNDARIES.md](BOUNDARIES.md) for
consumer evidence and genuine downstream obligations. Local scoped qualification
is not full server, portal, global resource, production or milestone acceptance.
