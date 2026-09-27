# Reproduce EXP-026

Baseline source: c1de456402961cf7d90703a4d8acca1a005392dc.
Registration: e962d38. Cycle start: 2026-09-27T13:00:59Z.

Copy runner.py.txt to /private/tmp/exp026.py. From the baseline checkout run:

```sh
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp026.py
```

Dependencies and the captured input SHA-256 are in summary.json. The runner
replays existing cards through SearchService and SearchCache, injecting only
compact separators into SearchCache.set in memory and restoring it afterward.
No source file is edited. A UTF-8 setex/get recorder stands in for the store;
there is no live Valkey measurement. The normalized second serialization makes
query IDs and elapsed times identical for exact byte comparison.

One execution completed with no stderr, all assertions passed. rows.json
records each query/repetition, normalized value hashes and byte counts;
families.json groups those same observations. stdout.txt retains the original
readout. Two repetitions are repeatability evidence, not independent queries.
No failures or trials were excluded; no candidate was tuned after measurement.
