# TypeSafe Jev research spike

Date: 2026-09-20
Issue: [#389](https://github.com/magnus919/SlopSearX/issues/389)
Decision: **learn more; do not propose implementation yet**

## Executive finding

Jev is technically plausible as an optional post-retrieval decision signal,
especially when fused with deterministic ranking. It is not a search engine and
does not belong behind the engine-adapter interface unless it independently
retrieves results. The current evidence does not justify adding it to SlopSearX:

- the live API smoke test confirmed the documented typed response contract;
- independent public evaluation suggests Jev is more promising as a fused
  second signal than as a standalone replacement ranker;
- SlopSearX EXP-004 was inconclusive because the execution environment returned
  no usable candidate corpus, so no SlopSearX relevance effect was measured;
- ordinary accounts do not have documented zero-data retention, and sending
  queries, URLs, titles, and snippets would be a material external-processing
  boundary;
- no affected-user interviews or task study establish demand or acceptable
  latency/privacy tradeoffs.

The next defensible step, if desired, is a separate preregistered offline
experiment over a frozen, licensed, independently judged candidate corpus. It
should test deterministic fusion of Jev probabilities with RRF, not replace the
baseline ordering outright. No production client, configuration, schema, UI, or
implementation issue is warranted from this spike.

## Evidence classification

| Evidence | Classification | What it establishes | What it does not establish |
| --- | --- | --- | --- |
| TypeSafe API/docs/legal pages, accessed 2026-09-20 | Provider primary source | Current documented contract, price, limits, account terms and data statements | Independent accuracy, durable pricing, or SlopSearX fit |
| One pre-registration synthetic smoke call | Direct observation | Authentication worked; `jev-latest` resolved to `jev-1.13.0`; typed outputs and usage were returned in 466 ms | Search relevance, reliability distribution, or user value |
| EXP-004 fixed run | SlopSearX experiment | Current execution environment could not acquire the registered evaluation pool; one non-empty Jev call was valid | Any Jev relevance lift or regression |
| Public third-party benchmark repositories | Independent but unverified here | Credible prior evidence and experiment-design warnings | A reproduced SlopSearX result or endorsement |
| Maintainer request in #389 | Authority-holder input | Explore potential value without authorizing delivery | Broad user demand or acceptance of external processing |

## Stakeholder map and missing voices

| Role | Present evidence | Key concern | Status |
| --- | --- | --- | --- |
| Authority holder / maintainer | Issue #389 and this research authorization | Is optional value worth investigating without changing the no-key product? | Consulted |
| Implementation knower | Current SlopSearX code/contracts | Shared pipeline, cache identity, deadlines, policy, snapshots, portal | Audited |
| Provider / data processor | TypeSafe documentation and agreements | API behavior, price, data rights, retention, service evolution | Documents reviewed; no direct clarification |
| Affected portal/API/MCP users | No interviews or task observations | Value, acceptable latency, external-data expectations, disclosure | Missing |
| Operator/security/privacy reviewer | Repository rules and provider documents only | Secret handling, DPA/ZDR, regional processing, auditability | Missing |

Product discovery is therefore incomplete for a delivery decision. Technical
research may continue, but user-value and privacy acceptance remain open.

## Provider contract and legal/operational evidence

All facts below were read on 2026-09-20 and may change.

| Topic | Verified current statement | Source and implication |
| --- | --- | --- |
| Endpoint | `POST https://api.typesafe.ai/v1/systemone`, bearer authentication, JSON body with `state`, `model`, and `questions` | [API reference](https://docs.typesafe.ai/api). A dedicated client seam is straightforward. |
| Typed primitives | Noul returns a yes probability; Choice returns a declared option and distribution; Score returns a rubric-weighted score | [API reference](https://docs.typesafe.ai/api). Schema validity is not semantic correctness. |
| Model | `jev-1.13.0`; `jev-latest` currently resolves to it, but aliases move | [Models](https://docs.typesafe.ai/models). Any evaluated deployment must pin a version and expose the resolved version. |
| Price and limits | `$0.042` per million input tokens; output free; 250,000 tokens/second, 1,200 requests/minute; 64k request context and 32k state-plus-longest-question constraint | [Models](https://docs.typesafe.ai/models). Limits are documented as dynamic. |
| Inputs | Text only; state may be string, object, or array | [Models](https://docs.typesafe.ai/models) and [State](https://docs.typesafe.ai/concepts/state). SlopSearX would send textual query/result material. |
| Errors | 401, 422, 429, and 529 are documented; 429/529 should use exponential backoff | [API reference](https://docs.typesafe.ai/api). Search must fail open and keep retries inside a stricter overall deadline. |
| Language | English is described as strongest; other languages require workload testing | [Models](https://docs.typesafe.ai/models). No multilingual quality claim is available. |
| Training | Provider states requests/responses are not used to train model weights without consent | [Models](https://docs.typesafe.ai/models), [Privacy](https://typesafe.ai/legal/privacy-policy), and MCA section 4.1. This is narrower than zero retention. |
| Ordinary retention | Privacy policy and DPA use purpose/necessity-based retention; DPA Schedule I says data is retained as long as necessary for processing and law | [Privacy policy](https://typesafe.ai/legal/privacy-policy) and [DPA](https://typesafe.ai/legal/data-processing). No fixed ordinary-account deletion window was found. |
| ZDR | Zero-data retention is offered for enterprise customers | [Legal reference](https://docs.typesafe.ai/legal). It is not documented as the default for the tested account. |
| Application integration | MCA permits integrating the API into customer applications for end users and assigns provider rights in Output to the customer | [Master Customer Agreement](https://typesafe.ai/legal/mca), sections 2.1-2.2 and 4.2. This supports an integration shape, subject to the order and remaining terms. |
| Restrictions | MCA prohibits offering the service standalone, model distillation, imitation training, reverse engineering, and exceeding limits | [Master Customer Agreement](https://typesafe.ai/legal/mca), section 2.3. A search enhancement must remain part of SlopSearX and must not create a Jev proxy. |
| Input responsibility | Customer must have rights, notices, consents, and permissions for Input | [Master Customer Agreement](https://typesafe.ai/legal/mca), section 5. Search snippets and personal/sensitive queries need an explicit data decision. |
| Telemetry | Provider may process generated telemetry without restriction, including hashes, statistics, classifications, metrics, and learnings | [Master Customer Agreement](https://typesafe.ai/legal/mca), section 4.3. Operator disclosure must not imply that only raw prompt retention matters. |
| Compatibility | Provider may update the service and acknowledges API incompatibility is possible, with commercially reasonable notice for material adverse changes | [Master Customer Agreement](https://typesafe.ai/legal/mca), section 2.5. Contract tests and versioned fallbacks would be mandatory. |
| Accuracy/warranty | MCA says output may be inaccurate or erroneous and must be independently evaluated | [Master Customer Agreement](https://typesafe.ai/legal/mca), section 9.3. Jev probabilities cannot be labeled factual confidence. |

Caching and redistribution of derived scores appears compatible with provider
assignment of Output, but the applicable order and any account-specific terms
were not inspected. That remains a legal/product gap, not a confirmed right.

## Independent prior evidence

The strongest relevant public result found was
[`jev-search-rerank-eval`](https://github.com/zhuyansen/jev-search-rerank-eval),
which reports 164 English/Chinese/mixed queries and 9,831 judged pairs. Its
reported standalone Jev rerank over a strong embedding shortlist changed
NDCG@10 by only +0.012 with a confidence interval spanning zero, while RRF
fusion of the embedding and Jev signals improved NDCG@10 by +0.090 with a
positive interval. It also demonstrates judge circularity: results look better
when Jev participates in its own labels. This was not reproduced during this
spike, but it materially changes the best next hypothesis from “Jev replaces
ranking” to “Jev may add a complementary feature to deterministic fusion.”

[`jev-rerank-bench`](https://github.com/anessbelbati/jev-rerank-bench) reports a
broad reranker comparison with saved evidence and cautions that Jev and Cohere
were essentially tied under one aggregation while other controls varied by
dataset and prompt shape. [`jev-search`](https://github.com/superagents-lab/jev-search)
is useful implementation prior art: it sends queries, titles and snippets to a
Jev provider, ranks by model judgment plus agreement/original rank, exposes
failures, and explicitly warns that relevance percentages are judgments rather
than verified accuracy. Neither project proves SlopSearX value.

## Architecture alternatives

| Alternative | Potential value | External data and latency | Contract impact | Assessment |
| --- | --- | --- | --- | --- |
| 1. Do nothing | Preserve deterministic, private-by-deployment behavior | None | None | Current decision until better evidence exists |
| 2. Offline/evaluation only | Test hypotheses without production dependency | Frozen sanitized corpus; no user traffic | Documentation/evidence only | **Recommended next step** |
| 3. Optional post-merge reranking | Improve top-result ordering | Query, URLs/titles/snippets; one bounded call after acquisition | Ranking/cache/snapshot/provenance/portal changes | Plausible, but unproven and highest visible compatibility risk |
| 4. Additive annotations only | Let clients inspect relevance/type signals without changing order | Same content disclosure and call latency | New derived view/fields; lower order risk | Useful experiment seam, but may add noise without a user task |
| 5. Query-intent/routing assistance | Choose engines/categories before dispatch | Query and routing catalog; adds pre-dispatch latency | Shared resolver, cache identity, policy and explainability | Duplicates existing deterministic routing concerns; lower priority |
| 6. Consuming workflow outside core | Specialized research clients decide when to pay/send data | Controlled by consumer | No default SlopSearX behavior change | Strong boundary for early use; preserves standalone model independence |

Jev should not be an engine adapter: it does not retrieve an independent result
set. The least-wrong future seam is a derived, optional post-retrieval processor
or an external consuming workflow. An integration must not be enabled merely by
the presence of a secret; credential availability and behavior enablement are
separate operator decisions.

## Data flow and fail-open boundary

The minimal plausible post-merge flow is:

```text
existing engine dispatch
  -> deterministic deduplication and RRF/presence
  -> bounded candidate projection (query, title, URL host/path, snippet)
  -> TypeSafe API (optional, explicit enablement, pinned model, hard deadline)
  -> validated probabilities
  -> deterministic fusion and explanation
  -> canonical response/cache keyed by effective model + rubric version
```

No key, disabled configuration, invalid credential, timeout, 429/529, malformed
body, missing answer, unknown version, or low-discrimination output must return
the unchanged deterministic order. The API key must never enter cache identity,
logs, telemetry, snapshots, diagnostics, or payloads. Sensitive-engine output,
personal queries, private research state, and fetched page bodies should be
excluded unless a separate explicit data policy authorizes them.

If model output affects canonical ordering, the cache identity needs effective
strategy, pinned/resolved model, rubric/config version, candidate projection
version, and enablement state. Snapshots must retain enough provenance to explain
that a derived order used an external judgment without presenting probability as
truth, authority, SafeSearch enforcement, corroboration, or research sufficiency.

## EXP-004 result

EXP-004 preregistered a 12-query MRR@20 comparison against presence and RRF,
with objective official-documentation target URLs, a +0.10 minimum effect, and
coverage, validity, regression, latency, and cost guardrails. The live run was
**inconclusive**:

- zero of 12 target pages entered the candidate union (minimum required: eight);
- DuckDuckGo and Google were blocked, Wikipedia returned 403, and Stack Exchange
  produced one candidate total;
- the only non-empty Jev call returned a valid typed answer from `jev-1.13.0`;
- 393 input tokens cost an estimated `$0.000016506`;
- no relevance comparison was possible, so zero MRR values are not negative
  evidence about Jev.

The registered run was not retried or tuned. A future experiment must use a
captured candidate corpus with established rights, labels, and adequate target
coverage; live acquisition reliability must not be allowed to erase the
reranking question again.

## Gap register

| Gap | Why it matters | What would resolve it | Status |
| --- | --- | --- | --- |
| Observed user problem | No task evidence shows current ordering causes meaningful failure | Portal/MCP task observations or user interviews | Open |
| Practical effect | +0.10 MRR was a pilot threshold, not stakeholder-validated value | Tie ranking improvement to a frozen user/agent task | Open |
| Candidate corpus | Live sandbox acquisition yielded no usable pool | Licensed captured SlopSearX responses with independent labels | Open |
| Search-snippet rights | Providers' redistribution/caching terms differ | Source-by-source legal/terms review and evidence policy | Open |
| TypeSafe output caching | MCA assigns Output, but the account order was not inspected | Review applicable order/account terms | Open |
| Ordinary retention | No fixed deletion period found; ZDR is enterprise | Provider confirmation or enterprise ZDR agreement | Open |
| Sensitive/personal queries | No policy authorizes external transmission | Explicit operator policy, exclusions and disclosure | Open |
| Multilingual quality | Provider says English is strongest | Separate multilingual corpus and thresholds | Open |
| Acceptable latency/cost | No affected-user/operator threshold exists | Task-specific SLO and budget decision | Open |
| Model upgrades | Alias movement can change ranking | Pin/re-evaluate/version migration policy | Open |
| Portal disclosure | External processing and probabilistic order would be visible behavior | UX design and acceptance review | Open |
| Stakeholder validation | Affected users and privacy/security reviewers were absent | Conduct discovery before specification | Open |

## Interpretation audit

| Statement | Origin | Risk and treatment |
| --- | --- | --- |
| Explore optional value while preserving no-key operation | SAID by maintainer | Low; direct spike constraint |
| Do not treat the spike as delivery authorization | SAID in issue #389 | Low; no implementation created |
| Reranking is the most plausible first use | INTERPRETED from architecture and prior evidence | Medium; compared with five alternatives, not accepted as requirement |
| Explicit enablement should be separate from key presence | INTERPRETED from compatibility/privacy risk | Medium; requires operator validation |
| Fusion is preferable to standalone replacement | INTERPRETED from independent benchmark evidence | Medium; requires SlopSearX confirmation |
| Users will accept external snippet processing | INFERRED gap | High; not promoted to a requirement |

## Final recommendation

Outcome for issue #389: **learn more, without implementation**.

Jev has a credible technical shape and exceptionally low nominal token cost for
bounded judgments, but SlopSearX has not measured a relevance benefit, user
demand, acceptable privacy boundary, or production latency/reliability envelope.
Do not create a Jev client or feature issue yet.

A future `EXP-005` would be justified only when it can preregister:

1. a frozen, redistributable SlopSearX-shaped candidate corpus with adequate
   recall and independently produced labels;
2. RRF plus Jev fusion as the sole candidate, compared with untouched presence
   and RRF;
3. held-out evaluation, per-family regression limits, and a user- or agent-task
   interpretation for the minimum useful effect;
4. explicit data-rights, retention, spend, and evidence-retention decisions.

Only a supported confirmation result plus stakeholder validation should open a
separate implementation proposal. That proposal would still require normal
review and would not authorize merge or deployment.
