# EXP-002: Empty-query URL normalization fast path

## Registration — 2026-09-19

- Baseline: main `d0aed40`; no unfinished experiment or overlapping issue/PR.
- Problem: `_normalise_url` builds and filters a parameter dictionary and
  replaces the parsed query even when the query is empty. Avoidable local work
  could reduce service latency without changing search results.
- Candidate: immediately after successful urlparse, return
  `urllib.parse.urlunparse(parsed)` when `not parsed.query`; retain the original
  malformed-URL fallback and the full existing nonempty-query path.
- Primary outcome: mean paired reduction in elapsed time per
  `SearchService.search` call across the equally weighted fixed corpus below.
  Useful effect: at least 10% AND 0.2 milliseconds per call. Require the lower
  95% paired-block bootstrap bound to exceed both thresholds. Small microsecond
  savings alone do not justify a production optimization in this cycle.
- Corpus: three engines with 20 results each. Families: no query, mixed
  (alternating query/no query), all tracking queries. URLs are synthetic
  `https://example.org/{index}` with optional `?utm_source=fixture&item={index}`;
  identical feeds exercise deduplication. Real shared service and presence ranker,
  fake offline adapters, no cache/rate limiter/router/catalog, explicit engines.
  This measures the uncached local service path, not network or user latency.
- One candidate, no tuning. 40 paired blocks per family, 20 calls per arm/block,
  10 warmup calls per arm/family. Randomize arm order with seed 20260919.
  Bootstrap 5,000 draws of 40 blocks within each family with seed 20260920;
  combine equally weighted means. Percent reduction is 1-candidate/baseline.
  Blocks characterize local timing variability, not independent users/queries.
- Guardrails: full serialized response equality apart from query_id and elapsed
  response_time_ms on every measured call; zero errors; each family's mean
  candidate time <=105% of baseline. Also exact URL-normalization equality on
  uppercase schemes, fragments, empty query delimiters, tracking parameters,
  repeated/blank parameters, relative paths, malformed brackets and surrogates.
- Decision: supported only if both lower bounds and every guardrail pass;
  not-supported if the completed measurement misses either useful-effect
  threshold or any guardrail. Inconclusive if uncertainty overlaps a threshold
  while its point estimate clears it; blocked if execution fails.
- Stop after fixed corpus, on correctness failure, or 45 minutes. No production
  edits, external calls, paid APIs or personal data. Inject the candidate helper
  in the isolated benchmark process so normal service dispatch/ranking still run.
- Commands: extract `reproduce.md` Python block to `/private/tmp/exp002.py`;
  `PYTHONPATH="$PWD" /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp002.py`.
- Evidence: `docs/experiments/evidence/EXP-002/` includes exact inert reproduction
  source, raw block timings, edge cases, environment, analysis and checksums.
- Portal impact: shared normalization may affect ordering/deduplication. This
  experiment changes no shipped behavior; a supported implementation would need
  existing merger, service, HTTP/MCP and portal contract tests plus required CI.
- No implementation PR without supported evidence and normal implementation
  validation. Persist every outcome through a documentation PR.
