# EXP-057 offline qualification

All ten offline checks pass. The normal CLI accepts the synthetic 80-card/80-question neutral fixture, completes all twenty-one case operations, reaches both original q1–q8 reference inventories, preserves all five navigation controls and correctly rejects neutral answers as a quality improvement. The failed CLI preserves the raw HTTP error receipt, returns complete frozen q2 W0, and leaves exactly nineteen operations uninvoked without partial quality analysis.

Nine real mechanics tests include at most two accepted swaps, rebuilt/protected state, no third call, first/second-round no-action termination, a second-round failure discarding a real first swap, no-facet and navigation bypass, parser deadlines, overflow and study deadlines. All inherited parser, budget, HTTP and owned-process tests pass. The fourteen source hashes and six artifact hashes are recorded in qualification-result.json. Output directory arguments in the public check records are sanitized; original outputs are retained privately.

This is offline contract qualification only. It does not establish candidate ranking quality, provider acceptance of every branch or production readiness. Live mode requires the exact committed qualification revision and its twenty-member manifest. Run the fresh twenty-one-case study once, serially, stop first failure and preserve all attempts; do not reuse partial EXP-056 rankings as a completed EXP-057 cohort. Brave searches remain zero.

Example (use an existing credential privately in the process environment; never print or save it):

```sh
PYTHONPATH=. uv run python docs/experiments/evidence/EXP-057/normal-runner.py.txt \
  --mode live --qualification-revision QUALIFICATION_COMMIT_SHA \
  --output-dir /tmp/exp057-new-live-run
```

The original production acceptance checklist remains required, including untouched confirmation, complete shared-service and experimental X context propagation, substantive review, applicable CI and actual implementation merges. No runtime/default/deployment/Hermes change follows from this packet.
