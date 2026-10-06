# Query-planning capability delivery — 2026-10-06

The maintainer explicitly authorized implementation and merging after EXP-079–081. [Issue #682](https://github.com/magnus919/SlopSearX/issues/682) defined acceptance criteria. [PR #685](https://github.com/magnus919/SlopSearX/pull/685) delivered the caller-directed capabilities; it merged as `f244683716c74ccc25869fa94019ce2d36628351`.

## Delivered behavior

- `slopsearx_plan_research`: zero-dispatch preview retaining the original plus named caller-defined evidence needs.
- `slopsearx_plan_query_variants`: optional zero-dispatch terminology variants, bounded and identifier-preserving; no automatic default or quality guarantee.
- `slopsearx_plan_research_followup`: zero-dispatch evidence-linked proposals with terminal parent/admitted result checks, live snapshot/source policy, duplicate/progress/budget rejection and ready continuation arguments.
- Durable planning/reference metadata and server-derived evidence scope revalidation, nullable legacy compatibility, preserved legacy continuation digests, and execution-time duplicate checks for planning-tagged requests.
- `plan_research_with_evidence` prompt, direct/gateway parity, public docs and policy-guarded portal provenance.

Runtime code does not call a model. The agent caller owns decomposition, query choice and sufficiency. No ordinary HTTP defaults, adapter/ranking behavior, provider credentials, deployment or automatic quality claims changed. EXP-079/080 remain exposed fixed-corpus reading-lead evidence; EXP-081 remains inconclusive. The validated optional expansion interface is not adoption of a measured expansion-quality improvement.

## SHA-bound verification

Final reviewed PR head: `685758a455b75de915fc82fca6181fccfd4bdaba`. Runtime was unchanged from the corrected execution head `5ad0705facd8886d5fefc3f888faa0082e9332b1`; only the prompt inventory changed afterward.

Local verification: 2,438 tests passed with disposable real Valkey, 86.81% coverage; three Chromium portal journeys passed; all-files pre-commit (lint, strict typing, dead-code and architecture layers) passed. Nullable metadata fixes passed 86 affected tests; execution duplicate fixes passed 118 affected tests. `graphify update .` regenerated the AST graph with no model/API calls.

Complete CI passed on the final head: Python 3.12/3.13, Valkey integration, SearXNG compatibility, portal contract/browser, keyless/enhanced Jev modes, lint/types/analysis, duplicate detection, monitoring, CodeQL and amd64/arm64 builds. Conditional publish/tag jobs were appropriately skipped.

Droid found the execution-time duplicate gap and stale prompt inventory. Both were fixed and all threads resolved. [Final substantive review](https://github.com/magnus919/SlopSearX/actions/runs/37482500303): “LGTM — no issues found”; explicitly confirms both prior findings are addressed. Merge used an exact-head match and preserved signed commits. The merged main state passed 166 planning/transport/registry/portal contract checks. The temporary validation Valkey was shut down.

## Rollback and operations

Revert the feature PR to remove the interfaces. Drain jobs containing additive planning metadata before an older worker deployment; older workers cannot enforce the new metadata checks. Stored evidence retains existing TTLs. Research tools retain `MCP_GRANT_RESEARCH=1` and current specialist/sensitive-engine grants. Repository delivery is verified; no production deployment is asserted. Usage and error contracts are in [QUERY_PLANNING.md](../QUERY_PLANNING.md).
