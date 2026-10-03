# EXP-033 evidence

Run worker.py.txt with the repository Python environment and PYTHONPATH=. at
baseline ad03963 and candidate f4ab41d486f77725304aa1b58dac9c03fb5ae0f3:

```sh
EXP033_OUTPUT=/private/tmp/exp033-replay PYTHONPATH=. .venv/bin/python /path/to/worker.py.txt
```

Uses fake adapters through normal HTTP and MCP boundaries. No paid/provider
calls. First worker setup failed before measurement due to a mistyped fixture
class import; corrected to _MockEngine and explicitly froze source categories
for the unrelated-source control. First measured candidate missed the early
MCP invalid-scope advisory (13/14); trial 2 and final pass 14/14. Full regression
then exposed three sensitive-policy error metadata violations; candidate fixed
them by retaining the original metadata-free policy rejection.

comparison.json excludes listed volatile identity/timing fields and advisories
from payload equality. It also verifies exact adapter-call equality. These are
contract measurements, not agent or human relevance/task-success measurements.
