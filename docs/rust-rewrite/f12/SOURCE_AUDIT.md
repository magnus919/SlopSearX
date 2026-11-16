# F12 #891 source audit (read-only; implementation remains blocked on F09 acceptance)

Audit revision: published F09 head `3506f3c46281ea89cc64f69f37c8720004633ed8`,
Python source unchanged from fetched main `1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`.
F08/F07 are accepted; PR970/F09 is still under review. No F12 source edits,
live provider calls or deployment. Actual offline probes are preserved in
`/private/tmp/f12-source-probes.py` and `/private/tmp/f12-source-probes.json`.

## Current contract

- `slopsearx/jev.py:186-294`: API-key presence after Python strip is the only
  enablement switch. Missing/empty TYPESAFE_API_KEY returns None. Threshold
  defaults0.65 and clamps[0,1]; timeout defaults1000ms and floors50ms. Either
  parsing ValueError resets both defaults. Capture NaN/infinity and Python numeric
  grammar explicitly rather than assuming Rust parse/clamp parity.
- Candidates are current active specialists from audited routing roles, excluding
  sensitive names and circuit-open adapters; known catalog entries additionally
  exclude disabled or required-auth/unconfigured engines. Missing catalog telemetry
  keeps the candidate. Empty candidates return an empty zero-latency decision with
  zero requests. Reuse accepted F06 routing cards; do not duplicate card ownership.
- One sorted question map is sent in one POST to TypeSafe `/v1/systemone`, model
  `jev-1.13.0`, Bearer authorization. Disclosure is raw user query plus engine ID,
  display name, purpose/use_when and effective categories; no result payloads.
  Preserve exact request payload and counts. F08 pooled transport must not add
  default POST retries: source has one request. Source scalar HTTPX timeouts are
  per-phase; any native total-deadline tightening needs explicit documentation.
- Wrong model, missing/malformed/nonfinite/out-of-[0,1] provider scores, HTTP or
  transport failure return None atomically. Scores meeting threshold are ordered
  by descending score then engine name. Source accepts JSON booleans as numbers
  (True=>1.0): preserve its capture and review any stricter native change.
- Cache identity is SHA256(sorted-key default-spaced Python JSON of model,
  CARD_VERSION='1', threshold), first16hex. Decision key `jev-routing:v1:` hashes
  sorted-key/non-ASCII-escaped/default-spaced JSON containing stripped query,
  sorted candidates and identity. API key/timeout/cost budget are absent. Native
  source-compatible bytes and Python float formatting need dedicated goldens.
- Cache hit requires exactly current candidate score keys and returned engines
  belonging to those keys; latency0. Cache read failure falls through to one
  provider request. Cache write failure preserves the decision. TTL default3600,
  floor1; invalid TTL skips the cache write inside the caught write block rather
  than substituting a default. Use real disposable Valkey for TTL/store evidence.
- `service.py:705-731`: advice runs only with no truthy raw engines/categories and
  media_type=None, including scope preview. Explicit/raw whitespace lists suppress
  advice even if normalized selection is empty. Advice only appends to deterministic
  scope, retaining general engines; provenance stores additions and their scores,
  adding `+jev specialists` only when additions exist. Reusing resolved scope in
  search avoids a second billable preview call. Keyless/failure leaves baseline.

## Confirmed source hazards to adjudicate before native implementation

1. **Automatic cost/count bounds are bypassed by appended advice.** Offline source
   probe config max_engines1/max_cost_classfree selects brave, excludes paid pubmed,
   then appends pubmed from a stub: final selected[brave,pubmed] while the exclusion
   still says pubmed exceeds cost. Native optional advice must remain constrained
   by current policy/budget; retain this exact source observation and explicitly
   review the conservative correction. Do not route advice through an explicit
   engine proposal: F09 intentionally bypasses automatic routing for explicit lists.
2. **Cache score/selection validation is weaker than provider validation.** An
   injected cache `{scores:{pubmed:'nan'},engines:[pubmed,pubmed]}` is accepted with
   NaN and duplicate admissions, zero provider calls. Cached scores also need not
   meet threshold or canonical order. Candidate-set validation does prevent cache
   output from naming an excluded sensitive engine. Native malformed-score/duplicate
   correction must be declared, not hidden by changing source golden answers.
3. **Boolean provider score accepted.** MockTransport True admits pubmed with1.0;
  1.01 and missing score return None; each performs exactly one mock request.
   Scope issue says malformed/out-of-range fallback: establish whether booleans are
   deliberately preserved or conservatively rejected before coding.

## Evidence/implementation checklist once prerequisites are accepted

`tests/test_jev.py` covers registry-card completeness, key-only enablement, keyless/
enhanced CI matrix with MockTransport, one batched request/cache reuse, baseline
preservation, explicit-scope no-call, failure fallback, snapshot provenance and
HTTP/portal exposure. `tests/test_routing_eval.py` supplies deterministic eligibility,
cost/count/coverage invariants; it does not currently exercise post-Jev merging.
Add offline captured malformed/cache/deadline/tie/threshold cases with real request
counts and exact cache bytes. Revalidate live eligibility after cache/provider
returns through accepted F09 and at execution; avoid stale advice admitting revoked
sources. Native executable availability is separate from descriptor metadata.
Preserve source false/inconclusive evidence and describe deliberate budget/cache
hardening. No relevance or live-provider quality claim follows from these fixtures.
Native `cargo xtask check F12`, both-architecture CI/report wiring, disposable
Valkey gates, source identity and G08 oracle retirement are required. F12 must also
replace the validation-tool's current unimplemented F12 negative with another
actually unimplemented plan item while retaining that fail-closed test.

## Parent adjudication (conditional on F09 acceptance; no implementation yet)

Enforce current automatic eligibility/cost/count bounds after all advice; never
convert advice into an explicit-scope bypass. Invalid/nonfinite cache scores and
duplicate/noncanonical/under-threshold cache selections become cache misses with
bounded normal fallback, without blanket normalization. Preserve source Boolean
provider coercion True=>1.0/False=>0.0 with explicit source cases. Retain the
hazard observations as intentional-difference evidence. F09 acceptance remains a
hard blocker before implementation.
