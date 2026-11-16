# Codec allocation inventory (qualification incomplete)

This is an implementation inventory, not an aggregate heap bound. The output proof in CODEC-EXPANSION-BOUND.md establishes at most B returned codepoints for B HTTP body bytes; it does not establish peak capacity, allocator overhead, or process RSS. Caller raw body and headers, child framing, returned codepoints, UTF8 conversion, parser trees, response frames and parent projection must be charged for their actual overlapping lifetimes.

The current leaf has no lazy heap-backed mapping cache: single-byte, CJK, strict-validity, Unicode name, IDNA and label-classification data are static Rust arrays or include_bytes slices. These contribute mapped/read-only executable pages rather than request-owned Vec allocations. Their process RSS still belongs in process/container qualification. Tests' BTreeSets and decompression do not execute in the production decoder.

| Production path | Owned allocations beyond caller input | Outstanding accounting work |
| --- | --- | --- |
| Label lookup | normalized String; dotted fallback replacement String while normalized remains live | Charge capacities, including attacker-controlled label length, before allocation. |
| Single-byte | collected UTF8 String from mapping, then Text Vec while String remains live | Charge both capacities; replacement U+FFFD can require three UTF8 bytes per source byte. |
| CJK/HZ/ISO2022 | output Vec initially reserves B u32; ISO action uses tiny temporary point Vec | Bound temporary action capacity and allocator overhead; no cached table materialization. |
| UTF8 | owned lossy String (up to 3B UTF8 bytes), then Text's collected u32 Vec while String remains live | Remove intermediate String or charge both capacities. from_utf8_lossy may borrow before into_owned copies. |
| UTF16 | Vec<u16> units, lossy UTF8 String, then Text Vec; units ends at match-arm return, String overlaps Text conversion | Derive all growth capacities and phase overlap; output proof alone is insufficient. |
| UTF32 | collected UTF8 String, then Text Vec | Charge both allocations during conversion and trailing-replacement growth. |
| UTF7/raw/unicode escape | output Vec; named escape uppercases name into a temporary byte Vec | Charge growth capacity and name temporary; static name lookup does not allocate a table. |
| Punycode | output Vec plus arbitrary-integer limb Vecs, clones for term/next/adapt | Derive digit-count-to-limb bound and simultaneous clone capacities; cancellation alone is not a heap quota. |
| Strict parameter decoding | corresponding codec allocations, then optional ASCII String while Text remains live | Include selected raw parameter bytes, all-label validation and final ASCII projection; no replacement-output inference. |
| Private IDNA2003 | per-label decoded points, B2 mapped Vec, NFKC decomposition/composition Vecs, prepared/lowercase Vec, encoded roundtrip bytes | Source label bound 1024 does not bound an entire header/sequence of labels. Derive expansion and phase overlap from actual implementation and generated-table maxima. |

`Text::into_codepoints` transfers the Vec without copying. `Text::from(String)` currently collects a separate Vec; dropping the original raw input before later UTF8 conversion removes only that specific overlap. It does not erase intermediate allocations above.

The common ProcessPool lease retains caller accounting until verified child cleanup and owns cancellation/deadline kill+reap. A lease is an ownership mechanism, not automatic allocation instrumentation. A06 must not assume a complete heap bound from the donor facade. A future budget hook must charge checked capacities before allocation/reallocation in these paths, or the backend must establish a conservative proven phase envelope plus process memory enforcement. No public allocator/budget API is adopted in this packet.

## Post-v3 direct-output work

The next working version removes body UTF8/16/32 intermediate String and units
Vec allocations, emits singlebyte/ASCII/Latin1 directly into points, removes
strict singlebyte/UTF16 String intermediates, replaces ISO action Vecs with stack
records and compares Unicode names without an uppercase name Vec. UTF7 reserves
its point capacity once. The original inventory above describes immutable v3;
these family changes passed the full30 native codec tests. They do not yet make
output buffers charge-owning or establish the remaining aggregate bound.
