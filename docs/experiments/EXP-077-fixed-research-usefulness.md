# EXP-077 — fixed candidate, fresh research-usefulness decision

Status: design and source ready to seal; no acquisition, capture, selector or answer invocation.
Baseline source: `15b2410770d4bdf1dfcf6ad4f2da27ec1904c337`.
Issue: [#516](https://github.com/magnus919/SlopSearX/issues/516).

This is the final bounded study for the candidate frozen in the
[approved release decision](research-usefulness-release-decision.md). It tests
whether selecting across the complete eligible pool helps finish research
questions under equal reading and answering budgets. EXP-076 is historical
rejected evidence; its exposed cases are not regraded.

Both prospective cohorts are in [cohorts.json](evidence/EXP-077/cohorts.json).
Each has eight research tasks and five exact navigation targets. Acquire the
first cohort once, then freeze its shared source captures and independent
references before any selector or answer output. Acquire the untouched second
cohort only if the first passes. There is no tuning, replacement question,
rescue query or automatic retry between stages.

The [protocol](evidence/EXP-077/protocol.json) fixes the candidate, model,
resource limits, stages and gates. Each stage requires at least +.02 mean
nDCG@10 with a positive bootstrap lower bound under each independent reference,
and at least two additional completed research tasks under each independent
answer assessor, with no baseline completion lost. All other approved ranking,
source-use, navigation and resource guards remain mandatory. Independent Luna
assessments are model assessments, not human ground truth. Passing supports a
bounded opt-in claim, not an automatic change of the default.

Search requests use the native production merge/group/rank path in an isolated
context with reranking and suggestions disabled. No Brave requests are planned.
Owned experiment HTTP ceilings are 53 acquisition requests and 641 capture-stage
requests (640 distinct public URL attempts plus one health check). The scraper
may make additional requests internally; those are unknown unless exposed and
are not represented as bounded by 641. Raw capture contents remain private;
public records retain sanitized hashes, counts, failures and decisions.

## Execution and delivery

1. Seal exact design, packet assignments, schemas and source pins; qualify the
   precise inert runners with focused tests and a bounded substantive review.
2. Acquire/freeze first-stage pools, captures and independent references.
3. Execute the frozen selector and equal-budget answer comparison once.
4. Preserve results and make the declared pass/stop decision. If it passes,
   repeat on the separately registered untouched confirmation cohort.
5. Only if both pass, implement and deliver the full optional feature through
   SlopSearX and the experimental X callers, preserving default behavior and
   compatibility. Review and required CI apply to those product PRs.

This draft is not a completed registration, qualification, experiment result or
product implementation. Missing qualification and invocation commands must be
resolved before network/model calls; documentation commits do not complete #516.
