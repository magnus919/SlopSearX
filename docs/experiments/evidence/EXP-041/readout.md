# EXP-041 correctness readout

Functional outcome: **supported**, pending implementation CI and substantive review. Official arXiv HTML routes now use existing identifier, version and work-group rules. HTML `.pdf` suffixes, extra paths, invalid/oversized versions and deceptive hosts remain rejected. Distinct identifiers and Zenodo DOI records remain separate even when titles match. The grouping policy version changes from v2 to v3 so old canonical cached responses are not reused.

The initial baseline matrix produced 13 failures and 7 passes. After correcting the illustrative HTML regression to the exact saved version-v4 card and adding mixed-version/serialization checks, the unchanged final fixture produced 14 failures and 7 passes against the pinned old scholarly module. The candidate grouping suites passed all 55 cases. All members, same-engine vote bounds, both rankers, explicit older-version requests, stable ambiguous-version selection and public/cache round-trips are covered. Initial matrix and sanitized failure output are retained; final test-file and source hashes are recorded in qualification.json.

Full candidate validation passed: 2,333 tests, 57 skipped, seven warnings, 85.41% coverage. Type checks passed for 118 source files, portal/API contracts passed 114 tests, and pre-commit checks passed. The first full-suite attempt was unable to bind local fixture servers under sandbox restrictions; that failure is retained, and the permitted full run passed. Dedicated real-Valkey and browser jobs remain required CI evidence rather than being represented as local passes.

No Brave, Jev or engine calls and no deployment changes occurred. EXP-040's failed ranking decision, labels and receipts remain unchanged. This corrects duplicate-work identity; it does not demonstrate better downstream research answers or validate a coverage selector.

## Reproduce the old-code comparison

Run from the source checkout with the final test file, in an isolated Python process. This loads the exact pinned old module without changing worktree files:

```python
import subprocess
import slopsearx.scholarly as scholarly
import pytest

source = subprocess.check_output([
    "git", "show",
    "13acdf2cc59af764f3f71103885e4f2dd4c3bc42:slopsearx/scholarly.py",
])
exec(compile(source, "baseline/slopsearx/scholarly.py", "exec"), scholarly.__dict__)
raise SystemExit(pytest.main([
    "--no-cov", "-q", "tests/test_scholarly_arxiv_html_identity.py", "--tb=short",
]))
```

Candidate command: `PYTHONPATH=. .venv/bin/python -m pytest --no-cov -q tests/test_scholarly.py tests/test_scholarly_arxiv_html_identity.py`. Source hashes, exact qualification commands and fixture history are in [qualification.json](qualification.json).

### CI qualification follow-up

The first Python 3.13 job exposed an existing queue-fixture timing race; the fixture was made deterministic without changing the Jev runtime. The reviewer-reported experiment-ledger placement was also corrected. Combined grouping/reranking tests passed 103 cases after repair. Final CI and substantive review remain required; see qualification.json for the preserved failed-job receipt.
