# EXP-077 fixed study design draft

Status: task/acquisition design only; no invocation registered. The approved
[release decision](../../research-usefulness-release-decision.md) controls the
frozen candidate and numerical gates. This draft permits no source acquisition
or model call before registration and exact runner qualification.

Two predeclared cohorts each contain eight distinct primary research tasks and
five exact-navigation tasks. Critical checks in cohort-draft.json remain fixed
through both stages. Stage 2 acquisition and scoring occur only after stage 1
passes, with no candidate or threshold tuning. An independent Luna critique
found no paraphrase of EXP-060's primary tasks; adjacent topics retain distinct
operational checks. Novel question text does not prove novel source acquisition
or model decontamination. Fresh result pools, previous-pool overlap and every
failed/empty/overflow attempt must be retained and assessed.

The ten declared target repositories were verified as public by ten metadata
requests; target-verification.json retains the results. These were target
definition checks, not query acquisitions or ranking outputs.

## Fixed acquisition path

Use the actual isolated production SearchService merge/group/ranker path.
Instantiate explicit public adapters with jev_router=None, rerank_provider=None,
suggestion_service=None, cache=None and generate_suggestions=False. No deployment
or environment/default mutation. Reference category, public GitHub repository
mode with no token, English requests, one page and max20 per applicable adapter.
Research engine list: arxiv/openalex/github/wikipedia; navigation: github only.
No Brave, dynamic specialist expansion, suggestion query, pagination or retries.

Physical HTTP maximum per stage is 53: eight primary queries times up to two
arXiv GETs (only its constrained HTTPS redirect), one OpenAlex GET, one GitHub
GET and two Wikipedia GETs, plus five GitHub navigation GETs. Freeze exact
transport time/byte/dispatch bounds and adapter pins before invocation.
Serialize with seven-second query pacing; returned throttling/errors are
retained, not rescued. Twenty-result settings cap dispatch output but do not
guarantee four natural post-group pools above forty. Never concatenate captures
or trim an over-bound pool to qualify.

## Documentation preflight (2026-10-05)

[GitHub search documentation](https://docs.github.com/en/rest/search/search)
describes best-match ordering and a ten-per-minute unauthenticated search limit.
Use public repository mode; do not silently enable code search or authentication.
[OpenAlex authentication](https://help.openalex.org/api/authentication/)
describes basic keyless use and budget/rate-limit errors. Its
[current pricing reference](https://help.openalex.org/access/pricing/)
supersedes the retired flat-call limit quoted in the adapter docstring. Use
keyless requests only, with no paid account or automatic top-up. No live API
availability, remaining quota or adequacy has been measured. The legacy doc
URL could not be fetched; current official pages were found via a documentation
lookup. Documentation reads/searches are separate from experiment acquisition;
no Brave search was used.

## Remaining registration requirements

Freeze source/answer critical checks, exact prompt/schema/packet assignments,
consumer projection and independent grading order; preserve sparse-but-useful
reading leads separately from captured factual support. Seal both cohorts and
complete acquisition/model/capture budgets, source hashes, candidate wire
contract and analyzer before calls. Keep source/answer integrity checks from
PR670 bounded; do not expand them into another open-ended framework.

Passing both adopted stages unblocks full opt-in implementation and required
compatibility/security/integration review. Failure or inconclusiveness ends this
fixed study and triggers explicit reassessment, not automatic tuning.
