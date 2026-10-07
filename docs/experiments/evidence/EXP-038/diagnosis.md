# EXP-038 W2 outcome diagnosis

## Result

W2 is **not supported as the ordinary-search default**. Its q1–q8 paired mean nDCG@10 changes were −0.0189 under the root labels and −0.0382 under the peer labels. Their query-bootstrap 95% upper bounds were +0.0288 and +0.0261, both below the registered +0.05 advancement requirement. Operational validation, exact-target navigation, repeat stability, q9 extension, top-ten useful-count, and facet-retention guards passed. The primary quality gate did not.

This is a ranking-quality result on exposed assistant-labeled development queries, not a claim about general search quality or factual correctness. The q9 climate extension improved by +0.1411 / +0.0552, but is reported separately and does not compensate for q1–q8.

## Where the loss concentrates

| Query | Root Δ nDCG@10 | Peer Δ nDCG@10 | Readout |
|---|---:|---:|---|
| q1 multi-agent coding collaboration | +0.0327 | −0.0885 | Reviewers disagree on several incoming/outgoing cards; W2 retained 10 useful results under both. |
| q2 enterprise agent governance | +0.0148 | −0.0783 | Same pattern: no useful-count or facet loss, but peer labels penalize the changed order. |
| q3 durable execution | +0.0379 | +0.0475 | Positive for both references. |
| q4 SWE-bench leakage | +0.0364 | +0.0441 | Positive for both. |
| q5 SLSA provenance | +0.0431 | +0.0431 | Positive for both. |
| q6 OpenTelemetry GenAI spans | −0.0284 | −0.0583 | Peer useful@10 fell from 10 to 9; no facet was lost. |
| q7 claim/citation verification | −0.2164 | −0.2540 | Largest shared loss; both reference sets favor the W0 order. |
| q8 research stopping/gaps | −0.0710 | +0.0380 | Reference disagreement reverses the direction. |

The q7 top ten is the clearest diagnostic case. W0’s top ten included `c0` DEEPSCIVERIFY, `c19` Zero-shot Scientific Claim Verification, `c23` RefVerifier, `c26` When Retrieval Helps and Distracts, and `c18` ClaimVer. Both reviewer rationales describe these as direct claim/citation verification or benchmark evidence. W2 replaced those cards with `c27` HALLMARK, `c16` a claim-support audit issue, `c14` CoVeGAT, `c8` A Citation Is Not Proof, and `c3` Bytez. Those are still related results; the peer labels grade three of the incoming cards 2 rather than the displaced cards’ 3. The resulting loss is ordering among mostly useful results, not a missing top-ten facet: useful@10 stayed 10, and all annotated facets remained represented. The query is a bare keyword string (“claim passage evidence support contradiction citation verification benchmark”), so whether it requests substantive evidence, a survey of source types, or benchmark discovery is not explicit. Intent ambiguity is a plausible explanation for this case, not a demonstrated cause.

Q6 illustrates the source-lead boundary. W2 moved `c6`, the official OpenTelemetry “Moved: Generative AI semantic conventions” page, into rank 10. Its snippet points readers to `/llms.txt` and the Markdown page but contains little convention detail; the peer rationale calls it contextual/background evidence. W2’s top ten consequently dropped `c10` (MortalApps’ agent span types) and `c1` (Datadog’s OpenTelemetry convention summary); the peer useful count fell by one. The five constructed exact-target controls all ranked first under W2, so exact navigation itself worked in those controls. The q6 shift instead suggests a possible mismatch between broad topical source-finding and substantive evidence ordering. It does not prove that the prompt caused the shift or that the OpenTelemetry result is useless.

Q8 is a useful counterexample to a simple explanation: W2 demoted `c26`, a POMDP paper on context gathering and redundant search loops, and promoted `c7`, a guide about describing insufficient or inconsistent evidence. The root labels prefer `c26`; the peer labels prefer `c7`, reversing the measured effect. This reinforces that the available references are uncertain assistant judgments, not independent gold.

Q1 and q2 show similar label sensitivity: the root deltas are slightly positive while peer deltas are negative, despite unchanged useful counts and facets. Q3–q5 improve for both reviewers. W2 therefore appears compatible with some narrowly worded technical evidence queries, but this exposed sample does not support a reliable generic default.

## What this experiment cannot isolate

W2 changed both the relevance contract and request shape: W0 used the shared candidate-list state and generic ten-level rubric; W2 used card-local structured questions, query-only state, and a four-level task-fit rubric. This comparison cannot attribute q7/q6 losses to intent inference, the rubric, score resolution, or card-local transport. EXP-035 found card-local questions reduced score drift, but did not establish quality superiority. Do not revise W2 against q7 or change weights/rules after seeing these outcomes.

All five navigation tasks were deliberately constructed from already-known cards and targets. Their perfect ranks establish success only on those controls. Likewise, the positive q9 result is one exposed extension query, not replication across domains.

## Strongest next candidate

Keep generic W0 as the ordinary-search behavior. A distinct candidate is an **explicit research objective supplied by the caller or user workflow**, rather than asking Jev to infer evidence-seeking versus source-finding from terse generic keywords. In that mode, pass the task and any required facets as trusted request context and score visible substantive evidence for those facets; topical titles/citations are leads rather than evidence, and directly relevant contradictory evidence remains relevant. Do not add a fetch-probability controller. Exact named-document navigation can remain a separate explicit route with deterministic target matching, while “find publications/resources” should be an explicit source-listing objective if that use case is required.

A new qualification should first hold request shape fixed to separate objective wording from transport/context effects. If card-local batching is also under consideration, preregister a small factorial comparison (generic versus explicit research objective, crossed with shared-state versus card-local batch); do not conflate it with the full-set question. Use new independently reviewed intent-stratified query/card units for confirmation, with exposed q1–q9 retained as development evidence only. Freeze labels and the intent schema before model outputs, report evidence-seeking and source-listing strata separately, and retain exact-navigation controls as a guard. Keep whole-set (>40) scope in a separate study; EXP-038 only tests the frozen first-40 pools plus navigation variants.

No additional calls or edits to frozen history were made for this diagnosis.
