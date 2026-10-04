# EXP-037: Bound Jev response handling without changing ranking

## Registration (freeze before candidate work)

- State: registered
- Date / owner: 2026-10-04 / magnus919 with coding agents.
- Problem: the reachable Jev HTTP provider currently buffers its response without an application byte limit and accepts permissive JSON decoding. This exposes search requests to oversized or ambiguous provider responses. This is an engineering reliability qualification, not a relevance experiment.
- Beneficiary: all ordinary search callers through the shared SearchService; API, MCP and portal retain the same result contract.
- Hypothesis: explicit response and input bounds reject every declared hostile fixture while preserving every declared valid fixture and deterministic ranking behavior.
- Baseline: `bc03032322d3f801454eb2f8119f705e9e1fd2cb`; pinned `jev-1.13.0`, existing ten-level rubric, first-40 candidate membership, stable ties, one-second deadline, two admitted calls, no retries.
- Independent variable: provider transport/parser bounds and direct input validation only. No question wording, ranking objective, model, candidate cap, search routing or deployment change.
- Primary metric: correct outcomes / declared deterministic fixtures. Required effect: all hostile fixtures must fail safely and all valid fixtures must retain their exact expected ordered IDs. Any observed baseline defect must become a passing candidate fixture. Report baseline and candidate counts; do not claim production prevalence or statistical generalization.
- Frozen fixture families: announced oversized response; unannounced chunked overflow; multibyte overflow; exact byte boundary; malformed/nonobject JSON; duplicate object keys; nonfinite JSON numbers; missing/extra IDs; wrong model/type; bool/nonfinite/out-of-range score; oversized direct query/title/URL/snippet/request; valid sparse and ordinary scores; stable ties; semaphore queue deadline; cancellation while queued and in flight; resource release after failure; redirect rejection; exactly one HTTP attempt.
- Numeric bounds: decoded response body at most 2,000,000 bytes; existing request/query/card bounds unchanged. Queue and transport remain inside the existing one-second deadline. Async test timing tolerance may be 250 ms to avoid scheduler noise; it does not raise runtime timeout. No new retry, provider call, or credential logging.
- Guardrails: valid fixture exact permutations and stable ties unchanged; sensitive exclusion, fallback uncached, routing/cache identity, API/MCP/portal behavior and no-specialist-promotion semantics preserved. Parser changes that affect acceptance must change cache identity. Probability/score arithmetic consistency is outside scope because provider rounding semantics are not guaranteed.
- Dataset / rights: synthetic HTTP and candidate fixtures added to `tests/test_rerank.py`, redistributable project test data. Unit is one declared fixture; exhaustive finite claims only. No live searches, provider calls or fetched pages.
- Sampling / uncertainty: run each deterministic fixture against baseline and candidate; async cancellation exercises both admission states. Statistical intervals and multiple-comparison correction do not apply to finite contract assertions.
- Budget / stopping: zero external requests and spend; one implementation and qualification cycle, up to four hours including full checks. Stop adoption on any unresolved guardrail failure; preserve failures and reproduction patch. Further quality-contract studies require a separate registration.
- Commands: existing environment Python with `PYTHONPATH=.`; `pytest --no-cov -q tests/test_rerank.py`; `pytest --no-cov -q tests/test_formatter.py tests/test_server.py`; full `pytest --cov=slopsearx --cov=engines --cov-report=term-missing`; `pre-commit run --all-files`; `graphify update .` after code changes. Save exact interpreter, versions, commands, outputs and statuses in the readout.
- Evidence: `docs/experiments/evidence/EXP-037/`, including fixture inventory, baseline/candidate results and sanitized check logs. Never persist credentials or private deployment metadata.
- Portal impact: no rendering, request schema or user interaction change; shared fallback preserves ordinary results. Run existing portal contract regressions.
- Decision: supported only if primary metric is 100%, all guardrails pass, meaningful baseline defect is reproduced, substantive review findings are addressed and applicable CI is green. Otherwise not-supported or inconclusive, with candidate retained as reproduction evidence only. Merge is distinct from deployment.
- Registration commit: the first signed commit containing this document; record its immutable SHA in the appended readout rather than amending this registration.

## Readout (append after execution)

Pending.
