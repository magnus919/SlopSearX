# Reproduce EXP-032

Baseline: 1402e287348b965fb106264cf075c73ca13225c3 (pending PR #498).
Candidate: af399bbbb90ffb0203241a971f5140b081863987 (PR #500).
Frozen registration: local experiment/exp032-registration at 9ce2043, captured
in registration.json. Each arm ran a fresh worker process and private Valkey.

From either revision's checkout with the project dependencies, run:

```sh
EXP032_OUTPUT=/private/tmp/exp032-replay PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-032/worker.py.txt
```

If the evidence directory is absent on that revision, supply the worker from
current main by absolute path. The worker requires /opt/homebrew/bin/valkey-server
and uses a unique private socket, no TCP port, no persistence, and terminates it
in finally. It sets VALKEY_URL only inside its own process; no application store
is contacted. It deletes only search keys in that disposable instance. No live
adapter/model calls: the one-result engine exercises the ordinary search path.

Inspect rows.json and summary.json. Both arms contain the same eight cases twice,
seed 32 ordering. All baseline fault requests returned 500; all candidate requests
returned 200 with correct results, one dispatch and a subsequent repaired hit.
Healthy, intentional-negative and invalid-JSON controls pass in both arms.

candidate.patch.json contains an inert base64 reproduction patch; decode data
and verify sha256 before applying to the baseline. SHA256SUMS.txt checks every
stored evidence file. No population interval applies to this fixed fault corpus.

The first full integration run found an unrelated missing content field in a
Valkey lineage fixture. Add empty content so its original assertions execute.
Final full validation with a disposable SLOPSEARX_TEST_VALKEY_URL: 2,319 passed,
2 skipped, 86.70% coverage. All changed-file hooks and 117-file mypy pass.
All-files hooks retain the historical evidence-whitespace blocker; automatic
changes to those historical records were restored to preserve their hashes.
