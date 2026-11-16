# F12 optional Jev routing

Issue #891, implementation baseline `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`.
F07/F08/F09 are accepted prerequisites. This deliverable implements native
specialist advice and qualification; subsequent search/HTTP/MCP composition owns
wiring this optional service into its complete public endpoints. No live provider,
search, deployment, relevance, latency improvement or production parity is claimed.

`ssx_infra::jev::prepare_optional` accepts an optional configured router. Keyless
mode uses the deterministic F09 scope. The router owns a pooled F08 HTTP client;
it sends exactly one POST containing model `jev-1.13.0`, raw user query and sorted
specialist questions. Cards/display names come from the accepted F06 registry;
effective categories and eligibility come from the current runtime catalog.
Results, credentials, tenant identity and search payloads are never question data.
The key is sent only as Bearer authorization. Missing/whitespace keys disable the
router. Raw explicit engines/categories, even whitespace-only lists, or any media
type suppress advice. Empty eligible candidates produce an empty zero-latency
decision without a request. A preview may make one decision request on a miss.

`PreparedAdvice` retains the original proposal and raw advice. Its preview is
informational. Consuming `authorize` applies current F09 policy again without a
second provider call. The resulting private `ValidatedScope` retains advice for
`with_revalidated`; changed policy, eligibility, budget or grants require fresh
admission. Never turn advice into an explicit engines proposal: that would invoke
F09's intentional explicit-scope bypass. F09 preserves baseline engines first,
admits additions only into remaining automatic cost/count bounds, and recomputes
final explanations with a pure helper that cannot change the selected union.
F09's operator-disabled catalog routing retains its existing baseline fallback
semantics, including baseline order; optional additions still satisfy the current
additions budget and eligibility. Rejected advice cannot evict that baseline or
leave orphaned score/provenance metadata. Sensitive engines are never advice candidates, including
when a sensitive grant exists. Active descriptors remain previewable while
`authorize` denies an unavailable executable implementation, as accepted in F09.

Provider scores must all exist, be finite and lie in [0,1]. Preserve Python's
Boolean coercion (`true` to 1.0, `false` to 0.0). Selection includes the threshold,
then sorts by descending score and engine name. Wrong model, malformed data,
HTTP/transport errors and exhausted deadlines return no advice; deterministic
scope remains usable. F08 has one request, zero redirects, one MiB request/wire/
decoded limits and no retries. Its monotonic total deadline covers pool wait,
connection, headers, streaming, decode and score parsing. This intentionally
tightens source HTTPX per-phase timeout semantics into a finite total budget.
This HTTP deadline excludes advice-cache I/O: reads and writes have F07's own
separate operation bounds. As in the source, measured decision latency starts
after cache lookup and stops after score validation, before cache write. Cache
hits report zero. A route's total wall time can therefore include cache read,
provider attempt and cache write budgets. Prepared authorization and fresh F09
revalidation perform no provider or cache I/O.
Cookie/session ownership, client shutdown and transport implementation remain F08.
Numeric syntax retains Python Unicode decimal digits, valid underscores and
threshold NaN/infinity/clamp behavior, including canonical positive zero. Source
`overflow.json` observes a400digit positive timeout raising OverflowError, while
1e20milliseconds is accepted as1e17seconds. Native positive timeout values beyond
i64 disable the enhanced router instead of silently resetting to defaults; huge
negative values retain the50msfloor. This is an explicit bounded configuration
correction. TTL overflow is separate: the provider decision survives, with cache
write skipped for positive overflow and1second for huge negatives. The reference
TTL observations use actual private Valkey and SearchCache; source errors are
preserved. Score -0.0 and+0.0 compare equally before the engine-name tie break.

The Valkey key retains source-compatible SHA256 identity: model, cards version1
and Python float threshold produce the first16hex identity; stripped UTF-8 query,
sorted candidates and identity produce `jev-routing:v1:<sha256>`. Timeout, API key
and budget are absent from identity. Cached value bytes retain default-spaced
Python JSON and canonical floating point representation. Current eligibility is
rechecked on every request and after every cached/provider decision. Reads that
fail or contain malformed scores, duplicate/noncanonical/under-threshold selection
or mismatched candidates are misses. A cache write failure keeps the decision.
TTL defaults3600seconds, floors1; invalid or huge positive TTL skips writing. Huge negative TTL floors1. Cache operations use
F07's bounded single attempt and separate outcomes; no process-local decision
cache or shared state is added. F13 still owns SearchCache reconnect suppression
and the complete search-response cache; this standalone advice cache does not
claim to implement F13's retry contract.

`SOURCE_AUDIT.md` retains the original read-only findings and parent adjudication;
its statements about prerequisites describe that earlier audit, not current
acceptance. `hazards.json` preserves observed Python cached-NaN/duplicate admission
and automatic budget bypass. Native rejects these deliberately; it does not edit
those golden answers to manufacture parity. Ineligible advice is recorded as
rejected; admitted engines do not retain contradictory exclusion rows. Final
coverage and cost explanations reflect the final union once, rather than stale
baseline warnings or duplicated intermediate tradeoffs.

`cargo xtask check F12 --output target/rust-evidence/F12.json` verifies immutable
source/corpus anchors, exact named test inventory and completed nonignored tests,
then runs F09 qualification. Native tests use real loopback HTTP and a disposable
Valkey process with no production URL or global flush. Required Valkey executable:
`SLOPSEARX_TEST_VALKEY_SERVER=/path/to/valkey-server`. CI runs on both accepted
architectures and retains the report even on failure. Exact corpus mutation and
missing/renamed/ignored regression controls fail closed. The source fixtures are
synthetic observations, not provider recordings or relevance evaluations.

The isolated developer recapture uses pinned Python3.14.7/MCP2.3.0/FastMCP4.0.11:
`PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f12/oracle/export.py /private/tmp/f12-source-recapture.json`.
The exporter verifies pinned source blobs and refuses drift. Native gates never
invoke it. F01 inventories this temporary backend-coupled executable and G08 must
retire it before backend removal; it is not an optional-client exception.

Portal review: advice changes selected-engine provenance and explanations at the
shared policy layer, so downstream HTTP/MCP/portal composition must use the same
prepared decision. No browser contract is altered here: existing portal contract
tests remain required; native F12 asserts preview/provenance and no double call.
T02/T03 own full wire/HTML integration after native shared search composition.

Overflow recapture uses the same inventoried exporter with `--overflow` followed
by an output path. Set `SLOPSEARX_TEST_VALKEY_SERVER` to the disposable pinned
Valkey executable; there is no machine-specific fallback or production URL.
An unset or empty variable fails before starting a process. Its nine observations
were compared with a separately written source probe before freezing. Main source/hazard captures and earlier source pins
were not refreshed when this additive artifact was introduced.
