# Coverage-first ranking: approved candidate amendment

Status: **candidate amendment approved by the maintainer on 2026-10-09**.
Related to [#516](https://github.com/magnus919/SlopSearX/issues/516).
This records a prospective decision, not a registered or admitted invocation.

The maintainer approved replacing the failed complete-pool dual-Score/RRF
candidate with the [coverage-first prototype](intent-ranking-coverage-prototype.md)
for a new bounded study. This supersedes only the sole-candidate constraint in
the [2026-10-05 release decision](research-usefulness-release-decision.md).
Historical registrations, results and failed/inconclusive decisions remain
unchanged. Their clocks, leases, receipts and exposed cases confer no new
invocation authority or qualifying credit.

The candidate is the implementation merged in PR #728 at
`6818b658f8a701ab826582fc2dd496240378f39b`:

| Source | SHA-256 |
| --- | --- |
| `scripts/intent_ranking_coverage.py` | `806746b4fa5c497f269933008db76650e2c1113dbd35a08b011d86c4d81f11ca` |

It uses the pinned query Score plus a Choice over caller-facet subsets, reserves
an eligible lead for each facet, and fills the remaining complete order by
query score and canonical tie break. Missing usable facet leads or invalid
responses preserve the whole incumbent order. Freeze the exact instructions,
model, representation, score floor, validation and ordering before measurement;
no tuning or rescue after registration. Caller facets describe the task and
must not come from evaluator labels or observed candidate outcomes.

## Scope and retained acceptance

Keep the complete production objective: qualify usefulness, then deliver the
supported opt-in behavior through the shared SlopSearX service, API, MCP and
portal, with cache/version identity, fallback, compatibility, security,
integration, review and CI. Propagate caller context through GroktoCrawl X
where needed without a duplicate reranker. Existing query-only defaults,
SearXNG compatibility and deployment remain unchanged until separately
reviewed product delivery and authorized rollout.

The amendment preserves all numerical ranking, task-use, navigation, stability,
membership, timing and resource gates from the approved release bar. Register
eight **new** primary development tasks/pools and eight distinct **untouched**
confirmation tasks/pools together. Historical exposed cases may be exploratory
diagnostics only. Freeze fresh blind independent card/source references before
any Jev score or selector output in each stage. Freeze equal consumer and
answerer budgets; assess references separately, never average disagreement
into a pass. Substantive, applicable evidence must be delivered and correctly
used for every critical check; bibliographic or topical leads alone do not
complete factual-answer tasks.

Declare first-40 and broader 41–80-result qualification separately, retaining
at least four natural 41–80-card primary pools per stage. Bind the control to
the exact current production ranking source and configuration on identical
pools; a native pre-Jev ordering is not a substitute for the active production
path. Preserve all failures, empty/overflow pools and missing targets; no
replacement, concatenation, trimming, silent retry or rescue query.

The remaining Brave allowance stays reserved: eight of ten attempts were used,
two remain. Plan this study with a fixed non-Brave engine mix. Approval of the
candidate does not permit exceeding any prior resource ceiling. Derive the
exact physical-call schedule and token reservations from the new protocol;
do not silently import obsolete limits or treat available budget as evidence.

## Execution prerequisites and stop rule

Finish the [qualification checklist](intent-ranking-coverage-qualification.md):
fully pinned cohorts, packet assignments, source/dependency closure, no-call
preparation command, executable runner, exact budgets and failure inventory.
Qualify response archival and replay through the unchanged parser/selector,
including incomplete/failed slots and later uninvoked operations. Merge the
reviewed registration and qualification artifacts, then publish a new admission
receipt before bounded readiness or scientific calls.

Exactly one invocation per declared stage is permitted after those prerequisites
are met. A development pass permits only the already registered untouched
confirmation stage, with no tuning between stages. A failure or inconclusive
result ends this fixed study and is published through a results PR; it does not
authorize another candidate automatically. Both stages must pass before product
implementation. Structural tests, documentation merges and transport readiness
do not establish research usefulness or complete #516.
