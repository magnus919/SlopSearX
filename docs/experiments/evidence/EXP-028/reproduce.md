# Reproduce EXP-028

Baseline a8a70f580474c79c23d63a61409ea09048f41489; registration b9794df.
Cycle began 2026-09-29T13:01:41Z. Copy screen.py.txt and replay.py.txt to
/private/tmp/exp028-screen.py and /private/tmp/exp028-replay.py respectively.
From the baseline checkout with project dependencies:

```sh
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp028-screen.py
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp028-replay.py
```

Run replay only when screen passes. Both scripts completed once with empty
stderr. They inject the same candidate in memory and restore the baseline
method in finally. No production source files are changed. The candidate first
serializes literal Unicode, checks strict UTF-8 encodability, and uses baseline
ASCII escaping only if that check fails. Default separator whitespace remains.

The actual installed Valkey encoder handles values in an in-memory store.
This exercises SearchService and SearchCache, not Valkey network/expiry/memory.
The first script retains six exact service-level compatibility observations;
its captured_replay_ran=false describes that stage only. The second retains
80 captured write/hit pairs, four supplementary edge payloads, and input hash.
The primary measurement normalizes only query ID and elapsed time, identically
to EXP-026. Two passes are repeatability, not 40 independent queries.

No excluded failures, retries or changes after measurement. Package versions
are preserved in screen.json and summary.json. The corpus was already exposed;
this is an exact byte comparison on those inputs, not independent evidence of
population savings. No claim about CPU, latency, user benefit or store costs.
