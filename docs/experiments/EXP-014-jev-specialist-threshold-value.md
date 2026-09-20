# EXP-014: Marginal value of Jev-selected specialist engines

**Status:** supported; threshold 0.65 selected

## Question

At what Jev score does one more specialist-engine request add enough evidence
to justify its latency, quota, and result-space cost?

## Design

This is a fixed-curve experiment, not a threshold-tuning loop. Thirty new
synthetic queries are labeled with:

- `essential`: the specialist is the direct authoritative source;
- `useful`: the specialist can add distinct secondary evidence;
- all other eligible specialists: irrelevant for scoring.

Jev evaluates only specialist engines. Brave, Google, and DuckDuckGo remain an
intact deterministic general base and are never scored. Wikipedia, Stack
Exchange, Reddit, and Hacker News remain outside Jev's specialist surface.

Evaluate thresholds 0.45 through 0.65 inclusive in 0.025 increments. There is
no engine-count cap: every specialist at or above a threshold is selected.
One Jev response per query supplies the complete curve.

## Offline measurements

For every threshold report:

- essential and useful recall;
- selection precision where essential and useful are both relevant;
- mean specialist calls per query;
- broad-query abstention;
- weighted label utility: `2 * essential + 1 * useful - 1 * irrelevant`;
- marginal changes relative to the next-higher threshold.

## Acquisition and replay

Acquire once using the union selected at threshold 0.45 plus the intact general
base. Replay those identical feeds for every higher threshold. Thirty queries
means 30 expected Brave calls (36 absolute maximum). Non-Brave calls are capped
at 600 total and 30 per engine, with no relevance-based per-query cap.

For each threshold compare two top-ten views:

1. current production presence merging;
2. specialist-aware presentation: take the first result from each selected
   specialist in descending Jev-score order, deduplicate, then fill remaining
   top-ten positions from the ordinary merged general-plus-specialist results.

The presentation rule does not limit engines; the natural ten-result view may
show at most ten sources. Report, per additional specialist request:

- response yield: the engine returned at least one result;
- visible yield: the engine contributed to the specialist-aware top ten;
- labeled visible yield: it contributed and was essential or useful;
- essential target coverage and reciprocal rank;
- latency/status distribution.

## Registered interpretation

A threshold is operationally viable only if it achieves all of:

- essential recall >= 0.90;
- precision >= 0.75;
- broad-query abstention >= 4/5;
- labeled visible yield >= 0.40;
- at least 29/30 usable acquisition rows;
- Jev p95 <= 600 ms;
- all budgets and policy invariants pass.

Among viable thresholds, recommend the one with the highest essential
coverage@10, then highest labeled visible yield, then highest weighted utility
per specialist request, then fewer requests, then the higher threshold. If no
threshold is viable, recommend no threshold and identify the limiting layer.

The result is synthetic research evidence, not authorization to implement.
No post-hoc threshold is called confirmatory outside this frozen curve.

## Outcome

All 30 Jev calls and all 30 acquisition rows were valid. The run used 84,209
Jev input tokens, 30 Brave calls, and 99 non-Brave calls. Jev p95 latency was
297 ms. Thresholds 0.55 through 0.65 passed every registered viability gate;
the preregistered tie-break selected **0.65**.

At 0.65:

- essential recall: 0.9231;
- precision: 0.9688;
- useful-secondary recall: 0.5833;
- broad-query abstention: 5/5;
- mean specialist requests: 1.067 per query (32 total);
- labeled visible yield: 0.4375;
- specialist-aware essential coverage@10: 0.36, versus 0.08 with the current
  tier-first merger over the same captured feeds.

The curve reveals a useful operating band rather than a magical point. Moving
from 0.45 to 0.55 removed four requests, increased weighted utility from 54 to
56, preserved 0.9615 essential recall and 0.36 live coverage, and raised
labeled visible yield from 0.359 to 0.40. Moving from 0.55 to 0.65 removed three
more requests and improved yield per request, but reduced essential recall to
0.9231 and useful recall from 0.6667 to 0.5833. The registered rule preferred
0.65 because observed essential coverage was unchanged and per-request value
was higher. A risk-averse operator could reasonably choose 0.55 to retain more
potentially useful evidence; that is an interpretation, not a second
confirmatory winner.

Upstream availability remains the dominant ceiling: only 41-44% of selected
specialist calls returned results across the viable band. Specialist-aware
presentation made every returned selected specialist visible, demonstrating
that the ranking bottleneck identified by EXP-013 is addressable, but it cannot
recover empty, error, timeout, or rate-limited feeds.

See `evidence/EXP-014/summary.json` and `evidence/EXP-014/rows.md`.
