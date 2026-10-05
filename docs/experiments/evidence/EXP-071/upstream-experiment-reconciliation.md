# Upstream release and experiment reconciliation

Read-only catch-up against SlopSearX main after PR #628 and release v0.6.0. A GPT-6 Luna reviewer independently audited the committed experiment documentation and shipped seams. No provider, search, deployment or runtime changes were made.

## What shipped

[v0.6.0](https://github.com/magnus919/SlopSearX/releases/tag/v0.6.0), released through [PR #574](https://github.com/magnus919/SlopSearX/pull/574), contains optional key-gated query-only Score reranking over the first 40 bounded cards, stable descending scores, and an unchanged unscored tail. Scholarly grouping precedes reranking. The service retains sensitive-provenance bypass, shared cache/singleflight, exact membership validation, one-second advice bounds, no retries and whole-incumbent fallback. [PR #518](https://github.com/magnus919/SlopSearX/pull/518) qualified strict provider parsing and transport bounds. See [reranking guide](../../../JEV_RERANKING.md), [reranker](../../../../slopsearx/rerank.py) and [service](../../../../slopsearx/service.py).

No caller-purpose/facet or full-pool product contract is implemented by those changes. Reuse these seams and safeguards; do not create another Jev client in GroktoCrawl X.

## Prior tests constrain the next proposal

| Experiments | Evidence | Consequence |
|---|---|---|
| EXP-034–036 | Whole-pool requests feasible; batching changed scores; held-out gain insufficient | Pool size and structural validity do not prove quality; avoid naive score fusion |
| EXP-038–040 | Absolute task-fit/source-selection Score; useful-source identity or research-grounding losses | Absolute-purpose Score is already tested, not a new mechanism |
| EXP-042–046 | Facet judgments, reservations and contextual replacements; coverage/quality conflicts or no swaps | Do not assume independent facet winners improve the complete selected set |
| EXP-056–059 | Counterfactual additions/losses/sufficiency; deadline and quality failures; EXP-059 development passed | Successful exposed development is insufficient without untouched confirmation |
| EXP-065 | Shared versus question-local unchanged EXP-039 rubric; local candidate rejected | Do not repeat the unchanged absolute rubric |
| EXP-066–071 | Binary bucketing, relative/count bands, support plus priority; quality failures and a composite deadline failure | Preserve negative results; no runtime adoption from these documentation PRs |

The ledger and individual reports remain authoritative. Recent PRs #606–628 register, qualify and retain experiments; they do not deliver a product selector.

## Avoided next run: descending saved Noul probability

An independent read-only diagnostic reordered EXP-066 saved Noul responses by descending probability, with canonical UTF-8 URL/title/snippet/id ties. This is a posthoc ordering change, not registered qualification or a revision of EXP-066.

- Reference A mean candidate minus W0 nDCG@10: -0.0053; bootstrap95 interval [-0.0628, 0.0549].
- Reference B mean: +0.0094; interval [-0.0298, 0.0514].
- Useful top-ten counts fell for A q1/q3/q6/q8 and B q1/q5/q6. All incumbent useful facets remained covered.
- Repeat/rotation overlap met .8. Present navigation targets stayed top-one; absent q10/q11 targets remain unmet recall.

The [standalone diagnostic](noul-order-diagnostic.py.txt) and [hash-bound output](noul-order-diagnostic.json) reproduce these numbers directly from the raw EXP-066 receipts and audited references. From the repository root, run `python3 docs/experiments/evidence/EXP-071/noul-order-diagnostic.py.txt --repo . --output DIAGNOSTIC_OUTPUT.json`. The output records full per-case orders, metrics, input and receipt hashes, and fixed bootstrap settings. Top-ten overlap means shared IDs divided by ten, not Jaccard similarity.

It fails both quality/retention gates and does not justify another identical registered run. Probabilities concern the binary proposition, not degree of usefulness or calibrated source correctness. The proposed generative listwise permutation is also unsuitable: the available Jev primitives do not return that output structure.

## Remaining production work

The [full acceptance checklist](../../complete-pool-production-acceptance.md) remains unchanged: choose a genuinely qualified frozen candidate, obtain untouched confirmation, then implement opt-in complete eligible-pool selection and original caller-context propagation through service/cache/singleflight/HTTP/MCP/portal/snapshots and X direct/research/sync/SSE/worker/replay paths. Ordinary behavior, sensitive guards, review and required CI must pass. No deployment or default changes are authorized by this audit.
