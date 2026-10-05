# EXP-075: single-transaction dual-Score rank fusion

Status: completed; development rejected. The prospective protocol below is
retained unchanged as the registration record. The qualified producer completed
44 calls; the original evaluator failed before metrics. Separately source-bound
post-run verification retained that failure and established rejection under the
corrected evaluator. See the [completed readout](evidence/EXP-075/completed/readout.md)
and [bounded offline diagnosis](evidence/EXP-075/q4-instability-diagnosis.md).
Issue: #516. Baseline source: `1b286066bde749fed32a0d2033586b9e59d18ee6`.
The machine-readable protocol is `evidence/EXP-075/protocol.json`.

## Consequential question

Can one Jev transaction return separate query-relevance and caller-purpose
judgments, whose fixed rank fusion improves complete-pool selection while fitting
the ordinary one-second reranking phase? This is a replacement for the existing
query-only model transaction on explicit-purpose requests, not a serial second
model call added afterward. Ordinary requests remain unchanged.

The running service currently has no caller-purpose/facet fields. Here the
frozen evaluator supplies each original case's explicit purpose and ordered
facets; it never infers them from a search query. This is a proposed input
contract, not an already reachable deployed feature. Adding and propagating those
fields through every caller, transport and state identity remains required by
the full production-acceptance checklist after development qualification.

EXP-073 and EXP-074 remain terminal incomplete comparisons. EXP-074 passed
capacity and five research operations before its combined two-call deadline
failed. No prefix quality result is available. This new mechanism changes the
request context and phase boundary because it ships one transaction; it does not
relax, replay or regrade either failed two-transaction study.

## Frozen mechanism and comparator

For each operation, obtain an independent, byte-identical fresh W0 evaluation
control using the existing first-40 query-only request and native tail. That
control is necessary for the unchanged quality contrast. It is outside the
replacement candidate phase and is not an extra production dependency.

The candidate uses one shared state: the W0 query/candidates state plus an
explicit `caller_context` object containing original purpose and ordered facets.
It contains at most 40 query Score questions, using existing objects/criteria,
and one purpose Score question for every eligible card, up to 80. Namespaced
answer keys map back to supplied IDs. Query instructions still refer to the
unchanged local IDs in the prefix state; purpose instructions refer explicitly
to `caller_context` and include their own complete visible card. The purpose
instruction prefix and all remaining question/criterion text are frozen in the
protocol or pinned EXP-072 contract.

These are two separate score populations and two full permutations. Query scores
sort descending with stable original prefix-position ties (`c0` through `c39`,
or the actual shorter prefix), then append the unchanged native tail.
Exact equal-weight reciprocal rank
fusion uses k=60 and canonical UTF-8 ties. No raw-score addition, revised weights,
confidence-based tuning, one-score proxy or dropped tail is allowed.

Shared state is deliberate and changes the estimand: contextual isolation and
unchanged individual judgments are unproven. The independent W0 comparator
prevents a changed query component from silently redefining the baseline.

## Maximum request and provider uncertainty

The [API reference](https://docs.typesafe.ai/api) supports structured per-question
instructions and Score rubrics with two to ten levels. The
[Score guidance](https://docs.typesafe.ai/primitives/score) describes batching
separate judgments; its parallel-execution claims do not prove acceptance or
quality for this 120-question maximum. The
[model reference](https://docs.typesafe.ai/models), checked October 5, reports
64k combined input and 32k for state plus the longest question. No published
question-map maximum or verified local tokenizer is assumed.

Qualify the exact complete 80-card/120-question synthetic shape before the
research schedule. Preserve byte envelopes, context checks available without an
assumed tokenizer, raw usage and provider context rejection. A rejection is
terminal; never trim cards, split the pool or retry it to manufacture a pass.

The proposed purpose-capable wrapper caps its own request at 384,000 bytes
and shared state at 128,000 bytes; provider acceptance remains unproven.
The existing query-only provider and independent
W0 control retain their 128,000-byte request limit. The larger capability is part
of this new candidate mechanism; capacity within it does not prove fit under the
legacy limit. Ordinary requests keep the legacy limits. Any eventual opt-in
implementation must bind the new limits in provider identity and prove resource,
timeout and compatibility behavior before release. This study changes no runtime
limit or existing provider interface.

## Registered measurement and stopping

Reuse the exposed 21-operation schedule and frozen independent A/B references.
There are at most 44 serial calls: capacity control/candidate, then 21 independent
control/candidate pairs. No new searches, no retries, no parallel provider calls.
Brave remains 8/10 used. Use a fresh in-process async client per call.

On candidate failure, preserve the complete ordinary incumbent from before Jev
(the native/provenance order supplied by this corpus). The independently fetched
W0 control remains comparison evidence; it is not an extra model call available
to the production replacement. Never salvage only one valid component from an
invalid composite response. This makes failure behavior match the service seam.

Measure the control's durable phase and candidate's durable phase separately,
each bounded by one second. Candidate timing starts before constructing,
canonicalizing, namespacing and serializing its actual composite request.
It includes that work, its pending receipt, its sole
exchange, both-family parsing, exact fusion and final receipt fsync. An admission
preview may verify shape/size before key access, but the measured phase must
reconstruct and byte-match the request rather than reuse the prepared body.
Also retain combined paired wall time; do not present that as the
candidate phase. Any failure stops the schedule and preserves all uninvoked
operations and unknown usage. This prospective accounting follows the actual
one-call replacement mechanism; EXP-073/074 accounting is unchanged.

Only a complete schedule can be graded. Preserve both-reference mean nDCG gain
at least .05 with positive bootstrap lower bound; per-case loss no worse than
-.03; useful-count/facet retention; repeat/rotate overlap at least .8; present
navigation targets at rank one; exact complete membership and resource bounds.
Absent q10/q11 targets remain unmet acquisition recall and confer no adoption
credit. The one natural long pool cannot establish general tail benefit.

Commit registration before implementing the candidate or measuring it. Commit
and independently review an exact source/dependency qualification before one
live admission. Acquire a durable exclusive study lease before DNS and lazy key
access, retain it after setup failure, and retain the earlier EXP-074 offline
incident without claiming it had zero attempts.

## Delivery boundary

This study can establish development evidence only. Genuinely untouched new
tasks and new pools, full service/cache/singleflight/HTTP/MCP/portal compatibility,
GroktoCrawl X direct/research/sync/SSE/worker/replay propagation, review and exact
CI/merge evidence remain mandatory under
`complete-pool-production-acceptance.md`. No runtime, default, deployment or
Hermes changes are authorized by a capacity pass or this registration.
