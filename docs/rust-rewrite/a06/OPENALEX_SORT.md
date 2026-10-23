# OpenAlex source ordering candidate

`scholarly_order.rs` compares only values originating from the bounded SourceJSON
byte decoder. Its100level depth and4,300integer-digit limits are part of this
candidate's envelope. `scholarly_sort.rs` translates CPython3.14.7 run detection,
binary insertion, left/right gallops, low/high merges and Powersort scheduling.
It uses fallible index/scratch reservations and cooperative comparisons/moves.
No standard Rust total-order slice sort, NaN rejection or panic recovery is used.
The result is an index permutation; no result/factory is activated by these APIs.

The translated source is `Objects/listobject.c`, SHA256
`9bea0c5d2101a2619f34cab86b4895a1236e4cc19f55ce55694b5e7909bf6127`,
from the pinned local CPython3.14.7 source used by A05 provenance work. The complete
PSF license is retained in `crates/ssx-source-codecs/LICENSE-PYTHON`, SHA256
`b0e25a78cffb43f4d92de8b61ccfa1f1f98ecbc22330b54b5251e7b6ba010231`.
This is native Rust code derived from licensed source, not a Python runtime.
Final donor reconciliation must retain that license even if codec packaging moves.

The source appends raw positions directly to `(position,word)` tuples. Quoted
positions remain strings; relevance-score float conversion happens later and
cannot change sort keys. CPython json.loads returns the same canonical NaN object
for every rawNaN constant token. Tuple/list/dict equality uses its identity fast
path, so two decodedNaNs can reach a word tie-break even though direct numeric
NaN equality is false. The engine-local comparator models that creation path;
it does not redefine all independently constructed/computedNaN equality.
Nested containers are distinct after JSON decoding; shared strings, null, booleans
and integers have ordinary value equality regardless of identity.

The172complete whole-adapter controls retain their34/42/88record prefixes.
The additive direct-helper corpus stores full reconstructed abstracts or exact
source exceptions for the same172IDs; whole-adapter content alone is sliced at
500characters and cannot qualify a permutation's tail. Native tests require all
172full-order IDs and a separate exact69ID two-pair comparator set. Numeric controls
cover2^53/2^63 boundaries, largestfinitefloat integer equality±1, negativefractions,
signedzero and4,300digitintegers in both comparison directions.

This is focused source-operation evidence. Broader full-sort stress/negative
controls, accounting for pair/index/scratch/future allocations, full OpenAlex
consumer integration and real outbound date-boundary/proof evidence for issue984
remain required. The helper/resource/factory acceptance boundary is unchanged.

R2 focused qualification adds84actual-source controls: finite/reverse/random/
alternating/NaN run shapes at3–1,024positions and late mixed-type failures.
All172full direct abstracts/errors and500character adapter slices match native.
Test-only branch counters observe low/high merges and both inconsistent-order
exhausted-side paths:138/68merge entries,24low-A-exhausted and24high-B-exhausted
paths. Native1ms deadline controls interrupt a million-character comparison inside
left gallop and a million-index overlapping copy. Before pending.push, an explicit
capacity guard prevents implicit growth even if a stack-invariant regression
occurs. Boxed recursive comparison-future allocation accounting remains pending.
These observations qualify operations, not the full resource envelope/factory.
