# EXP-005: Jev fusion over Brave candidates

Status: **registered; not yet executed**

Registered: 2026-09-20

Parent research spike: [#389](https://github.com/magnus919/SlopSearX/issues/389)

Predecessor: [EXP-004](EXP-004-jev-official-doc-rerank.md), which was
inconclusive because its unauthenticated acquisition engines returned no usable
candidate corpus.

## Question

When Brave Search reliably supplies candidates for official-documentation
queries, does a fixed fusion of Brave rank and Jev relevance probability improve
the first relevant result without unacceptable regressions, latency, or cost?

This is an exploratory confirmation experiment. It is not an authorization to
implement a Jev client, change ranking, or enable external processing.

## Pre-run acquisition check

Before registration, one non-measured Brave request for `Python asyncio
TaskGroup documentation` returned status `ok` with 10 results. No Jev request
was made. This check establishes only that authenticated acquisition is
available in the execution environment.

## Frozen corpus and labels

Use the same 12 queries and canonical official-documentation URL matchers
registered in EXP-004. Each matcher is an independently specified binary label:
the matching official page is relevant and all other URLs are non-targets for
the MRR calculation.

Candidates are the first 20 Brave web results returned once per query during
the registered run. There are no retries and no post-run query, target, prompt,
candidate, or threshold changes. Because this is a live provider corpus rather
than a redistributable frozen corpus, the result can support another research
step but cannot by itself close the rights/provenance gap identified in #389.

## Systems compared

1. **Brave baseline:** untouched provider order.
2. **Jev standalone (diagnostic only):** descending Jev probability, with Brave
   rank as the first tie-breaker and normalized URL as the second.
3. **Fixed fusion (candidate):** descending
   `0.70 * reciprocal_rank + 0.30 * jev_probability`, where
   `reciprocal_rank = 1 / brave_rank`. Brave rank and normalized URL break ties.

Jev uses the pinned model `jev-1.13.0`. Each query makes exactly one request
containing the query and, for each candidate, its index, title, normalized URL,
and at most 500 snippet characters. Each candidate receives the same Noul
question asking whether it is a direct, useful result with strong preference
for the official primary documentation page.

## Primary metric and decision rule

Primary metric: MRR@20 for the canonical target URL.

The result is **supported for further research** only if all of these hold:

- at least 10 of 12 targets occur in the Brave candidate sets;
- at least 11 of 12 Jev calls are valid HTTP 200 responses from
  `jev-1.13.0`, with one finite probability in `[0, 1]` per candidate;
- fusion MRR@20 exceeds Brave MRR@20 by at least `0.05` absolute;
- among queries whose target is present, fusion worsens no more than two target
  ranks and no single target falls by more than three positions;
- Jev request p95 latency by nearest-rank percentile is at most 1,500 ms;
- estimated Jev input cost is below `$0.01` at the registered
  `$0.042 / 1M input tokens` price.

If either coverage or valid-call minimum fails, the result is
**inconclusive**. If both prerequisites pass but any effect, regression,
latency, or cost guardrail fails, the result is **not supported**.

The Jev-standalone metric is diagnostic and cannot determine the outcome.

## Evidence and privacy boundary

Retain:

- aggregate metrics and per-query ranks;
- engine status, result count, and latency;
- candidate URL, title, Brave rank, snippet length and SHA-256, Jev probability,
  and fused score;
- resolved model, response status, token usage, Jev latency, exact harness, and
  checksums.

Do not retain snippet bodies, authorization headers, API keys, raw provider
responses, or `.env`. Do not send sensitive-engine output, private research
state, or fetched page bodies.

## Stop rule

Run the registered harness once. Do not retry, tune weights, edit prompts,
replace queries, change URL matchers, or repair individual results after seeing
measurements. Any follow-up is a new numbered experiment.

## Product interpretation boundary

Even a supported result would establish only relevance potential on a narrow
English official-documentation task. It would not establish user demand,
privacy acceptance, multilingual quality, production reliability, cache and
snapshot semantics, portal disclosure, or permission to ship. Those open gaps
remain in the #389 research report.
