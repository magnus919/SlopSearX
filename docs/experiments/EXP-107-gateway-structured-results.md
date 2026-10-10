# EXP-107 — preserve structured results through the remote gateway

## Registration

Start 2026-10-10T13:00:49.921Z; baseline `8c69cde`, isolated worktree.
Existing issue #487. Observed code: `_extract_tool_result` ignores the SDK's
structured result field and parses display text only. Before: structured result
can be replaced by a display wrapper/empty dict; after: structured object survives
remote MCP HTTP → gateway → MCP HTTP client unchanged. Evidence class: protocol
defect replay, not ChatGPT UI support or agent-task uplift.

Reachability/readiness: existing real authenticated remote/gateway HTTP test
harness, FastMCP ToolResult and native MCP CallToolResult fields. Installed
FastMCP 3.4.7/MCP 1.29.0; existing `_field` supports SDK v1/v2 names. No search
engine/provider/Valkey/credential calls needed. Public repository-authored fixtures.

Primary: native client structured-content equality in four registered cases:
structured object plus narrative text; structured object plus different JSON
text; structured object with no text; empty structured object plus display text.
Require 4/4 candidate and at least +3 cases versus baseline. Finite exhaustive
corpus; no population uncertainty, exclusions, tuning or retries.

Controls: JSON text-only dictionary, plain text-only fallback, empty unstructured
result and error flag with structured content. Preserve legacy error conversion
and its precedence, fallback bodies, tool input schema and bearer-auth behavior.
Protocol/native round-trip correctness is the claim; production usage/frequency
unknown. Candidate only reads existing structured field after error handling and
before text fallbacks; no UI metadata, resource forwarding, rendering or dispatch
changes. #487 remains open for those other components.

Commands: PYTHONPATH=. .venv/bin/python retained HTTP replay worker for baseline
and candidate; targeted gateway tests plus server/formatter contracts, mypy,
pre-commit, full checks and normal exact-head CI/substantive review for delivery.
Default 45-minute ceiling; zero provider/model/paid calls. All raw results,
reproduction worker, source patch, environment/checks and controls under EXP-107.
No candidate implementation before this committed registration. Supported only
if primary and every guard pass; otherwise retain findings before discarding code.
Portal review: gateway-only conversion, no portal service or browser contract
change; normal portal CI remains required evidence. No SLO/alert/deployment.

Opportunity selection: coverage-ranking amendment approved, but fresh unseen
tasks/qualification/admission remain outstanding; do not repeat old blockers.
Latest completed seven reviewed: EXP-100 through EXP-106 (including terminal
continuations/private evidence directories). Historical quality failures remain
terminal; no accepted runtime ranking candidate awaits delivery. Supported
EXP-106 already merged in #733/#734; EXP-095 prerequisite only.
