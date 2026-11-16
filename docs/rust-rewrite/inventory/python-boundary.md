# Python source and interpreter boundary

Baseline: `20109f9083babd014971951e8c02565ae51c2326`
Schema: `1.0.0`
Machine-readable inventories: [python-disposition.json](python-disposition.json) and [python-invocations.json](python-invocations.json).

The source manifest compares exact tracked path sets: `.py`/`.pyw`/`.pyi` files plus extensionless `ssx`. At this baseline it lists 363 paths and the sorted-set comparison passes: 122 backend/runtime, 240 backend-coupled tooling/test, and one wire-only client. Each source has evidence markers, a primary plan owner, related owners, and a disposition.

`ssx` is standard-library HTTP code with no backend imports, embedded server, FFI, or subprocess fallback. It is architecturally client-only; T08 still requires replacing its public command with native Rust. O02 owns `docker/healthcheck.py`; O05 owns test conversion; O06 prepares CI and pre-commit successors; O09 activates replacement gates.

## Active interpreter and toolchain paths

`python-invocations.json` records actual or configured Python interpreter/tool commands, including shell/workflow commands, Docker health/server argv, executable shebangs, and mandatory pre-commit hooks. Grouped config records cover setup-python selectors, pre-commit hook/tool declarations, pyproject runtime/dev requirements and entrypoints, full requirements/uv locks, and Docker/Compose/Kubernetes image configuration. Actual commands are retained individually; lockfiles are represented by whole-file evidence to avoid repeating every package row.

## `.py.txt` historical evidence versus active dynamic source

The appendix inventories all 729 tracked `.py.txt` paths with historical status and Git modes. Of these, 728 are non-runtime historical documentation/evidence artifacts; eight have Python shebangs and all are mode `100644`. Documented run references are recorded separately. File mode and suffix alone do not establish that code cannot be run; documented commands are distinct from automatic runtime invocation.

One path is active as dynamic Python source: `coverage-first-study/w0-rerank-v1.py.txt`. The offline legacy replay loader checks its exact hash, compiles it, and executes it in a private module. `coverage_selector_input_map.py` calls that loader, and `coverage_local_authority.py` supplies the bytes. `coverage_operator_runner.py:613` only reads and verifies the bytes. Its `:742` compile checks ordinary loaded `.py` module code for equivalence; it does not execute the `.py.txt` artifact. O08 must port the replay or retire its consumer before the artifact can be treated as data-only historical evidence. Full paths and line anchors are in `python-invocations.json`.

## Final boundary

- `backend/runtime`: Rust successors required for server, adapters, MCP, portal, health, and shared state. Runnable old backend source may remain only in Git history after retirement.
- `backend-coupled tooling/test`: mandatory build, test, acquisition, evidence, review, release, and operations tooling moves to Rust or ordinary shell before its gate transitions. This includes the current dynamic replay source consumer.
- `client-only`: only wire-only clients with no backend import, embedded server, FFI/PyO3, subprocess fallback, or mandatory-gate dependency. `ssx` qualifies architecturally but receives a required native successor.
- `historical evidence`: preserve original bytes and record actual modes/references. The active replay input is separately tracked until its consumer is removed.

## Drift check

At each rewrite merge, fetch upstream `main`, regenerate exact tracked Python path sets including extensionless shebang scripts, scan active command/config/deployment paths and pre-commit hooks, reconcile dependency/entrypoint declarations, and inspect documentation examples plus code consumers. Compare exact path sets and Git modes; do not infer runtime status from extensions. Update the baseline SHA and change/capability ledger whenever the source revision changes.
