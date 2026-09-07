# Internal ownership boundaries

MCP tool handlers in `slopsearx/mcp/tools.py` own request validation, the shared
fail-closed `_enforce_policy` gate, runtime access and response orchestration.
They delegate result presentation to two modules:

- `slopsearx/mcp/result_serialization.py` builds compact cards and expanded
  records, preserving payload limits, provenance and progressive disclosure.
- `slopsearx/mcp/retrieval_url.py` classifies URL syntax and literal addresses
  for downstream handoff. It does not resolve DNS or fetch content; downstream
  retrievers remain responsible for post-resolution SSRF protection.

These projections cannot import tool handlers or MCP runtime state; the
`mcp-result-projections` import-linter contract enforces that direction. Existing
helper and contract-constant imports from `tools` remain compatibility exports.
The FastMCP registration and `state_factory` injection boundary are unchanged.

Research is divided along state ownership:

- `slopsearx/research_models.py` owns job/query records, coverage classification,
  retry state helpers and payload round trips.
- `slopsearx/research_store.py` owns durable records, cancellation flags, leases,
  ready-index scripts and reconciliation. It never executes search queries.
- `slopsearx/research.py` owns query planning and runner orchestration, and
  re-exports existing model/store names for compatibility.

The architecture-layer contract enforces runner → store → models. Tests that
replace storage prefixes or its clock must patch `research_store`, where those
values are consumed; changing a compatibility-export binding does not change
the owning module. Runner clock controls remain in `research`.
