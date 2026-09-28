# EXP-027: Literal UTF-8 cache serialization

## Registration — 2026-09-28

- Cycle start: 2026-09-28T13:01:35Z. State: registered. Owner: Codex.
- Baseline: `1b8fbbd41db0c6ea584782cce0e0b5d9e7ca2df5`.
- Observation: SearchCache.set uses json.dumps with ASCII escaping. Non-ASCII
  text therefore occupies escape sequences in values sent to Valkey.
- Distinction from EXP-026: test only ensure_ascii=False; retain default
  separators. This is not a retry with a lowered whitespace-saving threshold.
- Hypothesis: literal UTF-8 saves >=10% aggregate cache-value bytes on the
  fixed captured-card corpus without losing any baseline-cacheable response.
- Candidate: replace json.dumps(value) in SearchCache.set with
  json.dumps(value, ensure_ascii=False), injected in memory only. No fallback,
  other serializer changes, key changes, runtime files or production writes.
- Primary metric: 1 - aggregate candidate UTF-8 value bytes / baseline bytes.
  Minimum useful effect: 10%, retaining the storage-efficiency bar of EXP-026.
- Workload: same 20 rows of evidence/EXP-016/brave-acquisition.json through
  normal SearchService and SearchCache as EXP-026, with card fields preserved.
  Repeat twice with alternating arm order. Normalize only query_id and elapsed
  time for byte counting. Input SHA-256:
  bda41e595a347ec51a993ea5a3b94512f120f6a4202f2e72cf6c0608389366b1.
- Guardrail first: replay a single result through SearchService for each fixed
  title: ASCII, accented cafe (café), CJK (中文), emoji (🦑), and lone high
  surrogate U+D800. Content is fixture, URL https://example.org/encoding,
  query encoding fixture, engine brave. A captured external string may contain
  an unpaired surrogate; JSON decoding permits it and baseline serialization
  escapes it. Candidate must preserve every baseline write and cache hit.
- Zero tolerance for lost writes, repeat engine dispatch on a supposed hit,
  payload differences beyond volatile ID/time, key or TTL changes. The recorder
  uses the installed Valkey client's actual default encoder before storing bytes.
- Sampling: exhaustive deterministic compatibility cases, then 20-row corpus
  twice only if all guards pass. Exact bytes, no population CI or user claims.
- Stop at first guard violation and classify not-supported; do not run the
  remaining byte comparison or tune the candidate. Missing runtime is blocked;
  other incomplete execution is inconclusive. Supported requires >=10% plus
  every guardrail. Budget 45 minutes, no paid or upstream calls.
- Exact command: PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python
  /private/tmp/exp027.py. Retain full runner, observations, encoder errors and
  environment under docs/experiments/evidence/EXP-027/.
- Baseline numeric byte total is to be measured only after compatibility passes;
  EXP-026's total is context, not an observation of this candidate.
- Boundary: normal service/cache path, in-memory client with real Valkey encoder;
  not live Valkey allocation, network latency, expiry or production traffic.
- Portal: cache loss could change latency and redispatch; no product change
  authorized by this experiment. Supported implementation needs normal checks.
- EXP-025 remains blocked with no new corpus. No unfinished experiment can run.

## Readout — not-supported

Registration `3fdf05b` preceded the candidate. Five compatibility cases were
completed (20 service calls); ASCII, accented, CJK and emoji cases passed.
The U+D800 title failed: baseline wrote one value, served a cache hit and called
its adapter once; the candidate wrote no value, missed cache and called the
adapter twice. The installed Valkey encoder raised UnicodeEncodeError on both
candidate writes. SearchCache caught the errors as designed, so the response
remained available but cache behavior regressed. Decoded response equality alone
would have missed this regression.

The zero-tolerance cache-preservation guard failed. Per the frozen stop rule,
no captured-corpus byte comparison ran. Primary bytes/effect and statistical
uncertainty are **unmeasured**, not zero. Exact fixed-case behavior is sufficient
to reject this candidate, not to estimate production frequency or user impact.
No paid/upstream calls, runtime source edits, CI, pre-commit or implementation
PR. There were no method changes or reruns after observing the failure.

[Screen evidence](evidence/EXP-027/screen.json) and
[reproduction](evidence/EXP-027/reproduce.md) preserve the candidate and all cases.
The in-memory method was restored; only documentation and inert evidence remain.
Persist through the accompanying documentation-only PR. Any future UTF-8
candidate must preserve surrogate handling before efficiency can justify it.
