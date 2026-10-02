# Reproduce EXP-031

Baseline 26623e64c559c4db284124a941b821f7dcf02d19.
Registration 598575c; correction registration 3bd3e0b.
Copy trial2/orchestrator.py.txt to /private/tmp/exp031.py and
trial2/worker.py.txt to /private/tmp/exp031-worker.py. From a clean pinned
checkout with project dependencies run:

```sh
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp031.py
```

Requires local loopback permission. Original corrected trial exited 0. Eight
fresh subprocesses each compare both arms using unchanged MCP transport and
normal generic/targeted search paths. Fixture adapters declare consumption and
reflect accepted parameter values in their returned content; no adapter claims
actual enforcement. Actual dispatch params are retained for every pair.
Store reset gives identical cold cache state in both arms. No live upstream,
Valkey or model/paid calls. Candidate modifies only warning prose in memory.

trial1 retains the original scripts, four complete generic pairs and the first
targeted pair that stopped on a cache-state mismatch. That trial was incomplete;
it is not pooled into corrected results. No rubric or candidate tuning occurred.
Use a separate clean checkout/output location when reproducing either trial to
avoid overwriting preserved evidence. trial2 includes every fixed case twice,
raw payloads/dispatch/logs, normalized byte counts and summary. Repetitions show
repeatability, not independent people or query distributions.

Contract truthfulness is evaluated against the permitted adapter consumption
and audited-enforcement distinction. There are no current built-in language
consumers; this proves an extension-contract defect, not its prevalence in live
traffic or a measured human/agent completion benefit. candidate.patch.json and
validation.md preserve the proposed implementation and its promotion blockers.

Raw logs and the unified patch that contained trailing space padding are stored
as base64 JSON containers to preserve their exact bytes without introducing
whitespace-hook failures. artifact-encoding.json maps original filenames to
containers and original checksums. Decode the data field with base64.b64decode
and verify original_sha256 before use; this is representation only, not a new
trial or a change to the observations.
