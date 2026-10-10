# EXP-107 protocol replay

A local upstream FastMCP fixture returns native structured objects independently
of display text. The real authenticated MCP HTTP session and normal gateway proxy
then expose them through a second MCP HTTP boundary. Four structured cases and
four legacy/error controls run once at baseline and candidate. Primary 0/4 to
4/4; control objects compare equal across arms. Raw upstream/gateway envelopes
and worker are retained.

Reproduce in recorded checkouts with project Python and fixture helper availability:

```sh
PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-107/worker.py.txt /private/tmp/exp107-new-output
```

Both workers exit zero and finish every request, but the fixture logs the same
MCP cancel-scope shutdown error in both arms. No lifecycle success improvement
is claimed. Local full suite's one inventory-mismatch failure reproduces on
unchanged baseline; those logs remain alongside successful relevant checks.
All-file hooks reformatted unrelated sources; their preserved diff documents
what was restored, not an included implementation change.

This proves the four native structured objects survive the proxy. It does not
establish ChatGPT rendering/host support, complete MCP App metadata/resource
forwarding, agent-task success or production frequency. No external providers,
models, live search or Jev calls.
