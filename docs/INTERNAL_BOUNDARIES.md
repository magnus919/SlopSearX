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
