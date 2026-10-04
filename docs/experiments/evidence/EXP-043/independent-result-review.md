# EXP-043 result integrity review

Reviewed the saved EXP-043 prepared requests, receipts, raw response blobs, attempt ledger and analysis in the SlopSearX worktree. This was an offline, read-only audit: no provider/search calls, deployment, or edits. The pinned runner's own offline verifiers were used to check evidence consistency, not to dispatch requests.

## Integrity and registered-plan eligibility

The prepared manifest verifies against the registered source pins and body plan. It contains exactly 65 requests: 18 W0, 26 D, 21 F. `verify(..., require_refs=True)` returned `ok: true`, `ready_to_run: true`, 65 prepared requests, 65 valid receipts, and no receipt errors. The strict receipt reader reparsed and validated every raw response against its structured receipt; the attempt-history reader verified 65 started and 65 completed attempts. Response SHA-256 and byte counts, request identity, status, usage, and raw blobs were bound through the ledger.

All requests completed validly; the analysis records the expected model and complete answers. No retries, local overflow, timeout, or provider rejection appears in the attempt history. Candidate membership checks passed for all 13 pools and every registered ranking arm; constructed80 correctly contains 79 grouped representatives and was not counted as a natural relevance pool.

Actual limits were within the registered observed gates:

- Input: 1,236,249 total tokens; 46,145 maximum per request, below the 3,000,000 total and 64,000 per-call limits.
- Output: 108,840 tokens, below 160,000.
- Largest request bodies: W0 63,112 bytes; D 193,528; F 123,219. Largest state: 38,522 bytes. All are below registered byte bounds.
- Maximum remote HTTP duration: 626.5 ms per call; maximum D+F pair sum: 1,014.9 ms. Both pass their registered deadlines.

The calls were accepted and their reported input-token counts met the 64K per-call gate. The records do not expose state-token usage separately, so they cannot independently prove Jev's 32K state-plus-longest-question sublimit. This remains an observation about accepted requests, not a prospective token-fit guarantee.

## Frozen analysis outcome

The decision is `unsupported_for_selection`; this is consistent with the preregistered gates, even though primary relevance and operational checks passed.

The primary E−W0 nDCG@10 gains passed under both reused references: A mean +0.2157 (bootstrap lower 95% +0.1311), B mean +0.0677 (lower 95% +0.00415). Ordinal noninferiority E−D passed, stability passed, navigation passed, full membership passed, and operational limits passed.

Two registered quality conditions failed:

1. **No E−D coverage gain.** Under both references, the aggregate macro coverage of E equals D; the strict-improvement pool sets are empty for A and B. E−D coverage change is 0.0 for both aggregate reports. Thus the requirement to improve macro coverage under at least one reference is unmet.
2. **Research facet loss under reference B.** For the research pool, E retains 10 useful top-ten cards versus 9 in W0 and improves nDCG@10 by about 0.0332, but drops the useful `evidence_grounding` facet that W0 covered. The preregistered useful-facet preservation guard therefore fails for this pool/reference. E has the same top-ten as D, so it does not correct D's loss either.

These are failures of the candidate contract as registered, not invalid-data conditions. The evidence does not support adopting this candidate or claiming that the two-request design improves facet coverage. It does support the narrower observed statement that this exact bounded request plan was accepted and met the measured latency/token gates on this saved synthetic projection.

The corpus is a frozen `frozen_projection`, and both reference sets are reused assistant annotations rather than independent human gold. No fresh production capture or confirmation was performed; production readiness remains unestablished.
