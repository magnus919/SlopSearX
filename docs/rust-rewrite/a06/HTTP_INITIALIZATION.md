# Native HTTP initialization

`AdapterHttp::new_ready(startup_deadline, cancellation)` is an explicit native
startup constructor. It returns the HTTP owner `Arc` and a `Preparation` outcome
before consumers are published. `JevRouter::new_ready` uses this production
constructor. Regulatory factory qualification uses the same constructor before
real factory registration. A complete T02 native service bootstrap is still a
separate handoff; legacy constructors do not silently become initialized.

Preparation builds one independent client without a URL, request or socket, only
for the existing ordinary default pool. Environment-derived ephemeral profiles
are reported explicitly; custom TLS, identity, HTTP/2 and no-trust sessions retain
their existing cold lifetime and upstream options. Injected transports do not
initialize native clients. Request/session headers, credentials and cookies are
not copied into preparation.

An unused prepared client shares the existing ten-client adapter retention
allowance, ordinary pool identity and five-second idle expiry. Checkout moves it
once into its real origin. Preparation describes completed startup work, not a
promise that an entry remains eligible indefinitely; delayed publication does
not renew its expiry. A later cold request can still time out before dispatch.

Cold client construction runs outside the request's executor poll and slot
mutex. Its worker owns the existing adapter and pool admission permits and
reserved slot until construction completes. Cancellation never releases those
leases early or sends a late request. The same absolute request deadline covers
admission, construction, exchange and decoding. No new request quota, independent
worker allowance, retry or shared `Client` clone is introduced.

One runtime-owned startup guardian retains the unpublished HTTP owner. Receiver
closure, initialization failure or missing publication acknowledgment closes and
drains that owner. Successful initialization joins its builders before sending
the owner; a successful send alone is insufficient. The receiving constructor
checks closure, cancellation and deadline in that order, then acknowledges and
returns without yielding. `Poll::Ready` is the publication boundary. Dropping an
unread enclosing task result after that boundary uses ordinary owner `Drop`;
strict asynchronous joins require an external shutdown supervisor.

The guardian is not externally joined by a vanished caller. Its cleanup requires
a live Tokio runtime; it is not a global reaper. Operating-system proxy discovery
cannot be forcibly canceled, so verified startup cleanup can outlast the startup
deadline. Existing synchronous custom environment certificate-file resolution
remains an inherited F08 boundary. This component proves neither a global memory
bound nor a portable construction-latency bound.

The source-backed Darwin investigation observed a first request poll spending
85.1195 ms synchronously before yielding. Four fresh processes measured the exact
upstream client builder with environment trust disabled/enabled/disabled/enabled:
1.851125, 82.0965, 0.222625 and 68.197084 ms. Those controls made no HTTP calls and
logged no proxy values. They support the system-proxy construction cause on this
host, without claiming a universal timing bound or changing proxy/TLS semantics.

Qualification includes blocked-builder lease/slot ownership, deadline and caller
abort, owner closure, actual joins after canceled drain, typed initialization
failure, ephemeral/injected exclusions, prepared expiry and moved-client socket
use. Constructor controls cover both abort before publication and a successful
unread send, plus cancellation/deadline before the final receiver poll. A real
naive no-ack mutation fails the unread-send control; exact restored source passes.
The actual four-engine factory test retains its 50 ms deadline, one-request,
nonzero socket read and EOF assertions. Startup duration is reported separately.
No provider calls or timer relaxation qualify this change.
