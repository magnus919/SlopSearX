# Reproduce EXP-027

Baseline 1b8fbbd41db0c6ea584782cce0e0b5d9e7ca2df5; registration 3fdf05b.
Copy runner.py.txt to /private/tmp/exp027.py and run from the baseline checkout:

```sh
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp027.py
```

The runner injects exactly ensure_ascii=False in SearchCache.set in memory.
It uses the normal SearchService fresh-read and cache-read path and the installed
Valkey Encoder with utf-8/strict defaults. A local recorder replaces storage;
there are no network calls. The method is restored in finally.

One execution completed, with an empty stderr; the rejected candidate is a
measured guard failure, not a harness execution error. screen.json retains every
case, write key/TTL, encoder error, adapter call count and hit flag. Primary
captured-corpus byte measurement was intentionally not reached. stdout.txt
preserves the original readout. No failed cases were dropped or repaired.

This fixture proves a compatibility regression for one representable Python
string accepted by the baseline cache. It does not measure its prevalence in
upstream results, user-visible HTTP behavior, real Valkey storage or latency.
A fallback or selective escaping would be a different candidate and requires a
new registration. Do not combine this rejected change with EXP-026 after seeing
its result or infer a storage improvement from the passing edge cases.
