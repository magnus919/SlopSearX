# Measured improvement loop

SlopSearX experiments improve useful search, human task completion, or agent
task completion while preserving the shared service and compatibility contracts.
The objective is material, evidenced improvements delivered to the repository.
Every experiment also produces evidence and a decision; weak candidates are
rejected. Follow [the loop design](DESIGN.md) for opportunity selection, evidence
standards and delivery accounting. Production operation belongs to operators.

## Run one cycle

1. Read this ledger, relevant project contracts, current issues and open PRs.
   Resume an unfinished experiment before proposing another. Do not duplicate
   rejected work without stating what new evidence makes a retry worthwhile.
2. Select the strongest consequential opportunity using [DESIGN.md](DESIGN.md):
   identify the affected task, before/after behavior, practical effect and
   reachable measurement path before registration. Resume supported delivery
   first, and avoid repeating unchanged blockers. Check portal prerequisites.
3. Copy [TEMPLATE.md](TEMPLATE.md) to `EXP-NNN-short-name.md`, assign the next
   unused ID, and add it to the ledger. Complete and commit the registration
   **before implementing or measuring the candidate**. Record the baseline SHA,
   primary metric, minimum useful effect, guardrails, sample plan, stopping
   rule, resource limits, and exact commands. Check existing issues and create
   a focused issue before an implementation PR, per CONTRIBUTING.md.
4. Use an isolated branch/worktree from a recorded main SHA. Keep unrelated
   work intact. Compare baseline and candidate on identical inputs through
   the normal service/transport path appropriate to the claim. Pin fixtures,
   dependency versions, configuration, random seeds, and environment details.
5. Run the registered comparison and required regression checks. Preserve raw
   observations, failures, exclusions, commands, and environment metadata in
   `docs/experiments/evidence/EXP-NNN/` or a durable linked artifact with a
   checksum. Remove secrets and personal queries; retain only redistributable
   fixtures. Do not overwrite unsuccessful trials with successful reruns.
6. Append the readout and update the ledger. Use exactly one outcome:
   `supported`, `not-supported`, `inconclusive`, or `blocked`. Record unfinished
   work as `registered` or `running`, never as a completed result.
7. For `supported`, open a focused, ready-for-review implementation PR with the
   experiment, evidence, practical effect, limitations, tests, portal impact,
   and rollback. Follow DCO, pre-commit, CI, and review requirements. The standing implementation merge rule below authorizes merging after
   passing CI and review; it does not authorize deployment or changing production defaults.
8. For other outcomes, discard candidate implementation changes after retaining
   the readout and sufficient reproduction material (a small sanitized patch
   or durable candidate commit). Preserve the experiment on a documentation-only
   branch/PR even when the candidate is discarded. Label it explicitly as an
   experiment record, not an improvement. Never delete unrelated work.

## Documentation-only PR delivery

Automatically merge documentation-only experiment PRs, including the initial
process documentation and ledger updates for every outcome. Do not request code
review or run CI/pre-commit for these PRs. Before merging, inspect the changed
paths and diff to confirm they contain only documentation and inert experiment
evidence, with no executable code, configuration, dependencies, or workflows.
Use a DCO-signed documentation commit with `[skip ci]` to suppress workflows
that honor that marker. Do not request automated reviewer comments.

If supported implementation work remains under review, a separate documentation
PR can persist its findings immediately; record the implementation PR as pending,
not shipped. Verify the documentation PR actually merged and update the local
main checkout with a fast-forward when safe, preserving unrelated work.
If branch protection prevents merging without checks or review, report that
specific blocker; do not disable repository protections. Mixed documentation and
implementation PRs retain normal checks and the standing merge rule below.

Experiment records are append-only history: corrections and subsequent trials
must identify the earlier result. Record persistence is required for every
outcome; failed implementation work must not disappear with its branch.

## Standing implementation merge rule — authorized 2026-10-03

Implementation or mixed PRs authored under `magnus919` are authorized for
merge once all applicable CI checks pass and Droid supplies a positive substantive
code review. A green review job without usable review text is insufficient.
If Droid is unavailable, fails or supplies no usable review, the runner must
perform and record its own substantive review of the current candidate. Fix
findings and rerun affected checks before merging. No human self-approval is
required. Verify the PR author, candidate SHA, review evidence and complete CI suite;
never bypass branch protection. Verify the merge, update delivery in the ledger
and safely fast-forward local main. This authorizes repository merging, not
production deployment. Retarget stacked PRs after prerequisites merge; if
retargeting does not trigger CI, refresh the branch to trigger normal CI.

## Decide before seeing results

Each experiment has **one primary outcome** and a predeclared minimum useful
improvement. Secondary metrics explain results; they cannot rescue a failed
primary outcome. Predeclare guardrail tolerances for relevant query families,
errors, latency, cost, accessibility, and correctness. Policy, tenant isolation,
filter truthfulness, serialization, and API/MCP/portal compatibility must pass
regardless of the primary metric.

For noisy measurements, specify the independent sampling unit, sample-size
rationale, fixed sample/stopping rule, estimator, and uncertainty calculation.
Use paired inputs and randomized/interleaved baseline/candidate runs where
appropriate. A supported result needs its uncertainty bound to clear the
registered useful-effect threshold and all guardrails. If the budget cannot
resolve that effect, report `inconclusive`. Repeated timings of one query are
not independent users or queries. Do not stop when a result first looks good.
Account for testing multiple candidates; use fresh confirmation data for the
selected candidate. Do not tune on held-out evaluation inputs.

For exhaustive deterministic claims, report exact differences across the fixed
corpus and repeatability; statistical inference may be inapplicable. Limit the
claim to that corpus and behavior. Passing tests proves correctness, not a user
benefit. Synthetic relevance fixtures do not establish production relevance.
Automated browser actions measure scripted flows, not human usability. Human
usability claims require actual participant evidence. Agent task-success claims
require a frozen task set, scoring rubric, model/configuration, and repeated
runs; token reduction alone is insufficient if task completion regresses.

Use `not-supported` when the completed comparison fails the decision rule or a
guardrail. It means this candidate lacks support under these conditions, not
that the idea can never work. Missing data, underpowered comparisons, and
infrastructure failures are `inconclusive` or `blocked`, not negative evidence.

## Useful measurement seams

| Objective | Primary outcome examples | Existing starting point | Limits |
| --- | --- | --- | --- |
| Search usefulness | independently judged nDCG@K or task success | [Retrieval evaluation](../RETRIEVAL_QUALITY_EVALUATION.md), `tests/test_rank_fusion.py` | Current synthetic fixtures are not production-quality evidence. |
| Routing | correct scope selection across a fixed labeled set | `tests/test_routing_eval.py` | Scope accuracy alone does not establish relevant results. |
| Agent experience | task success, calls or tokens per successful task | `tests/test_mcp_harness.py`, `slopsearx/mcp/harness.py` | Deterministic fake engines test contracts, not live availability. |
| Human experience | completion rate, time to find a useful result | [Portal acceptance](../PORTAL_ACCEPTANCE.md) | Visual inspection and accessibility checks remain necessary. |
| Efficiency | latency distribution, CPU or memory per equivalent response | paired replay through shared service | Protect result equivalence; separate warm/cold cache and upstream variation. |

## Execution limits

Default to one active experiment and one candidate per cycle, offline replay,
no paid API calls, and a 45-minute execution budget. Register tighter or justified
alternative limits before running. Stop on guardrail breach or budget exhaustion,
record partial evidence, and resume only under an explicit updated plan. Live
traffic, production writes, new spend, or participant recruitment require scope
that the user has authorized. Never change the decision rule after seeing data;
a revised rule is a new experiment linked to its predecessor.

A recurring runner should read [RUNNER.md](RUNNER.md). The scheduler determines
cadence; this document does not itself start background work. If a previous
cycle is still active or its documentation is not persisted, resume it instead
of opening a competing experiment. Automatically merge documentation-only
experiment PRs as described below; merge implementation PRs only under the standing rule below. Do not deploy automatically.

## Ledger

| ID | Hypothesis / question | State | Decision / evidence | Implementation PR |
| --- | --- | --- | --- | --- |
| [EXP-049](EXP-049-snapshot-corruption.md) | Reject malformed persisted snapshots consistently across agent read paths | supported | Normal-path defect replay: 0/24 → 24/24 correct invalid-cursor rejections; 18/18 controls unchanged, no engine dispatch or record mutation. Production prevalence unknown. | [#556](https://github.com/magnus919/SlopSearX/pull/556) merged `50f19c8`; no deployment |
| [EXP-045](EXP-045-contextual-source-replacement.md) | Can contextual keep-or-replace judgments improve complete-pool selection without losing useful evidence? | inconclusive | [Readout](evidence/EXP-045/readout.md): neutral acceptance passed; first development response had one probability distribution summing to 0.99, failing the frozen 1e-6 tolerance. One case request, 20 operations unattempted, no retries; no quality claim; 0/10 Brave. | Follow-up #516; no runtime change |
| [EXP-047](EXP-047-visible-card-answerability.md) | Can visible cards support a specific whole-set replacement judgment? | incomplete | Both fresh Luna reviewers stopped on truncated file-read output; no grading, Jev/search calls or adoption claim. | Follow-up #516; no runtime change |
| [EXP-048](EXP-048-complete-pool-review-delivery.md) | Can the same blinded cards be delivered completely for review? | incomplete | Complete delivery; A valid, B contradictory; grounding ambiguities; no adoption or new replacement-quality authorization. | Follow-up #516; no runtime change |
| [EXP-050](EXP-050-identifier-bound-source-information.md) | Can exact publication identifiers supply missing public metadata? | information screen passed | [Readout](evidence/EXP-050/readout.md): one lookup, 178 matched / 41 unresolved / 204 no identifier; all 423 cards retained; no ranking claim, 0/10 Brave. | Follow-up #516; no runtime change |
| [EXP-051](EXP-051-enriched-visible-support.md) | Separate visible facet support from reading-lead value using identifier-linked metadata | incomplete | Both one-shot Luna reviews incomplete (input truncation / output capacity); 0/2 valid records. No retries or new search, metadata or Jev requests; original responses retained. | Follow-up #516; production acceptance remains open |
| [EXP-052](EXP-052-pool-visible-support.md) | Qualify one-complete-pool independent visible-support references | incomplete | First pair stopped: registration artifact omitted from allowed inputs; 0 valid records, 2 invoked / 10 not invoked. No retries or new retrieval. | Follow-up #516; production acceptance remains open |
| [EXP-053](EXP-053-complete-input-visible-support.md) | Supply complete identity-verification inputs to independent references | registered | Same frozen data/schema/validator; fresh cohort; original and method registration artifacts explicitly supplied. No new retrieval. | Follow-up #516; production acceptance remains open |
| [EXP-046](EXP-046-bounded-choice-probability-validation.md) | Does a fixed one-percentage-point probability-total tolerance permit reliable evaluation of the unchanged whole-pool replacement policy? | not-supported | [Readout](evidence/EXP-046/readout.md): all 21 cases valid; 18 global KEEP/3 branch KEEP, no replacements; B research grounding preservation and strict coverage gain fail; 22 total attempts, zero retries/searches; 0/10 Brave. | Follow-up #516; no runtime change |
| [EXP-044](EXP-044-relative-facet-reservations.md) | Can relative facet leaders preserve requested topics without degrading whole-pool usefulness? | not-supported | [Readout](evidence/EXP-044/readout.md): reservations restore B research grounding but fail B primary uncertainty, B noninferiority and per-pool losses; zero new calls; no fresh confirmation. | Follow-up #516; no runtime change |
| [EXP-043](EXP-043-separated-visible-facet-selection.md) | Do independent source-usefulness and stricter visible-topic judgments improve whole-pool selection? | selection not supported | All 65 calls valid; primary gain passes A/B, but zero added facet coverage and B research grounding loss fail unchanged gates; 0/10 Brave used. | Follow-up #516; no runtime change |
| [EXP-042](EXP-042-explicit-facet-source-selection.md) | Can explicit per-topic judgments preserve reading quality while a bounded selector improves coverage? | not-supported | [Terminal readout](evidence/EXP-042/readout.md): 35 valid/36 attempted; large mixed request HTTP 400; no retries. Corrected partial analysis shows no coverage gain and fails reference B uncertainty. 0/10 Brave used. | Evidence retained; candidate rejected; follow-up #516 open |
| [EXP-041](EXP-041-arxiv-html-identity.md) | Do official arXiv HTML representations group with their abstract/PDF work without false merges? | supported; merged | Candidate grouping 55 passed; full suite 2,333 passed, 85.41% coverage; required CI/review passed. Zero provider/search calls. | [PR #529](https://github.com/magnus919/SlopSearX/pull/529), merged `80029eb`; no deployment |
| [EXP-040](EXP-040-complete-purpose-source-selection.md) | Can explicit-purpose source selection improve complete natural pools while preserving useful coverage? | not supported | 48/48 valid calls; primary and cardiac gains, but research evidence-grounding facet lost under assessment B. All navigation/stability/operational gates passed; 0/10 Brave attempts. | Follow-up #516; no runtime change |
| [EXP-039](EXP-039-explicit-purpose-jev-development.md) | Does explicit caller purpose and source-selection scoring improve reading-lead ranking? | not-supported for selection | 40/40 valid; mean nDCG +0.2145/+0.0753 clears statistical gates but useful-source identity retention fails. [Readout](evidence/EXP-039/readout.md). 0/10 Brave attempts. | [PR #522](https://github.com/magnus919/SlopSearX/pull/522); no runtime change |
| [EXP-038](EXP-038-intent-aware-jev-development.md) | Does intent-aware card-local ranking improve evidence ordering without navigation regression? | not-supported for default adoption | 32/32 valid; all guards pass, but mean nDCG −0.0189 root / −0.0382 Luna misses +0.05 gate. 0/10 Brave attempts used. | None; follow-up #516 |
| [EXP-037](EXP-037-jev-transport-bounds.md) | Can bounded transport and strict parsing reject hostile responses while preserving valid rankings? | supported; merged | 48 targeted, 114 API/portal and 2,312 full-suite tests pass; review and all applicable CI green. No deployment. | [PR #518](https://github.com/magnus919/SlopSearX/pull/518), merged `b8720d9` |
| [EXP-036](EXP-036-heldout-complete-set-reranking.md) | Does fresh complete-set card-local ranking improve task usefulness? | quality gate not met; tail benefit inconclusive | Seven valid paired original queries: mean nDCG delta +0.0104 root / +0.0327 Luna, below +0.05. Cardiac breadth case has 44 cards and regresses under both references; 21/22 candidate responses accepted. No runtime change. | [PR #512](https://github.com/magnus919/SlopSearX/pull/512), merged `35b60dc` |
| [EXP-035](EXP-035-card-local-whole-set-reranking.md) | Does card-local question context stabilize complete-set reranking? | supported mechanism; quality adoption pending | 21/21 valid; maximum whole/batch drift 2.11→0.41; top-ten overlap 0.9–1.0; constructed 80-card request 388ms. Exposed RAG nDCG regressed 0.0497; fresh validation required. | [PR #512](https://github.com/magnus919/SlopSearX/pull/512), merged `35b60dc` |
| [EXP-034](EXP-034-whole-set-jev-reranking.md) | Can whole-set scoring improve relevant-tail access over first40? | feasible; quality inconclusive; naive batches not supported | Two real44/45-card pools,14/14 valid; nDCG mean delta -0.0026. Batch composition drift up to2.11 and one ranking regression. | [PR #512](https://github.com/magnus919/SlopSearX/pull/512), merged `35b60dc` |
| [EXP-033](EXP-033-capability-advisories.md) | Can bounded capability advisories truthfully disclose relevant limitations? | supported for disclosure | 0/14 to 14/14 eligible notes; 8/8 quiet controls. [Readout](EXP-033-results.md). Query benefit unmeasured. | [PR #506](https://github.com/magnus919/SlopSearX/pull/506), merged 2026-10-03 |
| [EXP-032](EXP-032-corrupt-cache-recovery.md) | Can corrupt-cache fallback restore correct search completion? | supported | Real-Valkey fault replay: 0/16 → 16/16 correct completions; repaired hits and controls pass. Production prevalence unknown. | [PR #500](https://github.com/magnus919/SlopSearX/pull/500), merged 2026-10-03 |
| [EXP-031](EXP-031-filter-warning-truth.md) | Does enforcement-specific prose eliminate false consumption warnings? | supported | Warning truthfulness 50%→100%; +0.493% bytes, other fields equal. Oct 3: prior blockers resolved; full checks and review pass. | [PR #510](https://github.com/magnus919/SlopSearX/pull/510), merged 2026-10-03 |
| [EXP-030](EXP-030-negative-cache-path.md) | Can TTL validation prevent silent negative-cache loss on the service path? | blocked | No current production set_error caller; intended service benefit cannot be replayed. | None |
| [EXP-029](EXP-029-empty-search-recovery.md) | Does an empty-search hint improve human recovery within two minutes? | blocked | No available participant cohort or measured recovery baseline; no UI change or effect measured. | None |
| [EXP-028](EXP-028-safe-utf8-cache.md) | Can UTF-8 with ASCII fallback preserve caching and save 10% of bytes? | not-supported | Fallback passed all guards; 0.315% byte saving misses 10% minimum. | None |
| [EXP-027](EXP-027-utf8-cache.md) | Does literal UTF-8 reduce cache bytes without losing cacheable responses? | not-supported | Lone-surrogate cache write lost; adapter dispatched twice instead of once. Stopped before byte comparison. | None |
| [EXP-026](EXP-026-compact-cache-json.md) | Does compact cache JSON save at least 10% of response bytes? | not-supported | 6.06% cache-value byte saving, below 10% minimum; all 80 cache round trips passed. | None |
| [EXP-025](EXP-025-captured-ranking.md) | Does opt-in RRF improve captured-query nDCG@10 over presence? | blocked | No eligible independently judged captured relevance pool; no ranking replay or effect measured. | None |
| [EXP-024](EXP-024-call-local-url-memo.md) | Does call-local URL memoization materially reduce service latency across overlap levels? | not-supported | 14.06% / 0.0582 ms saving; misses 0.2 ms minimum. All 14,400 service response comparisons passed. | None |
| [EXP-023](EXP-023-jev-utility-composition.md) | Does code-composed Jev primary/secondary evidence judgment improve full-catalog specialist routing over the shipped single-Noul rule? | inconclusive | Exposed-corpus utility/query 1.611 vs 1.722 shipped; +4 useful but +8 irrelevant requests, advance gate failed. Zero search calls or product changes. | None |
| [EXP-022](EXP-022-jev-composed-next-source.md) | Do atomic Jev judgments composed in code improve realized next-source value over a same-state compound Jev judgment? | inconclusive | Replay on exposed cases: identical held-out selections and -0.25 realized net utility/query in both arms; +0.00 delta, advance gate failed. No fresh searches or product changes. | None |
| [EXP-021](EXP-021-jev-card-triage-brave-e2e.md) | Does Jev improve additive quality triage on actual Brave-backed cards, including prompt-injection controls? | inconclusive | F1 0.851 vs 0.667 baseline, but query-bootstrap interval includes a non-useful effect and one injected attack was missed; no security-gate use. [Readout](JEV-ADVISORY-E2E-RESULTS.md). | None |
| [EXP-020](EXP-020-jev-next-source-brave-e2e.md) | Does Jev improve realized next-source value after a Brave-backed first pass? | inconclusive | Net utility/query 0.00 vs -0.25 baseline; wide interval, one useful npm source missed, archive availability confounded value. [Readout](JEV-ADVISORY-E2E-RESULTS.md). | None |
| [EXP-019](EXP-019-jev-annotations-brave-e2e.md) | Does Jev improve multi-label annotations on actual Brave-backed result cards? | inconclusive | Held-out F1 0.900 vs 0.957 deterministic baseline; wide interval and taxonomy gaps. [Readout](JEV-ADVISORY-E2E-RESULTS.md). | None |
| [EXP-018](EXP-018-jev-card-triage-e2e.md) | Can Jev improve additive direct-lead and instruction-attack hints on actual search output? | inconclusive | Shared acquisition yielded zero general cards: DDG and Google were blocked on all 20 queries; no Jev calls. | None |
| [EXP-017](EXP-017-jev-next-source-e2e.md) | Does Jev add net evidence value when advising a follow-up source after an actual search? | inconclusive | Shared acquisition yielded zero general cards: no first-pass response for next-source advice; no Jev calls. | None |
| [EXP-016](EXP-016-jev-source-annotations-e2e.md) | Can Jev improve multi-label source-type annotations on cards acquired by the shared search service? | inconclusive | 40/40 free general adapter attempts blocked; 60 free specialist attempts retained. No paid search or Jev call. | None |
| [EXP-015](EXP-015-jev-advisory-calibration.md) | Do calibrated Jev scores improve additive multi-label annotations, next-source advice, and result-card quality triage on synthetic holdout cases? | inconclusive | Per-label held-out F1: annotations 0.952 vs 0.625 baseline; next-source 0.875 vs 0.857; triage 0.909 vs 0.769. Tiny synthetic holdouts, uncertainty includes zero; no product change. Zero search calls. | None |
| [EXP-014](EXP-014-jev-specialist-threshold-value.md) | At what Jev score does another specialist request add enough evidence to justify its cost? | supported | 0.55-0.65 viable; registered rule selected 0.65 (92.3% essential recall, 96.9% precision, 43.8% labeled visible yield). Specialist-aware coverage 36% vs 8% tier-first. | None |
| [EXP-013](EXP-013-jev-specialist-augmentation.md) | Can Jev add only genuinely specialist engines to a deterministic general-search base? | not-supported end-to-end; routing supported | Offline specialist routing passed every gate (F1 0.8736, recall 0.9744, precision 0.7917), but acquisition coverage improved only 2/25 to 3/25 because specialist results rarely reached the tier-first top ten. | None |
| [EXP-012](EXP-012-jev-explicit-engine-routing.md) | Do explicitly engine-bound Jev Nouls improve bounded routing over the production scope resolver? | advancement failed; further research supported | Held-out F1 improved 0.1905 to 0.6286, but recall 0.7719, precision 0.5301, and paraphrase agreement 0.6778 missed production-advancement gates. Stopped before acquisition; 0 Brave calls. | None |
| [EXP-011](EXP-011-jev-per-engine-routing.md) | Can batched Jev per-engine decisions improve bounded engine routing over the deterministic router? | inconclusive | Development used 135,999/200,000 Jev input tokens and exposed ambiguous engine binding plus a non-production fallback baseline. Stopped before held-out or search acquisition; 0 Brave calls. | None |
| [EXP-004](EXP-004-jev-official-doc-rerank.md) | Can a bounded Jev rerank materially improve official-documentation navigation over presence/RRF? | inconclusive | Live acquisition returned zero canonical targets; the registered relevance effect could not be measured. | None |
| [EXP-003](EXP-003-default-language-report.md) | Does reporting default English restore truthful language-filter metadata? | supported | Language metadata coverage 33.33% to 100%; +4.27% bytes; other fields identical. | [PR #392](https://github.com/magnus919/SlopSearX/pull/392), merged 2026-09-20 |
| [EXP-002](EXP-002-empty-query-fast-path.md) | Does skipping empty query processing reduce shared-service latency meaningfully? | not-supported | 6.86% / 0.0265 ms saving; below 10% and 0.2 ms targets. Response equality passed. | None |
| [EXP-001](EXP-001-results-only.md) | Can results-only MCP output save >=10% bytes without losing interpretation facts? | not-supported | Completed isolated retry: 6.97% smaller (target 10%); explicit engine outcome facts lost. All eight pairs completed. | None |
| Historical reference | Would RRF improve all three synthetic query families? | not-supported | [Existing evaluation](../RETRIEVAL_QUALITY_EVALUATION.md): two improve, science regresses. Retrospective reference, not a preregistered experiment or a new run. | None from this loop |

## One-off loop redesign pilot

[SLO-PILOT-001](SLO-PILOT-001.md) audits measurement readiness and adds a draft
[SLO declaration](../SLO.md). It is an explicitly requested extra run, not a
scheduled experiment or evidence of improved production availability. Its
implementation delivery remains subject to normal review and CI.

## Candidate backlog (not yet tested by this loop)

These are investigation leads, not accepted hypotheses or promised benefits.

- Identify repeated response-normalization work; test whether eliminating it
  reduces CPU/latency while preserving complete response semantics.
- Measure whether a bounded MCP presentation reduces tokens per successful
  agent task without reducing task completion or provenance usability.
- Investigate portal explanations of empty/partial results against the accepted
  UX specification; measure a defined recovery task before proposing changes.

Promote a lead into a registered experiment only after inspecting its real
production path and establishing a measurable baseline.
