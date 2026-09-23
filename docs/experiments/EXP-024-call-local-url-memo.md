# EXP-024: Memoize URL normalization within presence ranking

## Registration — 2026-09-23

Baseline: `c355830` main. EXP-002 rejected an empty-query shortcut, not this
candidate. Presence ranking normalizes the raw URL for every engine hit before
deduplicating. Cross-engine identical raw URLs can reuse an identical normalized
value within that ranking invocation. This is a distinct bounded candidate;
keep EXP-002's useful-effect thresholds rather than relaxing them.

Candidate: one local dict in `PresenceRanker.rank`, keyed by exact raw URL,
filled lazily with the unchanged `_normalise_url` result. Discard the dict at
return. No cross-request caching, no RRF modification, no URL semantics changes.
Inject the candidate method only into an isolated measurement process; do not
edit shipped code before evidence supports implementation.

Primary metric: paired mean elapsed milliseconds per complete
`SearchService.search` call on the equally weighted nine-cell corpus below.
Useful effect requires the lower bound of a 95% paired-block bootstrap interval
to exceed both 10% relative saving and 0.2 ms absolute saving. These thresholds
preserve EXP-002's practical significance rule. Do not promote a microbenchmark
win that misses the full-service criterion.

Corpus: three synthetic engines, 20 results each. Raw URL overlap across engines
is 0%, 50%, or 100% of each feed; query strings are absent, alternating, or on
all URLs (nine cells). Shared indices use `https://example.org/shared/{i}`;
engine-specific indices use `https://example.org/{engine}/{i}`. Query form is
`?utm_source=fixture&item={i}`. Three query-string families cross all three
overlap levels. Fresh result objects each call. Real shared service/presence
ranker, explicit engines, no cache/rate limiter/router/catalog/Jev. No network.

Per cell: ten warmup calls per arm, 40 paired blocks of 20 calls per arm.
Randomize block arm order with seed 20260923. Bootstrap 5,000 resamples of 40
paired blocks within each cell, equally weighted across cells, seed 20260924.
Record every block; no early stopping for promising timings or tuning. Units
are timing blocks, not independent users. Intervals only describe this machine
and fixture workload; no production or human task claim.

Guardrails: full serialized response equality on every call, excluding only
query_id and response_time_ms; zero errors; each cell's candidate mean <=105%
of baseline. Additional deterministic rank comparisons include malformed hosts,
fragments, repeated/blank query params, case variants and surrogate query values,
with duplicate cross-engine inputs and a per-engine budget. Compare all fields,
including provenance, positions and tiers, on fresh baseline/candidate copies.
Dictionary lifetime is one call and size is bounded by distinct input URLs;
this is not a measured memory-improvement claim.

Supported only if primary thresholds and all guardrails pass. Completed
comparison missing a point-estimate threshold or guardrail is not-supported;
point estimate clearing thresholds but intervals overlapping them is inconclusive.
Infrastructure failure is blocked. Stop on correctness failure or 45 minutes.

Reproduction: extract evidence/EXP-024/reproduce.md into `/private/tmp/exp024.py`;
run `PYTHONPATH="$PWD" /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp024.py`.
Retain code as inert Markdown, raw blocks, edge comparisons, environment,
exit/output logs and SHA256 manifest in `docs/experiments/evidence/EXP-024/`.
Commit this plan before implementing or measuring the injected candidate.

Portal impact: ranking is shared, so any promoted implementation requires
service, merger, provenance, HTTP/MCP and portal contracts plus required CI.
This experiment ships no behavior changes. Persist all outcomes in a docs PR;
only supported evidence can lead to an unmerged implementation PR.
