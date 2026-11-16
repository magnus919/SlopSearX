# Bounded native codec allocation plan

Status: concrete implementation proposal following the v3 development freeze.
No budget API or aggregate bound is accepted by this document. V3 is immutable;
A06 uses its reviewed bytes while a later coordinated donor implements this plan.
No global allocator, unsafe allocator hook, Python backend or process-only memory
claim is proposed.

## Owned allocation policy

A search retains one admitted operation lease through raw response, helper IPC,
decoding, backend parsing and final projection. Its finite quota is at most
256MiB under the approved A05 policy. Admission is not acquired separately for
individual child calls. Each live allocation has a charge owned with that buffer;
freeing its charge requires dropping the buffer. The parent lease clone remains
in the common supervisor until verified cleanup. Unknown cleanup propagates the
common failure and retains accounting, rather than pretending memory drained.

Introduce a leaf-private fallible `DecodeBudget` and charged buffer wrappers.
The decoder returns a distinct resource error; callers map it to the approved
sanitized whole-adapter resource error at the Decode stage. It must never become
UnicodeFailure, unknown charset fallback, truncated output or a partial result.
Existing unbudgeted entry points remain only while callers migrate; the mandatory
helper path must use the bounded entry point before acceptance.

Checked preflight computes every requested allocation layout before allocating.
A charge covers its capacity, not its initialized length. The fixed output
buffer has B u32 slots for B body bytes (the existing family proof establishes
that output count never exceeds B). Initialize/allocate once, maintain a checked
write cursor, and truncate length at return without shrinking/reallocating.
No family may push beyond this fixed capacity or implicitly grow it. This uses
safe standard buffers; supported Rust1.99/default-global-allocator allocation
layout and actual capacities must be verified on all supported targets. Malloc
metadata/alignment overhead and mapped tables remain separate process RSS policy,
not hidden inside a logical returned-codepoint claim.

Scratch buffers reserve a checked layout under the remaining quota before every
allocation or growth. Growth must charge the new allocation while retaining the
old charge until the allocator operation/old buffer lifetime ends. Prefer new
charged buffer plus move/copy plus explicit drop over opaque Vec growth where
old/new overlap cannot be accounted. Buffer wrappers expose no uncharged clone,
collect, to_owned or into-String operation. Allocation failure returns resource
failure. Cancellation kills the process while all these charges remain held by
the enclosing operation lease.

## Family changes

| Family | Concrete implementation change | Bound/proof needed |
| --- | --- | --- |
| Singlebyte, ASCII, Latin1 | Write mapping u32 directly into fixed output. Remove UTF8 String intermediate. | Exactly one slot per input byte; strict validity checked before/while writing. |
| UTF8/SIG | Iterate source-compatible replacement actions directly into u32 output; strict mode validates without allocating an intermediate String. | Preserve malformed subsequence consumption and partial SIG body/strict distinction with existing complete vectors. |
| UTF16 | Read u16 words from borrowed bytes and emit decoded scalars/replacements directly; no units Vec or UTF8 String. | Preserve BOM/error/trailing high-surrogate plus odd-byte behavior exactly; strict native-endian mode remains separate. |
| UTF32 | Read borrowed four-byte chunks and emit directly. | Preserve partial BOM/trailing bytes/surrogate validity and exact diagnostics. |
| CJK/HZ/ISO2022 | Pass a fixed output writer through action routines; ISO map lookup returns borrowed record/at most two stack codepoints instead of tiny Vec. | Existing consumption partition and static record maximum; no heap mapping cache. |
| UTF7/raw/unicode escape | Fixed output writer; named lookup uses borrowed bytes with ASCII-case-insensitive comparison rather than uppercase name Vec where possible. | Preserve full source name/alias/sequence/NUL/case controls; any retained temporary gets its own charge. |
| Punycode | Charge every arbitrary-integer limb buffer and clone, plus fixed output. Replace implicit resize/growth with checked reserve/copy. | Each input digit multiplies weight by at most35; use integer-safe six-bits-per-digit upper bound for limb layout, then inspect simultaneous delta/weight/term/next/adapt clones. Overflow diagnostic uses source integer semantics, not a smaller integer substitute. |
| Private IDNA | Process one source-bounded label at a time. Charge decoded label, B2 mapping, recursive NFKC decomposition/composition, prepared/lowercase and roundtrip bytes separately. Final ASCII output has a charged buffer across labels. | Source1024-byte label bound applies before decode; derive recursive decomposition maxima from pinned generated data, B2 max4, and roundtrip output checks. An arbitrarily long multi-label header is bounded by its actual admitted raw bytes and aggregate quota, not by claiming one label bounds the header. |
| Codec labels/selector | Charge normalized label and dotted-alias fallback while both coexist; avoid unconditional duplicate normalization. | Ordered raw header storage, RFC2231 fragments/concatenation/percent decoding, strict output and fallback copies belong to A06 shared selector's own ledger. Whole-body empty short circuit remains before selector. |

## Parent/helper boundary

The request frame declares validated raw body/header lengths and a finite child
allocation allowance derived from the admitted remaining operation budget. The
parent charges input/frame buffers before copying and passes the original lease
to exchange_with_lease. The child checks each length before preallocation and
rejects trailing bytes. It does not trust an arbitrary unbounded allowance from
config. Response codepoints/rows are validated and charged before allocation in
the receiver; the sender owns its output charge until write completion/exit.

For A05, subsequent CSS/row extraction must run in the same supervised helper.
No query bytes are needed in the child: return a fallback-title flag and apply
query fallback in the charged parent projection. Binary rows must use checked
length-prefixed fields and explicit tags, avoiding the current provisional JSON
response envelope and storage serializer cap. Google success feedback remains
between decoded challenge classification and parse/extraction; its original
search lease spans both child exchanges and parent feedback.

## Acceptance evidence

First qualify charged-buffer unit tests: exact-limit success, one-byte shortage,
checked arithmetic overflow, growth overlap, early failure, drop and returned
ownership transfer. Exercise each scratch-heavy family with real helper
cancellation/deadline and prove PID reaping plus admission release; a returned
resource error must preserve source feedback/request order. Run the existing full
body and strict source suites after changed family dependencies, then whole
adapter outcomes and shared selector controls. Add process RSS/FD tests and GNU
amd64/arm64/macOS native-link closure evidence separately. None of these is
satisfied merely by the inventory, a returned Vec length, or a passing normal
input test.
