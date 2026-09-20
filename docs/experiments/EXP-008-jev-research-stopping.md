# EXP-008: Jev research sufficiency and stopping

Status: **completed; not supported**

## Hypothesis

Jev can identify when a bounded evidence set satisfies every explicitly listed
research requirement without creating unsafe premature stops.

## Frozen method and gates

The harness contains 16 independently labeled sufficient/insufficient evidence
sets. Each receives one Noul probability; `p >= 0.5` means stop/sufficient.

Supported-for-further-research requires: 16/16 valid calls; **zero false stops**
on insufficient cases; sufficient-case recall at least `0.75`; overall accuracy
at least `0.80`; p95 at most 1,500 ms; and cost below `$0.01`.

This experiment cannot establish factual truth. A future stopping function
would remain subordinate to deterministic required-facet, budget, deadline,
policy, and provenance checks, with an `uncertain -> continue` fallback.

## Result

All 16 calls were valid and there were zero unsafe false stops, but Jev rejected
four of eight sufficient evidence sets. Accuracy was `0.75` and sufficient-case
recall was `0.50`, below the registered gates. The run is **not supported**.

See the [suite results](JEV-FUNCTION-SUITE-RESULTS.md) and
[evidence](evidence/EXP-008/summary.json).
