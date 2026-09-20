# EXP-009: Jev engine-plan selection

Status: **completed; supported for further research**

## Hypothesis

Given only policy-safe plans generated in advance, Jev selects the labeled
best engine family for ambiguous queries more accurately than the current
first-match keyword router.

## Frozen method and gates

The harness contains 15 labeled queries and eight fixed plans: web, code,
science, news, social, reference, historical, and jobs. Jev scores every plan
with the same Noul question; highest probability wins with declared plan order
as tie-breaker. The current `QueryRouter.match_topic()` is the baseline, with
no match mapped to `web`.

Supported-for-further-research requires: 15/15 valid calls; accuracy at least
`0.80`; accuracy at least `0.10` above the deterministic baseline; zero outputs
outside the offered non-sensitive plans; p95 at most 1,500 ms; and cost below
`$0.01`.

Jev never receives or chooses sensitive plans, and could never bypass the
shared policy gate or explicit engine/category selections.

## Result

All 15 calls were valid. Jev selected the labeled plan for 12/15 queries
(`0.80`) versus 7/15 (`0.4667`) for the keyword router, an absolute gain of
`+0.3333`, with no output outside the offered plans. The run is **supported for
further research**.

See the [suite results](JEV-FUNCTION-SUITE-RESULTS.md) and
[evidence](evidence/EXP-009/summary.json).
