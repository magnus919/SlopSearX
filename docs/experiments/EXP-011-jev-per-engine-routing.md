# EXP-011: Jev per-engine routing

**Status:** preregistered; no measurements collected

## Decision

Determine whether one batched Jev decision per eligible engine can select a
bounded search scope that improves useful evidence coverage over SlopSearX's
current deterministic intent router. This experiment does not authorize a
product feature.

## Hypothesis

Given a search query and concise, centrally maintained descriptions of each
eligible engine, Jev can independently estimate whether each engine is likely
to contribute useful evidence. Selecting at most three engines from those
probabilities will improve held-out target-engine F1 and live evidence coverage
without weakening policy gates or increasing the dispatch budget.

## Invariants

- Existing authorization, sensitive-engine, credential, health, and operator
  configuration checks run before Jev. Jev cannot make an ineligible engine
  eligible.
- Absence, timeout, or malformed output from Jev falls back to deterministic
  routing. SlopSearX remains fully functional without a TypeSafe key.
- The router selects no more than three engines per query.
- Search-result ranking and merging are unchanged.
- All data is synthetic. No user queries or user results are sent to TypeSafe.

## Frozen inputs

- Model: `jev-1.13.0`.
- Routing cards: `evidence/EXP-011/engine-routing-cards-v1.yaml`.
- Query manifests: `evidence/EXP-011/development-v1.json` and
  `evidence/EXP-011/evaluation-v1.json`.
- Harness: `evidence/EXP-011/harness.py.txt`.
- Maximum selected engines: 3.
- Candidate Noul thresholds: 0.45, 0.55, 0.65.

Cards exist for every registered adapter. Measurements use only adapters that
are active, policy-eligible, and sufficiently configured at runtime. Engine
cards are advisory context, not adapter configuration and not an authorization
mechanism.

## Phase 1: metadata validation

The harness must fail before network access if cards and the runtime registry
differ, a card is missing required fields, a manifest repeats an ID, a target
engine is unknown, or the split sizes/families differ from this protocol.

## Phase 2: development selection

Run the 12-query development set against four strategies:

1. `deterministic`: current `IntentRouter`, capped at three engines.
2. `choice`: one Jev Choice among broad, specialist, or blended plans.
3. `noul-minimal`: one batched Noul per eligible engine using registry metadata.
4. `noul-card`: one batched Noul per eligible engine using the enriched cards.

For each Noul strategy, evaluate all frozen thresholds offline from the same
response. Select the configuration lexicographically by: highest macro recall,
highest F1, lowest mean selected-engine count, then strategy name and higher
threshold. The selected configuration and its request-schema checksum must be
committed before the evaluation split is opened.

## Phase 3: held-out routing evaluation

The 30-query evaluation set has six families of five queries: packages/code,
science, medical, security, reference/archive, and general/news/social. Report
micro precision, recall, and F1; macro family recall; paraphrase agreement;
mean selection size; invalid responses; and policy violations.

The routing method advances only if all gates pass:

- recall >= 0.85;
- precision >= 0.65;
- macro family recall >= 0.80;
- F1 improves by at least 0.10 absolute over deterministic;
- paraphrase agreement >= 0.90;
- no family recall regresses by more than 0.10;
- zero policy or sensitive-engine violations;
- at least 29 of 30 responses are valid.

No post-evaluation tuning is allowed.

## Phase 4: bounded acquisition

Run only if Phase 3 passes. For each evaluation query, dispatch the baseline's
three-or-fewer engines and Jev's three-or-fewer engines. Call their union once,
so shared engines are neither billed nor rate-limited twice, then replay the
same captured responses into both strategy views. Brave may be used once per
query as a common control.

Hard ceilings:

- Brave: 30 expected calls, 36 absolute maximum. The six-call reserve is only
  for transport failure or an empty response; never retry a valid response.
- Other engines: 180 calls total and 8 calls per engine; no retries.
- Jev: 200,000 input tokens for the complete experiment.
- One development pass, one evaluation pass, one acquisition pass.

Stop immediately when any ceiling would be exceeded.

## Phase 5: product decision

Primary outcome: synthetic target coverage in the top 10 results. Secondary
outcomes: reciprocal rank, unique specialist wins, engine-family contribution,
failures, latency, and estimated marginal cost.

Recommend a product experiment only if all of these hold:

- target coverage improves by at least 0.10 absolute (at least 3/30 queries);
- specialist engines uniquely turn failure into success on at least five
  queries across at least three families;
- no family loses more than one successful query;
- dispatch budget does not increase;
- at least 29/30 acquisition rows are valid;
- Jev p95 latency is <= 600 ms;
- all budgets and invariants pass.

Recommend **no** if acquisition is valid but effect or guardrails fail. Report
**inconclusive** if fewer than 20/30 feeds are usable, more than 20% of free
engine calls fail environmentally, credentials fail, or routing and
acquisition effects cannot be separated. Any retry or revised design becomes
EXP-012.

## Evidence and reproducibility

The report must include sanitized per-query routing rows, per-engine status and
contribution, aggregate metrics, request/response schema versions, SHA-256
checksums of frozen inputs, budget counters, and a final yes/no/inconclusive
recommendation. Raw credentials, headers, and full third-party response bodies
must never be committed.
