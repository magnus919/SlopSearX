# Biomedical candidate runtime ownership

This candidate uses one 33,554,432-byte requested-backing ledger per adapter
invocation. The allowance is an explicit resource hardening, not a guarantee
that every source-valid input fits. Every row, field and first-phase temporary
uses the same root. A refusal returns terminal Error with no partial results;
there is no truncation to fit memory and no fresh quota per field/row.

The ledger covers charged row descriptors/reference vectors, scalar/URL/join
buffers, UniProt ranking and casefold destinations, deferred publication payload
projection, final result copies, and source-sized PubMed diagnostics. Scratch
stays charged while the final copy is created. PubMed and Europe PMC finish all
source row operations before projecting retained payload fields: a later source
error still precedes an earlier nonfinite payload representability refusal.
PubMed's author payload borrows all validated raw source Text values while only
its first three names contribute to display. Payload-only surrogates retain the
accepted replacement behavior; they are not rejected by scalar display helpers.

ProducedBatch consumes payload owners without cloning, constructs explicit
SearchResult fields under charge, and transfers the response at its named
into_adapter_response boundary. A07 source-sized failure text retains a charge
until its corresponding final failure-response handoff. Fixed control metadata
and cold shared immutable Unicode-table initialization are not a global memory
claim. Returned response retention, clones, cache/flight storage and shared
admission remain G02 responsibilities.

F08 request/response/decoder/session storage and the borrowed SourceJSON input
DOM remain separate inherited boundaries. The current input codec retains its
4 MiB bytes/text, depth 100 and integer 4300-digit limits and cooperative checks;
this packet does not claim those allocations belong to the row-output ledger.
The separately accepted R2 parser ownership component is not imported here:
The earlier A07 baseline lacked its F15 dependency. The final integration
includes accepted F15 through the normal main merge; no whole SourceJSON
overwrite from the separate R2 component was made. Allocator/RSS/process containment and total cross-query residency are
therefore not established by a 32 MiB row ledger.

## Cooperative source repr and shared scalar rules

source_repr::repr_budgeted borrows the accepted SourceValue tree. Its iterative
OwnedVec event stack owns every backing reservation before allocation; object
entry references use a second charged vector, with both copies live while events
are scheduled. It neither clones the input tree nor converts it to finite JSON.

The OwnedString output grows with checked max(2*capacity+8, required). The neutral
owner reserves replacement backing while old backing is still live, then drops
old storage before releasing its charge. Actual uncovered capacity fails closed.
Thus reported peak includes stack/output/temporary overlap, not just final size.
A failed/cancelled future drops every local owner and emits no partial string.

Each event and each 1024-source-scalar scan/copy checks the optional absolute
deadline and yields. Quote selection scans borrowed codepoints cooperatively.
The shared domain PythonReprCharacter token contains at most 10 stack bytes and
uses the existing Unicode16 nonprintable table; the old domain formatter delegates
to the same primitive. Raw surrogate repr escapes use six stack bytes; scalar
emoji and separate surrogate codepoints remain distinct. Finite float formatting
reuses the accepted fixed-stack Number::from_f64 path with its 25-byte output
reservation live through the destination copy. No legacy infallible recursive
repr is called by the runtime path. The adapter's existing raw top-string branch
remains distinct from Python repr quoting; no new top-string sanitation occurs.

UniProt uses the same reviewed casefold table through borrowed casefold_char
lookups (stack UTF8 key), with exact-sized charged output and 1024-scalar checks.
Existing whole-string casefold delegates to the same lookup. All reviewed table
entries plus unknown scalar/surrogate-boundary cases are tested. Cold OnceLock
initialization is shared-process overhead for G02 to qualify/initialize.

Fixed date/f64 formatting writes into a charged, capacity-limited fmt sink. ISO
week suffixes use a 12-byte stack buffer. The sink refuses unexpected width before
requesting more backing; it does not allocate an intermediate format String.

## Deadline and test boundaries

PubMed's aggregate deadline bounds both requests, source decoding and all row/
payload normalization. With F14 context, every adapter intersects each network
budget with the caller absolute deadline; PubChem's renewal cannot extend it.
The other four legacy direct calls retain no aggregate normalization timer
(None), while still yielding and applying resource bounds. None is never a
far-future invented Instant. Negative network budgets expire; positive infinity
or unrepresentable positive values use a finite caller deadline. Direct calls
without a finite lifecycle bound refuse those positive exceptional timers before
dispatch, an explicitly approved source-success difference.

The private observer is None in production and cannot alter execution. Decode
is marked inside parse_body after F08 execute completed and immediately before
the source decoder's only awaited child. Tests observe Pending there before
forcing expiry. Inner-loop marks follow 1024 actual elements. Payload tests wait
for the projection phase and at least 32768 additional live charged bytes before
dropping or expiring the actual adapter future. They verify zero residual charges
and no partial response; observer invocation alone is not the assertion.
Loopback handler aborts are fixture cleanup, not socket-close proof. The original
separate five-engine cancellation regression owns socket-close evidence.

The 14 resource tests cover eight prepared bodies, shared multirow quotas,
first-phase failure, source-sized failure ownership, actual source payload
outputs/ordered requests, post-network decode expiry, real projection/inner-loop
cancellation, legacy timer separation and five concurrent operation owners.
Their observed maximum single-operation peak is 4,432,365 bytes (1000 Europe PMC
records). The sum of individual peaks for the five concurrent realistic controls
is 7,774,452 bytes; retained batches total 5,083,889 bytes. The reserved candidate
allowance sum is 167,772,160 bytes. These are requested-backing observations,
not synchronized process RSS or an assertion about all production workloads.

The shared repr tests include35 actual-Python codepoint-tagged controls, flat
array 16384 / object 4096 cases, quota failure and cancellation after real growth.
The 300000-codepoint escaped-text control produces 1800004 bytes with observed
3145808-byte peak. Flat-array/object peaks include their materially different
event/reference backing. Exact logs and source hashes belong to the immutable
review receipt; timing samples are observations, not hard preemption guarantees.

## Evidence and remaining integration

The original 35-control tagged-depth capture is retained separately. The native
metadata codec cannot parse its expanded depth99 tag tree, so the replay capture
uses a lossless explicit nesting recipe; all 35 expected strings and reconstructed
inputs are unchanged. The receipt records both hashes and equivalence checks.
All twelve temporary Python producers are registered as historical developer
evidence with explicit G08 retirement; they are never native runtime/A07 gate
prerequisites. The 172 source captures and accepted diagnostic ledger are immutable.

Parent review accepted the shared repr/scalar/casefold, charged payload/batch,
actual factories/gate/metadata and final provider-phase runtime components. The
normal merge includes accepted F15/W11/W02; current SourceJSON is the exact F15
main implementation, without transplanting the separate owned-parser donor.
The merged inventory retains all parent/A07 records (399 source paths and 40
oracle invocations), and CI/O06 wiring is implemented and reviewed. DCO commits
and the normal merge exist. Final delivery retains scoped documentation-head report identities and requires
PR/review/CI. G02 global/input/retention accounting and actual HTML/full
MCP transport remain the explicit shared boundaries. No provider calls or
production deployment occurred. See REMAINING_A07.md and PORTAL_IMPACT.md.


## Provider-sized phase closure supplement

PubChem's CID-reference collection checkpoints every1024 elements after source
CID conversion; Europe PMC's MED identifier digit scan uses the same Python
Unicode digit predicate with1024-codepoint checkpoints. UniProt evaluates all
source ranks before ordering, then stably partitions only ranks0/1/2 with1024-row
checkpoints. The old synchronous sort is removed. Both original and ordered
rank-vector backing are charged to the same root during their overlap; the first
is dropped before row construction. Three passes preserve source ties and source
operation error precedence without a second rank evaluation.

Actual4096-CID,100000-digit and4096-protein controls observe a pending partial
phase before its explicit completion mark, then prove drop/deadline cleanup and
no partial results (PubChem retains exactly one dispatched request). Removing
each actual checkpoint in an isolated guarded mutation makes its control fail;
production sources are restored byte-exactly. Existing172 source captures and
normalized outputs/requests are unchanged.

Raw query-plan trim/split/identifier scans are explicitly a separate G02 input
boundary. This row-output/scratch ledger does not claim a global arbitrary-query
CPU or allocation bound. F08/input DOM ownership and final retained responses
remain the previously stated shared boundaries.
