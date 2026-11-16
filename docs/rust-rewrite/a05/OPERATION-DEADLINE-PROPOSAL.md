# Inherited HTML lifecycle deadline amendment (not adopted)

Status: source/native timing comparison and proposed contract for parent review.
No adapter deadline behavior is changed by this document. Original captures and
separate HTTP request timeouts remain evidence, not normalized away.

## Current anchors

| Layer | Exact current location | Observation |
| --- | --- | --- |
| Python Google | engines/google.py:47-67 | Rate admission precedes construction. `num=str(max_results)` is computed before start_time. Client timeout is timeout_ms/1000, applied to its one GET. There is no adapter-owned parse/format total deadline. |
| Python Google | engines/google.py:81-88 | Decode/challenge classification precedes proxy success; parsing follows success. There is no second Google HTTP request. |
| Python DDG | engines/duckduckgo.py:60-93 | Rate admission precedes client setup/start_time; one search-local client is reused for bootstrap and endpoints. |
| Python DDG | engines/duckduckgo.py:200-218 | Bootstrap clamps milliseconds with min(value,5.0), divides by1000, passes that separate request timeout and swallows bootstrap exceptions. |
| Python DDG | engines/duckduckgo.py:95-151,284-288 | Primary and eligible lite fallback are separate POSTs under the client per-request timeout. Primary HTTP timeout/rate limit is terminal; blocks/walls/parse failures can allow fallback. |
| Native Google | adapters/google.rs:75-82,126-129 | HTML admission gets now+timeout; after request formatting a new now+timeout becomes HTTP/helper deadline. Admission/format time can therefore be added again. |
| Native Google | adapters/google.rs:160-175 | Decode exchange precedes success feedback, then parse exchange. Both currently use the latter deadline; parse/decode helper deadline strings map through Error. |
| Native DDG | adapters/duckduckgo.rs:114-132,151-153,262-264 | Admission and initial Session each get fresh deadlines. Bootstrap gets now+5ms cap. Each endpoint resets Session to now+timeout, and decode/parse use that endpoint deadline. |
| Native accounting | html_budget.rs:23-43 | Operation owns admission/query/accounting but not deadline. Queue expiry currently returns an error string subsequently classified Error. |

Paths in the native rows are relative to crates/ssx-infra/src. Source pins remain
those in the original A05 source inventory. Source HTTPX timeout is a per-request
phase timeout; it is not equivalent to a total wall-clock search deadline.

## Proposed contract

Create one HTML operation deadline after successful existing F10 rate admission,
before HTML admission and formatting. F10 retains its accepted independent
bounded/cancellable admission contract and source-first decision ordering; this
amendment does not claim its elapsed time is inside the HTML operation timer.
The HTML queue, form/config formatting, HTTP work, decode, feedback, extraction,
borrowed wire validation and projection all consume the remaining operation time.
If the parent instead wants F10 elapsed time included, that requires an explicit
additional choice and timeout/decision precedence controls; it is not silently
included in this proposal.

Operation stores that deadline. Google uses it for its one Session, decode and
parse/extract exchanges. DDG preserves each source request subdeadline as
`min(operation_deadline, now + request_timeout)`. Bootstrap similarly uses
`min(operation_deadline, now + source_bootstrap_duration)`; its own5ms timeout is
still ignored when operation time remains. Endpoint/fallback setup never creates
new total time. DDG still considers fallback only for its existing retriable
outcomes and text mode, then checks remaining operation time before any new POST.

This intentionally reduces DDG fallback's available time: source can spend nearly
one timeout on primary and another on lite, plus bootstrap; a timeout_ms total
HTML operation cap permits only the remainder. Source permits unbounded local
parse/format CPU relative to its HTTP timeout; native work becomes deadline-owned.
These are timing hardening differences requiring approval, not source parity.
No configured per-request timeout is extended and no additional request is added.

Introduce typed lifecycle failures, not string matching. Deadline is terminal
EngineStatus::Timeout with source-style absent error_message; resource refusal
is terminal EngineStatus::Error with the approved sanitized resource reason;
source codec/parser/config errors retain their exact stage/message and existing
fallback eligibility. A local hardening failure must not be mistaken for a
source parser error and trigger another frontend. Admission queue expiry changes
from current native Error to Timeout explicitly.

Feedback proposal: bootstrap remains feedback-free. DDG local failure after an
endpoint was dispatched keeps its existing failure-feedback position, then
returns terminal Timeout/Error for hardening; refusal before dispatch has no
endpoint feedback. Google decode failure before success has no proxy success;
challenge still reports failure; after successful decode/challenge checks,
success remains before parse/extract, even if later deadline/resource failure
returns Timeout/Error. Never retract or duplicate success on the second helper
exchange. This is a proposed new-hardening policy, not a source observation for
an event that the unbounded Python parser could not produce.

Budget allowance reservation is immediate/fallible; waiting on the same
operation's exhausted quota could deadlock its retained buffers. The child
ProcessPool admission queue uses the inherited deadline. Cancellation during
that wait drops the original pending reservation; if a child was spawned, the
common supervisor clone retains it through verified kill/reap. Unknown cleanup
keeps accounting poisoned/retained. No new relative wait timer is introduced.

## Patch shape and qualification

1. Add `deadline: Instant` to Operation and typed start errors for Deadline,
   Resource and AdmissionClosed. Preserve finite max256MiB policy separately.
2. In each engine create the deadline once immediately before Operation::start;
   use it thereafter. Remove Google126/DDG132/DDG262 total-timer resets. Keep
   explicit min() subdeadlines for bootstrap and endpoint requests.
3. Wire v2 typed Decode/Parse/Extract failure responses so hardening is terminal
   without matching diagnostic strings; keep source errors/source fallback.
4. Pass deadline checks into private row size/validation/projection. Whole-field
   copy/UTF8 work has a finite frame bound but needs separate latency evidence.

Before adoption: controlled-clock queue+format elapsed tests, bootstrap cold
expiry versus short-subdeadline recovery, eligible primary consuming most total
budget then lite remainder, terminal primary429/timeout with exact request count,
Google decode→success→parse timeout ordering, and cancellation during pool wait
and active CSS/codec work with actual PID reap and charge/permit release. Retain
original source timeout captures; append actual source request-timeout metadata
and loopback elapsed controls for the deliberate reduced-fallback difference.
