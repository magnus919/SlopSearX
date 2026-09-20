# EXP-005: Jev fusion over Brave candidates

Status: **completed; not supported on this corpus because the baseline was saturated**

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

## Result (2026-09-20)

The registered harness ran once from registration commit `962ecf4`, without
retries or changes. Brave returned status `ok` for all 12 queries and supplied
13-20 candidates per query. Every canonical official-documentation target was
already Brave's first result, so the baseline MRR@20 was the maximum possible
`1.0`.

All 12 Jev requests returned HTTP 200 with a complete typed answer from the
pinned `jev-1.13.0` model. Jev standalone and the fixed fusion also placed every
target first: MRR@20 `1.0`, zero regressions, and maximum rank drop zero. Jev
request latency was 235.3 ms mean and 451.2 ms p95/max by nearest rank. The run
used 40,789 input tokens and 4,190 output tokens, for an estimated input cost of
`$0.001713138` at the registered price.

The exact registered outcome is **not supported** because the required absolute
fusion gain was `+0.05` and observed gain was `0.0`. This is a ceiling effect:
fusion could not improve targets that Brave had already ranked first. The run
therefore establishes reliable acquisition, valid low-cost Jev judgments, and
no regression on these easy navigational queries, but it does not measure
incremental relevance value on ambiguous or difficult searches.

Do not tune this experiment after the result. A further experiment would need
independently labeled queries where the baseline has genuine ranking headroom,
preferably in a frozen redistributable corpus. That is new research, not a
reinterpretation or rerun of EXP-005.

Evidence: [summary](evidence/EXP-005/summary.json),
[sanitized per-query rows](evidence/EXP-005/rows.json),
[exact harness](evidence/EXP-005/harness.py.txt), and
[checksums](evidence/EXP-005/SHA256SUMS.txt). Snippet bodies and credentials
were not retained.
