# Payload-only source projection

The additive `adapters::source_payload::project_field` implements the source
`_json_safe` operation for one retained top-level `build_payload` data field.
Its initial depth is1: the compact data dictionary is depth0. At depth>100,
any value becomes `<max depth exceeded>` before scalar or container processing.

Each UTF16 surrogate codepoint becomes one U+FFFD in both values and nested
object keys. Raw adjacent surrogate codepoints are not recombined; a scalar
emoji stays a scalar emoji. Key collisions after sanitizing preserve the first
insertion position and final value, matching Python's dictionary comprehension.
Nested nulls, false, zero, empty containers and order remain intact. The caller
drops only top-level None data values before this operation and constructs
provenance from the original retained field names in their original order.

Ordinary `Value::project`, result title/content, JSON parsing and text storage
remain unchanged. Nonfinite numbers still return an explicit typed projection
refusal; they are never silently replaced with null. Source `_json_safe` itself
retains nonfinite floats. Consumers must preserve the existing approved native
finite-output boundary and finish source operations before applying a pending
typed refusal. This component does not resolve that boundary globally.

The seven controls derive from actual pinned CPython3.14.7 `build_payload`
execution captured by A07. `source-payload-controls.json` records source hashes,
codepoint-tagged inputs for the four Unicode/order cases, exact list-depth
recipes for three depth cases, full expected data JSON and provenance field
order. It binds the original supplemental packet SHA256
`fe7593e1c308f1c64f3b1ac261d256058a6cabc77f63fa5ae378b2648ae50ebc`.
They are direct source helper controls, not seven complete adapter observations.

The original synchronous project_field is a semantic compatibility component,
not an aggregate allocation or cooperative CPU/deadline boundary. Strings are counted then copied;
arrays/maps are copied recursively through the fixed source depth boundary.
Runtime consumption requires supervised/charged ownership already being built
for A06, or separately qualified bounded ownership. No uncharged global clone,
factory activation, GNU qualification or retained-result memory claim follows.


## Charged component candidate

`project_field_budgeted(value,budget,Instant)` retains the existing convenient
finite-deadline signature. `project_field_budgeted_optional(value,budget,
Option<Instant>)` preserves legacy parse paths with no deadline: it still yields
and enforces the same caller ledger. No new far-future/network deadline is made.
Both return borrow-only BudgetPayload (private Value before private Charge),
without Clone, mutable-charge exposure or bare Value extraction. Mandatory
consumers must not clone through borrowed public Value references.

Every output array and newly empty locked IndexMap backing layout is charged
before fallible reservation. The map formula is pinned to IndexMap2.14.2 and
hashbrown0.17.1, checked64bit Value72/align8 layout, x86_64 SSE2 Group16 or aarch64
NEON Group8. n29 hash bucket rounding is64, not60. Local DarwinARM requested
allocations/capacity/calls/peak/drop match22 independent controls; GNU remains
pending. Key/value strings are separately counted and copied with scalar-count
checkpoints. Both recursive future boxes are precharged and drop inner before
charge. Fixed-stack Number::from_f64 is precharged25; integer RawValue+retained
number tokens reserve twice token length and map errors rather than expect.
The source parser accepts4300 integer digits and rejects4301 before projection.
Discarded duplicate values/keys and temporary validation tokens retain their
conservative reservations until root disposal; earlier resource refusal is
possible and is not described as exact live-size accounting.

Source text is limited to the existing MAX_TEXT4,194,304 codepoints before scan.
Arrays/maps are bounded by the shared finite budget and depth100 sanitation.
Map insertions hash/compare synchronously between before/after checkpoints.
Actual maximal keys16,777,216UTF8 bytes and sanitized collisions12,582,912bytes
produced about30–40ms executor gaps in local debug qualification. This is finite
measured cooperative latency, not hard deadline preemption or a proven GNU bound.
Allocator excess capacity is detected after allocation; allocator metadata,
Budget Arc/Mutex and runtime baseline are outside requested backing accounting.
Precharged Box/serde allocations may still encounter system OOM (process/overall
containment qualification remains separate), not a guaranteed recoverable error.

`publication_budgeted` builds only the native science/publication envelope,
with schema1 and empty normalized/inferred field lists used by these adapters.
Borrowed FieldRef::Value/Text/SelectedTexts avoids raw source/author clones.
Top-level None drops before sanitation; duplicate descriptor names follow first
position/last value, including lastNone dropping a field. This allocation-free
name walk is quadratic and checkpointed, not a performance improvement. Complete
actual PubMed and EuropePMC payload controls bind raw author codepoints, nested
key sanitation, metadata and insertion order. Two later-source-error records in
the same preserved A07 packet remain for consumer precedence tests, not extra
envelope replays.

ProducedBatch takes the SAME Budget and an exact retained-row capacity bound,
with fallible checked preallocation and no implicit Vec growth. Borrowed Skeleton
strings are copied under charge while caller-owned original builders remain live;
this does NOT retroactively account prior formatting/joins/source parser storage.
Callers use accepted OwnedString/OwnedVec for those first-phase allocations and
finish all source operations before applying pending projection refusals. Empty
engines BTreeSet/media/image/work_group fields allocate no storage; defaults are
constructed explicitly, avoiding hidden score/category duplication. Scores and
latency each reserve25 before typed finite construction. Error diagnostics are
copied under charge; non-success handoff clears partial results.

The named into_adapter_response moves result storage without clone/reallocation
and ENDS batch/operation accounting at return. Public returned response/clone,
cache/flight/snapshot/caller lifetime overload limits are REQUIRED G02 work.
There is no global retained-result or serialization claim. The shared-ledger
reply-buffer overlap control tests simultaneous reservation while the batch is
alive; it is not qualification of a serialization implementation. Root policy,
source-row scratch, actual helper wiring, GNU containment, aggregate memory and
full A06/A07 factory acceptance remain separate requirements.
