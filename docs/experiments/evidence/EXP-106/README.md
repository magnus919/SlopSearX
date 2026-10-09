# EXP-106 replay evidence

Originally labeled EXP-104 before an evidence-directory ID collision was found.
Original preregistration and paths are retained; scientific protocol unchanged.

The worker drives real GET /search, SearchCache and disposable Valkey with a
fixed healthy adapter. Six non-finite score cases plus eight earlier corruption
cases, once per arm. Baseline primary 0/6 (HTTP 500); candidate 6/6 (HTTP 200).
All 14 candidate cases recover, dispatch once and repair to a healthy next hit.
Three healthy/negative-cache/malformed-JSON controls pass in both arms.

Reproduce at recorded baseline/candidate using project Python:

```sh
PYTHONPATH=. EXP032_OUTPUT=/private/tmp/exp106-new-output .venv/bin/python docs/experiments/evidence/EXP-106/worker.py.txt
```

The worker creates a disposable Unix-socket Valkey and terminates it afterward;
no existing application data is touched. Raw rows, summaries, patch, environment
and validation logs are retained. No production-prevalence or quality inference
from this deterministic injected-fault corpus.

Local full-suite stalls/timeouts are retained, including unsuccessful split and
uninstrumented candidate runs; these are not passed results. Baseline without
coverage and isolated planning transport pass. The cause of the candidate suite
stalls remains unresolved. Exact-candidate normal CI passed both full suites
and portal/Valkey gates, with substantive Droid review before the verified merge.
