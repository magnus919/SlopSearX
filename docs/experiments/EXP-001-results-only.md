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

## Readout

Pending. Registration commit is recorded by the following readout commit.
