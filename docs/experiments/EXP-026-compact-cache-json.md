# EXP-026: Compact JSON cache values

## Registration — 2026-09-27

- State: registered; cycle started 2026-09-27T13:00:59Z. Owner: Codex.
- Baseline: `c1de456402961cf7d90703a4d8acca1a005392dc`.
- Observed opportunity: SearchCache.set uses json.dumps(value), whose default
  separators include spaces. SearchService writes its canonical response there.
- Hypothesis: removing separator whitespace saves at least 10% of aggregate
  UTF-8 cache-value bytes without changing any decoded response or cache behavior.
- Candidate: only json.dumps(value, separators=(",", ":")) in SearchCache.set;
  retain default ASCII escaping, key construction, TTLs, schema and get behavior.
- Primary metric: 1 - sum(candidate bytes)/sum(baseline bytes), across the fixed
  20 rows of evidence/EXP-016/brave-acquisition.json. At least 10% saving is
  required to justify a storage-efficiency change. This is a byte claim only,
  not measured Valkey memory, latency, cost, human or agent task success.
- Inputs: replay each row's existing general.cards through a fixture adapter
  and normal SearchService, canonical cache serialization and SearchCache set/get.
  Preserve card text, URL, tier and source engine; do not invent missing cards.
  Capture is already tracked; retain input hash, do not duplicate upstream text.
  This bounded card replay is not the original full production feed distribution.
- Sampling: exhaustive 20-row fixed corpus, two repetitions, alternating arm
  order by row/repetition. No tuning, exclusions or model calls. Deterministic
  byte differences need no statistical CI; do not extrapolate to unseen traffic.
- Guardrails: exact decoded canonical payload equality except volatile query_id
  and response_time_ms; byte count normalized to fixed-width query_id and zero
  elapsed time before comparison. Same keys and TTLs; both arms must cache hit
  without a second adapter call. No result, metadata or Unicode loss.
  Supplementary edge round trips: empty object, nested null/bool/numbers, ASCII
  control/quotes, accented/CJK/emoji and lone surrogate strings. Zero failures.
- Cache boundary: in-memory setex/get recorder encodes UTF-8 like the client;
  no claim about actual Valkey allocation, expiry timing or network latency.
- Frozen stop: one candidate; stop on guardrail failure, otherwise complete all
  80 fresh-write/cache-hit pairs. Maximum 45 minutes, offline, zero paid calls.
- Command: PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python
  /private/tmp/exp026.py. Retain that complete runner as inert reproduction text
  under docs/experiments/evidence/EXP-026 with raw rows and environment metadata.
- Supported: >=10% aggregate byte saving and every guardrail passes. Completed
  comparison below threshold or with violation: not-supported. Unavailable
  runtime: blocked; incomplete comparison: inconclusive.
- Portal impact: no presentation change proposed; a supported implementation
  still requires normal cache/portal checks and review. No automatic code merge.
- EXP-025 remains blocked with unchanged data; this evaluates storage, not rank.
