# A09 catalog, geocoding and archive adapter candidate

Issue #928; implementation baseline `9fae1639a57b9e3497c04c3f5fe304651d077ad5`.
A fresh fetch matched HEAD with divergence 0/0. A01 accepted merge
`025ab2f2d50febcb573ba62579eecf7e2d07ef3a` is an ancestor; its accepted source,
A01 handoff and live #917 acceptance were independently read. The live native
blocked-by relation for #928 named only #917. This is a local candidate; parent
source acceptance, current main integration, platform qualification and delivery
remain separate.

The five named implementations are Open Library, MusicBrainz, TMDB, Nominatim
and Internet Archive. They consume caller-owned F08 HTTP and SearchAdmission,
F06 resolved scalars/secrets, SearchContext and the accepted source codecs and
owned-operation projection. Factories attach independently through
`register_catalog_media`; availability does not alter catalog enablement,
active policy, grants or scope selection. Internet Archive remains disabled by
default and excludes `general`. All five remain specialists with existing Jev
routing cards, declared media capability and truthful unsupported filters.

`cargo xtask check A09` is native only. It verifies source/fixture identities,
exact original regression mappings, exact native package/target/test identities,
zero ignored tests and ordered per-source-case completion markers emitted after
real parser/request/output assertions and joined HTTP cleanup. It does not run
an interpreter or provider. `coverage.json` is separately SHA anchored in the
gate. Four temporary source producers in `oracle/` are manual evidence only, inventoried
in the global Python disposition/invocation ledgers under A09 and G08; G08 removes
the executables while preserving immutable captures and receipts.

`source.json` contains 343 actual offline source adapter controls, including
exact provider responses, requests and complete normalized outcomes.
`regressions.json` records 56 actual passing parameter instances across 51 original
functions, 42 original search invocations plus ten whole-adapter replays of
actual routing-helper assertions. It also preserves the actual original Open
Library rank/JSON formatter output. Inputs come from direct original regression
execution and separately designed synthetic provider cases; expected outputs
come from actual original execution. Every behavioral function maps its real
calls, or the explicitly owned downstream portal contract, rather than a generic
success sample. Static readiness counts were not treated as executed evidence.

`differences.json` records every exact output difference. Parent review accepted
105 library diagnostic wording replacements, 12 Internet Archive projection
exceptions becoming classified Error, and the malformed-field boundary: original
null/numeric titles or numeric publication dates become atomic Error because the
accepted native result requires text. Parent review accepted nine individual controls and nine mixed valid-then-invalid
controls with a corrected per-field reference packet and actual same-budget
release evidence. These eighteen controls are explicit boundary differences. Stringification would change the
original JSON types and is deliberately not claimed as parity. Source/native
status/results remain exact elsewhere. `header-differences.json` records each
full source/native request pair for HTTPX's implicit framework User-Agent;
explicit provider User-Agent, Accept and Bearer placement remain exact. The historical ledger remains
byte-exact. `header-profile-amendment.json` separately records expected current
native requests after the reviewed shared SlopSearX User-Agent profile; these
expectations become actual composition evidence only when replayed successfully.
The tests preserve the source and historical native tuples and compare current
observations without normalization.

The scoped native tests exercise actual F06 YAML/env-to-factory-to-request/outcome,
public no-auth requests and TMDB Bearer-only secrets, real socket cancellation and
owner shutdown, pending/denied/unavailable admission, deadlines and the one-request
archive fallback. Same-budget tests cover thumbnail overlap, handoff, refusal,
cancellation and every engine's atomic failure/release/next-call success. The
HTTP consumer executes actual Open Library parser → rank → existing six-component
URL parser → HTTP DTO and actual TMDB payload/thumbnail projection. These are
specific consumer contracts, not a complete public transport or HTML renderer.

`timeouts.json` adds twelve actual original private-loopback observations. Native
controls preserve catalog Error for configured zero/negative timers, distinguish
expired caller contexts, and assert zero or exactly one dispatched request. The
positive stalled diagnostic is explicit in `timeout-differences.json`. Six
nonfinite YAML controls follow the accepted F06 finite JSON profile and reject
before factory registration; this is a reviewed configuration supersession, not
raw-adapter parity. Infinite/NaN source stalls ended when the private server
closed, so those records do not claim source timer cancellation. All four
producers completed under `python -O`; explicit version/Unicode/source guards
remain active. Historical pre-correction captures are retained separately.

Earlier cancellation tests only proved early scratch release. The corrected
per-engine controls causally observe a stored first row and actual large second
field copy, then drop that suspended future and assert the same budget is zero.
The thumbnail control likewise observes its completed charged copy. These
observers compile only for tests; retained response/global memory remain G02.

One initial factory test mistakenly reached the default Open Library endpoint;
that failed run is disclosed in [INCIDENT.md](INCIDENT.md) and excluded from all
qualification claims. Current real-network tests fail closed on non-loopback
resolved URLs, missing YAML entries, external fallback URLs, proxy environment
and enabled Darwin system proxies. Injected transport replays cannot contact
providers. No provider or production call is part of the native gate.

See [BOUNDARIES.md](BOUNDARIES.md), [PORTAL_IMPACT.md](PORTAL_IMPACT.md),
`oracle-runs.json`, `coverage.json` and immutable failed/successful logs in
`evidence/`. No global RSS, live provider reliability, historical completeness,
full portal, full HTTP/MCP transport, relevance, deployment or final G09 claim is
made. The accepted checkout has no `graphify-out/GRAPH_REPORT.md` or wiki; the
required AST-only update attempt and outcome are recorded separately.
