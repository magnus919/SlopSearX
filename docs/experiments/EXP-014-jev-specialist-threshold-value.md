# EXP-014: Marginal value of Jev-selected specialist engines

**Status:** preregistered; no measurements collected

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
