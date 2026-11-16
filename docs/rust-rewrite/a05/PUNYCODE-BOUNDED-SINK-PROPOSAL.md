# Punycode shared charged scratch and insertion seam

Status: private implementation candidate authorized in direction; exact additive
sink API proposed here before any shared public strict facade changes. No new
codec limit, replacement inference, legacy fallback, or acceptance claim.

Existing Number is a private Vec<u32> magnitude used for delta, weight, term,
next, adapt delta and adapt extra. Source arbitrary-width arithmetic is necessary:
modulo insertion positions, bias adaptation, OverflowError and earlier Unicode
failure precedence cannot be replaced by a saturating machine integer. Preserve
one arithmetic algorithm for both legacy source regression and bounded workers.

Proposed exact additive foundation interfaces:

```rust
// Existing public output error; new distinct variant, not Resource/fallback.
pub enum Error { /* existing variants unchanged */ Deadline }

// Private sequence trait extends current private PointSink.
trait PointSequence: PointSink {
    fn len(&self) -> usize;
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error>;
}
// BudgetTextBuilder fixed capacity retains its charge. Append one checked point,
// shift only already initialized slots using copy_within, write inserted point.
// An out-of-range insertion or exhausted fixed limit returns OutputBound.
impl BudgetTextBuilder {
    pub fn try_insert(&mut self, index: usize, point: u32) -> Result<(), Error>;
}
```

Vec's PointSequence implementation preserves legacy insertion. The charged
implementation never exposes a mutable Vec or permits capacity growth outside
OwnedVec. Both share the same Punycode loop; no copied bounded algorithm.

Private Punycode mode carries Option<&Budget> and a borrowed checkpoint callback
`&dyn Fn() -> Result<(), Error>`. Legacy wrappers use no budget and an infallible
callback. Bounded workers provide the operation's deadline/cancellation check.
The existing independently owned process supervisor remains the hard execution
expiry/kill/reap boundary; a callback is an additional cooperative check, not a
replacement for that ownership or a claim about OS cleanup.

Number stores a private enum Legacy(Vec<u32>)/Charged(OwnedVec<u32>). Constructors,
copy, resize and push become fallible; reads/mutable slices, trim and division use
borrowed storage. Charged copy reserves independent backing capacity before any
copy; old and copied values coexist with separate charges. Growth delegates to
reviewed OwnedVec, which reserves old plus new capacities before allocation and
releases the old capacity only after replacement storage ownership transfers.
No guessed limb maximum, uncharged clone, bare Vec extraction or inner scratch
escape is permitted. Declining charge is an operation Resource failure, never
UnicodeFailure/email fallback or partial text success. Scratch charges drop on
normal/error completion; returned BudgetText keeps its independent output charge.

Checkpoints belong at the outer input/insert loop, extended digit loop, Number
limb add/multiply/divide/copy/resize loops and bias adaptation loop, before work or
new allocation. Failure is carried separately from source StrictError. The exact
private result is `Result<(), Failure>` with
`Failure::{Source(StrictError), Output(Error)}`; errors retain source end/offset
and Overflow precedence. A later callback/resource failure must not overwrite an
already encountered source failure. A deliberately injected earlier callback or
allocation failure is a documented native execution/resource refusal.

The source strict basic-prefix validation occurs before its output allocation.
Expose private checked prefix validation so a future bounded strict dispatcher
can run it before reserving output. The replacement body path has no prefix
Unicode rejection. Existing transport/admission bounds guarantee body insertion
count fits u32; nevertheless use checked conversion and typed OutputBound instead
of introducing a new panic at the leaf API boundary. Parameter/source1024-label
rules remain in IDNA, not new Punycode-specific input truncation.

Qualification before facade/helper wiring:

- Complete original replacement outputs and strict2255 overflow/end/error vectors;
  IDNA wrapper offsets, all-label order and1024-bound variants remain unchanged.
- Charged success retains output ownership after scratch drops; exact-capacity
  and one-byte-short budgets, forced clone/growth failures and overlap peaks.
- Deterministic checkpoint failure inside long multiplication/division/adapt and
  insertion work; exact typed Deadline, no partial result publication/fallback.
- Real slow-codec helper deadline/caller-drop kill+reap, admission reuse only after
  verified cleanup, retaining parent child allowance and payload ownership.
- Strict scoped Clippy, frozen source/input/log identities and independent review.
