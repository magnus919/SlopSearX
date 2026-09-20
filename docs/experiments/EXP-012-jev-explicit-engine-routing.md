# EXP-012: Jev routing with explicit engine-bound questions

**Status:** advancement gate failed; further research supported

## Decision

Determine whether Jev can improve a three-engine search scope over the current
production resolver when every independent Noul explicitly identifies and
describes the engine it judges. This corrects the measurement defects found in
EXP-011; it does not authorize feature delivery.

## Hypothesis

For policy-eligible engines, one batched set of explicitly engine-bound Nouls
will select useful specialist engines more accurately than the existing
deterministic scope, while keeping the same three-engine dispatch bound.

## Frozen inputs and invariants

- Model: `jev-1.13.0`.
- Reuse the frozen EXP-011 cards and synthetic manifests without modification.
- Candidate thresholds: 0.45, 0.55, 0.65.
- Maximum selected engines: 3.
- Baseline: effective configuration, adapters, `CapabilityCatalog`,
  `QueryRouter`, and `ScopeResolver`; take its first three selected engines.
- Candidate pool: enabled, non-sensitive engines whose required authentication
  is configured and whose circuit is not open.
- Each question text includes engine name, display name, purpose, use guidance,
  categories, type, and cost class. The shared state contains only the query.
- Jev cannot grant authorization or restore an ineligible engine.
- Any Jev failure falls back to the production resolver.
- No user data is used.

## Development and freeze

Run the 12 EXP-011 development queries once. Select the threshold
lexicographically by highest macro family recall, highest F1, lowest mean
selection size, then higher threshold. Commit the selected threshold, exact
request-schema checksum, effective eligible-engine names, input checksums, and
development evidence before opening the held-out manifest.

## Held-out routing gates

Run the 30 EXP-011 evaluation queries and their paraphrases. The candidate
advances only if:

- recall >= 0.85;
- precision >= 0.65;
- macro family recall >= 0.80;
- F1 improves by >= 0.10 absolute over the production baseline;
- mean Jaccard agreement between query/paraphrase selections >= 0.90;
- no family recall regresses by > 0.10;
- zero ineligible or sensitive selections;
- at least 29/30 query pairs are valid.

No post-held-out tuning is allowed.

## Acquisition, only after routing passes

For each of the 30 evaluation queries, call the union of baseline and candidate
engines once (maximum six), then replay those identical engine feeds into both
three-engine views using the production presence merger. A strategy succeeds
when its top ten contain a result contributed by at least one labeled-useful
engine. Report reciprocal rank of the first such result, result counts,
per-engine outcomes, latency, and unique specialist wins.

Budgets:

- Jev: 5,000,000 input tokens absolute maximum.
- Brave: 30 expected calls, 36 absolute maximum. Up to six retries are allowed
  only for transport failure or an empty response; never retry a valid result.
- All non-Brave engines: 180 calls total, 30 per engine, no retries.
- One development pass, one held-out pass, one acquisition pass.

The harness stops before any call that would exceed a ceiling.

## Product decision

Recommend a product implementation experiment only if routing gates pass and:

- top-ten target coverage improves by >= 0.10 absolute (at least 3/30);
- specialists uniquely turn failure into success on >= 5 queries across >= 3
  families;
- no family loses more than one successful query;
- candidate dispatch count does not exceed baseline dispatch count;
- at least 29/30 acquisition rows are valid;
- Jev p95 latency <= 600 ms;
- all budgets and policy invariants pass.

Recommend **no** when valid evidence fails the effect or guardrail gates.
Report **inconclusive** for inadequate valid data or environmental failures.
Any changed prompt, threshold set, corpus, or retry is a new experiment.

## Evidence requirements

Commit sanitized per-query routing and acquisition rows, all aggregate metrics,
effective eligible engines, input and schema checksums, usage and call counters,
and the final recommendation. Never commit keys, authorization headers, or raw
third-party response bodies.

## Outcome

All 12 development calls and all 60 held-out query/paraphrase calls returned
valid typed responses. Development selected threshold 0.65. Held-out routing
improved F1 from 0.1905 for the production resolver baseline to 0.6286 and
macro family recall from 0.1889 to 0.8056. It also stayed within the policy
boundary and achieved 219 ms mean / 299 ms p95 Jev latency.

The candidate nevertheless failed three registered routing gates:

- recall was 0.7719 (required 0.85);
- precision was 0.5301 (required 0.65);
- query/paraphrase selection agreement was 0.6778 (required 0.90).

The remaining gates passed: macro family recall, F1 improvement, family
regression, valid pairs, and policy eligibility. Total Jev input was 300,427
tokens across development and evaluation, well below the 5,000,000 ceiling.

Because the offline gate failed, the harness did not run acquisition. Brave
calls: 0. Other search-engine calls: 0. The registered production-advancement
decision is **no for this exact routing design**. The research conclusion is
**supported for further research**: Jev produced a large accuracy gain, but the
all-engine, three-slot design had insufficient precision, recall, and
paraphrase stability for autonomous dispatch.

See `evidence/EXP-012/development-summary.json`,
`evidence/EXP-012/evaluation-summary.json`, and
`evidence/EXP-012/evaluation-rows.md`.
