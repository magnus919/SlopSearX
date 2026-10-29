# T06 EOF fixture follow-up (#1070)

The Ubuntu prerequisite job [123497884479](https://github.com/magnus919/SlopSearX/actions/runs/40888307249/job/123497884479)
for PR #1066 at `88176fee9d58bd555c2bf8558e116ca2a116c8af` failed
`actual_stdio_eof_during_optional_prepare_and_preview_drains_owned_process`
with `Elapsed(())` (8 passed, 1 failed). Its test blob
`a38800c2733c686b7a01157a76878f86d2b739f5` is identical to the
`30270aaa7c459cec6404f4f015fa17077704c2a0` starting main revision.
The failure does **not** establish which wait expired, a production EOF defect,
or a date-filter regression. The retained read-only diagnosis is SHA-256
`5523e14d6c550c49eee002b15038bb79ac5eb6029f942827ff09883561d99495`.

The original success path already cancelled and joined the provider. Early
Result returns bypassed that teardown and relied on child `kill_on_drop`
without awaiting retirement. Separately, the provider emitted a readiness
permit after partial HTTP input reached EOF. This follow-up repairs those
source-proven weaknesses without claiming they caused the hosted timeout.

The loopback provider now accepts readiness only after the complete actual Jev
POST request: bounded headers (8 KiB), a single valid Content-Length, bounded
complete body (64 KiB including headers), and the expected JSON request shape.
Incomplete EOF, malformed or ambiguous framing, oversized input and a stopped
provider cannot create a readiness permit. Cancellation reaches an incomplete
read as well as accept and the pending-response wait. Teardown cancels and
joins the provider; a failed cooperative join aborts and awaits its task.

The EOF exercise owns the real qualification child, stdin, bounded protocol
reader, continuously drained stderr, loopback provider and private Valkey.
Every Result path performs teardown before reporting the original failure.
Child status and cleanup failures are reported separately. Phase and tool names
identify initialization, protocol failure, early child completion, request
readiness, stdin EOF, process retirement and peer-close observation. Protocol
reads allocate at most 16 KiB plus one byte; diagnostics retain byte counts
and actual child status without printing arbitrary payloads, headers or stderr.

All execution bounds remain unchanged: initialize observation is one second,
complete-provider-request observation is three seconds, EOF child retirement
is five seconds, and socket-EOF observation is one second. The qualification
binary retains both ten-second optional-provider deadlines. No production
provider, policy, API, filter, gateway, registry or portal behavior is changed.
The EOF contract still requires process success, exactly one observed provider
socket EOF and zero engine dispatch receipts before teardown.

The finite native T06 service inventory includes three additional controls:

- Incomplete headers/body, missing/duplicate lengths and oversized input must
  fail readiness; cancellation of an incomplete connected peer must join the
  provider and close its socket.
- Injected errors before initialization and while the real provider request is
  pending must preserve the primary failure, reap the child, join the provider,
  remove the private store directory and retire the loopback listener, for
  both search preparation and scope preview.
- A mismatched initialize reply identity and a real early child exit must produce
  different protocol/child-status diagnoses and complete teardown.

The exact named test and complete source-bound T06 gate remain required.
Local qualification and mutation-control outcomes are recorded with source,
commands, binary hashes and exits in the bounded task evidence packet; hosted
acceptance remains a separate obligation. This is a qualification-fixture
parity repair with no new intentional public behavior difference. Portal impact
is none: the changed code is test-only and the native finite gate inventory;
the required hosted portal gate remains part of delivery.
