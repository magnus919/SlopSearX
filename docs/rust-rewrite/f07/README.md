# F07 native Valkey substrate

Implementation baseline: `7cc420f7ffb61127750e77bf6130e225925f9186` (accepted F05,
PR #960). This package supplies reusable storage and ownership mechanisms; F13,
W00–W06/W11 and identity issues still own complete workflow/model validation and
application wiring. No Python runtime implements these APIs or their gate.

`ssx_infra::valkey::Valkey` uses the pinned redis 1.7.1 Tokio driver. One operation
budget includes waiting for the connection lock, establishing the connection and
receiving a reply. Connections are reused; failed connections are discarded and
a later separately requested operation can reconnect. Commands are never retried
inside an operation. This low-level API does not replace SearchCache retry policy:
F13 must preserve its five-second post-failure reconnect suppression at the cache
caller boundary unless a separate behavior change is reviewed and accepted.
F07 makes no cache retry-policy parity claim. `close()` sets a terminal flag before waiting for the lock
and drops the sole connection handle. The canonical redis constructor owns its
driver task, which is aborted when the final connection handle is dropped. This
module never clones that handle. Canceling a request does not retract a command
already sent to Valkey. Timeout/cancellation can leave a committed write; callers
must reconcile authoritative state before another logical attempt. Shutdown does
not claim to roll back any dispatched write.

`Read` distinguishes found bytes/objects, missing, unavailable, malformed and
closed. Object reads and script-record replies use the accepted bounded,
lossless `ssx-json` representation. Null, scalars and arrays are not object reads.
Reads do not invent schemas: workflow owners validate their model fields.
`Failure::CompletionUnknown` means dispatch occurred and the response was lost;
`ScriptErrorMayHaveWritten` preserves Lua's lack of rollback, including WRONGTYPE
errors after earlier writes. Neither result authorizes an automatic retry.
Connection failure before dispatch is `Unavailable`. No authoritative write has
a memory fallback. No exception text, URL credentials or stored payload appears
in public failure variants.

All 24 research/saved/staged Lua assets retain the exact bytes from the pinned
source. `provenance.json` records their source symbols and SHA-256 identities;
`cargo xtask check F07` verifies those bytes against pinned Git objects before
running real native integration cases. Lua outcomes remain separate: research
binary flags/idempotent tuples, saved create/revision/commit/outbox/consumer
statuses and staged named statuses. Unknown codes or malformed RESP shapes are
errors. Source cjson behavior is deliberate: empty Lua tables can encode as `{}`;
this layer does not recursively convert objects to arrays. Downstream readers may
normalize only schema-declared array paths. For example, a malformed staged
record with no `state` (including `[]` decoded by Lua as a table) yields the source
`not_queued` outcome; scalar/null indexing can raise a Lua error. We retain those
observations rather than claim a successful claim or silently redesign scripts.

`keys` preserves literal tenant text for research/snapshots/saved records, and
first-24 SHA-256 tenant/full SHA-256 idempotency encodings for staged records.
SCAN glob escaping is separate from key construction. All lease operations use
F05's owner-typed `LeaseToken` and an injected `TokenSource`. Production callers
must provide fresh unpredictable tokens per acquisition; tests inject explicit
values. `retained_ttl` takes F05 wall-clock expiry/time explicitly (clock injection
without coupling time to the connection), truncates like Python and adds the
saved/staged 300-second physical retention margin. Retention never grants
access: saved/staged logical readers must reject `expires_at <= now`; snapshot
expiry keeps its separate F05 contract. Restart fixtures retain absolute expiry,
and forward/backward wall-clock fixtures demonstrate the persisted source
semantics rather than promise immunity to wall-clock rollback. Quota time comes
from Valkey `TIME` as `ValkeyQuotaTime`; command deadlines use Tokio monotonic
time. Neither is substituted for persisted authorization expiry.

## Verification

Run `cargo xtask check F07`. It starts private disposable Valkey processes through
the native integration binary. The default executable is `valkey-server` on PATH; callers may set the explicit
`SLOPSEARX_TEST_VALKEY_SERVER` executable path. CI builds Valkey 8.1.10 from the
[official source archive](https://github.com/valkey-io/valkey/releases/tag/8.1.10)
with SHA-256 `c74e50cd83f6d398a3dc570e04ac2fe538249585d021f76ee2449bbf9ebd04ed`,
verified against the [official hash register](https://github.com/valkey-io/valkey-hashes/blob/f008568f8490576f183f736cf702d26e1caa9036/README).
`scripts/rust-valkey-build.sh` checks the archive checksum and executable version
on both Linux CI architectures; no preexisting system service is used. Missing executable fails the gate.
Each test creates an OS-random UUID-v4 directory and private Unix socket, disables
TCP listening, and ignores all application/store URL variables. Cleanup kills and
waits for the owned child and removes only its own directory. Restart evidence
uses a private RDB file. There is no global flush or production connection.

Cases cover competing independent clients, real expiry/reclaim, stale renew/
release/fenced writes, reconnect and terminal shutdown, canceled in-flight close
with independent CLIENT LIST socket observation, malformed JSON/RESP/wrongtypes,
lossless large integers, research idempotency/ready discovery/cancellation,
saved revision/policy/outbox guards and lost-index repair, staged charged unknown
observations/retry conservation, actual event cursor round trips, TTL margin and
persisted restart expiry. Partial Lua writes and committed writes whose replies
time out are positive failure oracles. Full later workflow orchestration,
application/browser behavior and provider delivery are not claimed by this gate.

Portal impact: native infrastructure only; existing Python portal/runtime and
public contracts remain the active implementation. Shared runtime wiring belongs
to downstream packages and must run the portal contract gate when introduced.
