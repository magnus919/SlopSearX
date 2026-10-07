# EXP-049 replay evidence

Eight single-field faults and six lifecycle controls, each through ordinary MCP
read_results/read_entities/read_result paths with disposable real Valkey.
Baseline 3d5e8ba; registration 0bfeab9; candidate c8c58e5. No data exclusions or
failed candidate trial. Baseline: 15 exceptions and nine returned evidence;
candidate: all 24 rejected with existing invalid_cursor. All 18 controls pass.

Reproduce worker.py.txt in each checkout with its project Python environment,
PYTHONPATH=., EXP049_URL pointing only at a disposable Valkey and EXP049_OUTPUT
a fresh directory. Worker cleans only keys it creates, never FLUSHDB.

comparison.json documents identity/time normalization for equal control bodies.
records_unchanged compares raw stored bytes before and after every read. No
engines dispatched. Payloads are purpose-built fault records, proving this
reachable defect behavior, not production prevalence or agent task success.

validation.json retains all checks. All-file hooks flag historical immutable
evidence whitespace, restored byte-for-byte; changed-file hooks and CI remain
the implementation gate. Source patch is inert JSON, not executable evidence.
