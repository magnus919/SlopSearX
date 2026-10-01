# EXP-030 path audit

Baseline: 058575daa037ed20737a9ca0ecfdf4f2149838ae. Registration: 5e31742.
Copy audit.py.txt to /private/tmp/exp030-audit.py and run python3 from the pinned
checkout. One execution exited 0. audit.json inventories all tracked production
Python source hashes, definitions, calls and attribute references.

One set_error definition exists in cache.py; no direct calls or attribute
references exist under slopsearx/engines. SearchCache is constructed by normal
build_context startup. Manual review of service.py cache writes confirms the
active path uses cache.set for canonical responses and skips all-unresponsive
writes. Existing set_error tests invoke the helper directly; they are not
normal-path writers. Negative sentinel reads do not establish a current writer.

AST analysis does not prove absence of reflection or external consumers. The
claim is limited to tracked application's explicit production paths. No external
consumer is available as a replay baseline in this cycle. No runtime benefit
or frequency of invalid configuration was measured.

Resume only if new evidence identifies a current negative-entry writer and a
reproducible relevant failure. Creating a new negative-cache policy requires
its own hypothesis, shared-service/portal review and compatibility analysis.
Do not tighten startup validation for this dormant mechanism based solely on
constructor tests or repeat this unchanged path audit every day.
