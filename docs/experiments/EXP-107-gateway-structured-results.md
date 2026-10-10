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

## Readout — supported and delivered

Registration `137e9c8`, baseline `8c69cde`, candidate
`6fbd0675091420397b6757e7e87816305e9f5b58`. Native structured-content equality
across authenticated upstream HTTP and gateway HTTP improved **0/4 → 4/4**
(+100 percentage points), exceeding the frozen +3-case minimum. All four
JSON/plain-text/empty/error controls match baseline exactly. No cases excluded,
primary reruns, changed thresholds or provider calls.

[Raw evidence and commands](evidence/EXP-107/README.md) retain every upstream and
gateway envelope, worker, summaries, source patch, environment, validation,
review/CI gate and verified merge. Both workers exited zero and completed every
request, but both logged the same cancel-scope shutdown error. This remains a
fixture limitation; no lifecycle reliability improvement is claimed.

Validation: 125 gateway/API/formatter tests passed, mypy passed 121 files, and
changed-file hooks passed. The full local suite completed with 2,859 passed,
58 skipped, 30 subtests and one inventory-mismatch failure; the exact test fails
on unchanged baseline too. The installed local environment is FastMCP 3.4.7 /
MCP 1.29.0; fresh normal CI passed the full Python 3.12 and 3.13 suites and all
applicable checks, including portal/Valkey gates. All-file hooks reformatted ten
unrelated coverage files, restored byte-for-byte; their diff/logs are retained
as excluded hook mutations, not candidate trials. AST graph refreshed afterward.

Droid supplied substantive positive reviews confirming structured-field
preference after error handling, empty-object preservation and unchanged legacy
fallbacks. Verified author magnus919, exact candidate SHA, every completed CI
check and review before merging [PR #759](https://github.com/magnus919/SlopSearX/pull/759)
at 2026-10-10T13:25:25Z as `032c56c673ea87da6885f2cfc19c699979a0a77f`,
without protection bypass. No deployment or release verification.

This establishes the four protocol-result corrections, not production frequency,
ChatGPT UI/host support, complete MCP App forwarding or successful agent tasks.
Issue #487 remains open for other metadata/resource/host components. Portal impact
review found no search-service/browser contract change; its normal CI gates pass.

Review-summary correction: the registration's shorthand EXP-100–106 included
prospective tooling. Latest completed outcomes reviewed are EXP-106, 104, 103,
102, 101, 100 and 99; EXP-105 is a prospective carry-summary correction, not an
accepted quality comparison. Old clocks/outcomes remain unchanged. Coverage-first
amendment approval resolves candidate choice, but fresh task registration and
admission still precede live quality calls. No accepted runtime ranking candidate
was waiting to merge. No SLO operation or alerts.

Actual elapsed accounting is retained in cycle-accounting.json. Runner live Jev,
search-provider and grader calls: zero. Subscription usage/cost unavailable; no
estimate from wall time. The day's cycle is complete with no catch-up trial.
