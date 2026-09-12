# EXP-001: Results-only MCP presentation

## Registration (frozen before candidate execution)

- State: registered; 2026-09-12; owner: Codex on behalf of maintainer.
- Baseline: `ebfdd7d` main; documentation parent `ecd261b` (PR #375).
- Observation: generic MCP search defaults to `results,engine_status`; an
  existing `include=["results"]` option omits engine outcomes. Test whether
  recommending that option universally would reduce input volume without
  removing facts needed to interpret results. No runtime edit is necessary.
- Hypothesis: results-only reduces aggregate normalized response bytes by at
  least 10% while preserving all four registered information tasks below.
- Independent variable: default include omitted versus `include=["results"]`.
  Both request `max_results=3`, explicit `engines=["wikipedia","duckduckgo"]`,
  language `en`, safesearch `off`, freshness `prefer_fresh`.
- Corpus: four fixture scenarios: both engines OK; duckduckgo RATE_LIMITED;
  duckduckgo TIMEOUT; duckduckgo OK with zero results. Wikipedia always supplies
  three results. Duckduckgo supplies three except for the zero-results case.
  FakeEngineSpec generates repository-owned synthetic results. No live engines,
  paid calls, personal data, relevance labels, or external fixtures.
- Primary metric: total UTF-8 bytes of sorted, compact JSON search payloads
  after replacing query_id, cursor, result_id, timestamps and elapsed/cache
  metadata with fixed placeholders. Preserve all field presence and all
  engine status, count, error, enforcement and result content values. Report
  raw bytes separately. Reduction = 1 - candidate_total / baseline_total.
- Guardrails: identical result URL/title/snippet/provenance, identical scope
  and enforcement, no tool errors, and all information tasks retained:
  (1) identify first result URL; (2) identify its contributing engines;
  (3) identify language-enforcement status; (4) distinguish duckduckgo's exact
  outcome and result count from explicit per-engine machine-readable facts.
  Task 4 requires exact status/count, not an inference from absence of results.
- Method: exhaustive paired replay of the four scenarios through the real MCP
  streamable HTTP server with fixture-injected engines/store. Two repetitions
  per scenario; default-first on repetition 1, candidate-first on repetition 2.
  Fresh server/store for each scenario/repetition. Record every response.
- No human/agent success or token-count claim. Byte count is a transport-content
  proxy only. Fixed-corpus arithmetic has no population confidence interval;
  repetition checks stability, not statistical sample size. No tuning or
  additional candidates. No held-out inference or production generalization.
- Decision: supported only if reduction >=10% and every guardrail passes on
  both repetitions; not-supported for a completed comparison failing either;
  inconclusive for unstable normalized results; blocked for unavailable harness.
- Stop: complete the fixed eight pairs, or stop on infrastructure failure or
  45-minute cycle limit. Guardrail failures prohibit promotion; finish this
  small offline corpus for diagnosis without exposing users.
- Commands (from isolated worktree): extract the reproduction code block from
  `docs/experiments/evidence/EXP-001/reproduce.md` into `/private/tmp/exp001.py`;
  `PYTHONPATH="$PWD" /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp001.py`.
  The script uses repository fixture server and the transport helpers from
  `tests/test_mcp_harness.py`. Capture output and responses in the evidence
  directory. Inspect diff and evidence; no runtime changes, CI, or pre-commit.
- Evidence: `docs/experiments/evidence/EXP-001/`; include reproduction instructions,
  script as inert Markdown, raw responses, metrics, environment and SHA256 sums.
- Portal impact: none; request-only MCP presentation experiment, no backend or
  browser behavior changes. Policy and cache/snapshot implementations untouched.
- Persistence: documentation-only PR for every outcome; no implementation PR
  unless evidence supports an actual follow-on change and its required checks.

## Readout: blocked (2026-09-12)

Registration was committed as `fdbb8d1` before candidate execution. The candidate
was solely an existing request option; no production code was modified.

The fixed comparison could not complete: two of eight planned pairs completed.
Attempt 1 exited 1 because sandbox policy prevented a loopback listener. Attempt
2 exited 1 when the analyzer assumed `enforcement.language` existed; the actual
explicit-engine baseline returned an empty enforcement object. We preserved the
responses and corrected the analyzer to record missing information, with no
change to the decision rule. Attempt 3 completed both healthy-scenario pairs,
then the third server emitted `ASGI callable returned without completing response`
during session initialization and stalled. It was interrupted (exit 130), per
the infrastructure-failure stopping rule. Root cause is not established.

| Healthy fixture only | Default | Results only | Difference |
| --- | ---: | ---: | ---: |
| Normalized JSON bytes per response, both repetitions | 2,665 | 2,493 | -172 bytes (-6.45%) |
| Explicit DuckDuckGo status/count available | yes | no | guardrail fails |
| Result cards, scope, enforcement equal | yes | yes | no difference |
| Explicit language enforcement available | no | no | baseline limitation |

The healthy-case size reduction is below the registered 10% threshold; the
aggregate metric over all four scenarios was not measured. Exact arithmetic
repeated in both orders; no population confidence interval or real-agent benefit
is claimed. Retaining identical empty enforcement objects is not evidence that
the language-information task can be completed. The baseline omission deserves
a separate targeted investigation; it is not caused by the candidate.

Decision: **blocked**, with adverse partial evidence; do not promote a universal
results-only recommendation or change defaults. No implementation PR. Full
comparison and uncertainty across the planned scenarios remain unavailable.
The existing opt-in remains untouched. Candidate cleanup requires no runtime
revert because this experiment changed only request arguments.

Evidence: [partial summary](evidence/EXP-001/partial-summary.json),
[reproduction code and analyzer correction](evidence/EXP-001/reproduce.md),
[final attempt stderr](evidence/EXP-001/run-3.stderr.txt), and
[SHA256 manifest](evidence/EXP-001/SHA256SUMS.txt). The evidence directory retains
all four completed response payloads, the two responses from the failed analyzer
attempt, and stdout/stderr for all attempts. Paths in stderr identify only the
local development environment; fixture identifiers are ephemeral and unauthenticated.

Persistence: this readout and ledger are submitted on `codex/exp-001-results-only`
stacked on process PR #375. No CI or review was requested. GitHub main requires
one approving review, so automatic documentation merging is blocked by repository
policy; no protection was disabled or bypassed. Local main remains unchanged
until documentation can merge safely.

Follow-up: reproduce the multi-server transport failure in isolation before
resuming the registered comparison. Independently investigate why explicit-engine
search has an empty enforcement object. Any change to the candidate or success
criteria requires a new experiment; missing scenarios must not be inferred.

