# F01 parity and Python-boundary inventory

Issue: [#880](https://github.com/magnus919/SlopSearX/issues/880).
Source baseline: `20109f9083babd014971951e8c02565ae51c2326`.
Read the [case study](../CASE_STUDY.md) and [execution ledger](../EXECUTION.md).
This is a contract inventory. Rust implementation and parity remain pending.

## Manifests

- [Core contracts](core-contracts.json), with [readable contracts](core-contracts.md):
  adapter registrations and batch owners, config defaults, runtime environment
  readers, durable state, identifiers, clocks and search behavior.
- [Public surfaces](surfaces.json), with [wire contracts](surface-contracts.md):
  HTTP methods/formats, mounted MCP transports and OAuth endpoints, all tools,
  resources and prompts, CLI and deployment entrypoints. SDK-owned endpoints
  identify the locked SDK source; they are not inferred from server.py alone.
- [Python dispositions](python-disposition.json), with [boundary findings](python-boundary.md):
  per-file migration ownership, [interpreter/config invocations](python-invocations.json), client-only
  classification and historical source consumers. A filename suffix does not
  prove that code is inactive.
- [O03 active tooling graph](o03-active-tooling.json) and
  [native evidence contract](../O03-tooling-and-evidence.md): source-blob-pinned
  script/module entrypoints, static import and consumer edges, O07/O08 ownership,
  prospective receipt schemas, and the offline `cargo xtask check O03` gate.
- [Drift and intentional-difference ledger](DRIFT.md): required procedure for
  reconciling upstream changes while the rewrite is underway.

Each manifest pins the same source revision. Plan IDs refer to [plan.json](../plan.json)
and the published issue index. A capability has an implementation owner and
existing regression anchors or an explicitly recorded test gap. A pending parity
marker means there is no accepted native implementation evidence yet.

## Acceptance method

The orchestrator independently compares names and metadata, not just totals:

1. Recover tracked source paths from `git ls-tree -r --name-only BASELINE`.
   Compare all `.py`, `.pyw`, `.pyi` paths plus extensionless Python entrypoints
   with the Python manifest; audit dynamic loaders and historical source
   consumers separately. Verify classifications, owners and all invocation paths.
2. Import the pinned baseline adapter registry and MCP tool registry in the
   existing development environment without starting service lifespans or making
   provider calls. Compare every adapter name and every tool name, contract,
   grant, state requirement, mutation flag and policy-gate flag with the manifests.
3. Compare adapter owners against the A01–A14 source assignments in plan.json.
   Check manifest IDs are unique, owner IDs exist and test/source paths exist.
4. Enumerate HTTP decorators by path and method in server.py and workflow_portal.py.
   Separately inspect locked SDK route builders for streamable HTTP, SSE and
   OAuth mounts. Check prompts/resources against create_server registrations.
5. Compare config dataclass fields and direct environment readers with the core
   inventory; inspect helper expansion and state namespaces. Review missing,
   null, empty, denied, expired and unavailable distinctions at the source.
6. Run the existing deterministic registry and routing metadata checks:

   ```sh
   PYTHONPATH=. .venv/bin/python -m pytest --no-cov -q \
     tests/test_mcp_tool_registry.py tests/test_jev.py
   ```

The Python environment in step 2/6 is the **temporary pinned reference oracle**,
not a Rust runtime dependency or a permanent native release check. F03/F04 must
provide native inventory/replay checks and remove backend imports from final
qualification. No live providers, production data or production rollout are
needed for this inventory.

## Handoff

F02 consumes the manifests to decide crate boundaries, state compatibility,
transport policy and intentional hardening. It must account for every inventoried
capability and dynamic execution path. F03/F04 turn these contracts into native
commands and portable fixtures. Test gaps are explicit work inputs, not implied
passing evidence. Implementation issues append exact artifact results to their
capability evidence before acceptance; closing F01 does not close any parity row.

## Recorded validation

[validation.json](validation.json) records the source-set comparisons, locked
dependency versions, regression results and acceptance corrections. The locked
reference run passed 15 tests and skipped one deployment-mode test that belongs
to the dedicated CI matrix. Neither that run nor the inventory asserts native
feature parity.
