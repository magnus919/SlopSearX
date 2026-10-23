# Closed scholarly JSON helper candidate

This is a design/prototype work item, not resource acceptance or factory wiring.
It extends the existing ProcessPool; it does not add another process manager.
A source-valid800,030byte response with400,000citation-count list elements takes
116,393microseconds in the current parent-side consumer despite a1ms deadline.
The formatting happens before the source float(list) error. Paired probe source
and log are `/private/tmp/a06-semantic-sync-cost-probe.rs` and `.log`.

## Alternatives and proposed ownership

A parent-side implementation needs iterative cooperative representation,
projection, field-copy and source-sort operations. The numeric lexical frontend
already yields, but that does not cover representation or opaque payloads.
Reimplementing Unicode representation is unnecessary and risks divergence.

The candidate instead uses closed, versioned SemanticScholar/OpenAlex operations
inside the existing supervised scholarly child. The original operation Arc stays
with the caller through checked reply decode/projection; its clone stays with the
supervisor until payload/reply disposal and verified reap. Existing cancellation,
unknown-cleanup poisoning and concurrent-shutdown contracts remain unchanged.
The same pure consumers are reused. Deadline cancellation terminates and reaps
CPU-bound formatting/sorting, rather than merely selecting around synchronous
work on a Tokio thread. No runtime registry activation is proposed yet.

Owned candidate paths are the existing arxiv_expat helper entry/frame dispatch,
its binary reply module, SemanticScholar consumer, new OpenAlex consumer and
source-order module. Accepted SourceJSON remains single-owned shared code; an
additive budget-aware entry point requires separate parent review. Its historical
parser and corpus must not be copied into a fork or silently changed.

## Allocation obligations before a prototype can qualify

The existing64MiB Expat/retained ledger does **not** account SourceJSON nodes,
recursive repr temporaries or arbitrary typed payload maps. GNU1GiB address-space
containment is not that proof. A complete envelope must include:

* parent transport body/header ownership, checked frame allocation and response;
* child raw input, decoded u32 input, parser containers/text and key-index scratch;
* consumer author/pair vectors, repr/concat/trim intermediates and retained fields;
* closed reply serialization, parent bounded decode and final typed output;
* native/Rust allocation metadata, overlapping reallocations and configured N.

Actual64-bit candidate sizes are Value48, Text48, Number24, Map24,
(Text,Value)96, String24and usize8bytes, measured by
`/private/tmp/a06-source-json-size-probe.rs`/`.log`. Cross-target CI must establish
its own layouts rather than assuming Darwin measurements qualify GNU.

SourceJSON's current object parser keeps a BTreeMap<Text,usize> scratch index,
clones each distinct key into it and uses infallible allocation APIs. Charging
only final Value capacity misses that entire peak. Text retains both Vec<u32>
and an optional scalar UTF8 String. Raw decoding retains its own u32 vector
until parsing completes. Array/map Vec growth, finite-float normalization scratch,
recursive boxed parser futures and key replacement all need explicit coverage.
A blanket input-times-factor estimate is not a completed proof.

The preferred prototype is an additive budgeted parser entry: retain the existing
decoder/grammar and preflight or charge each concrete allocation before reserve.
Where a standard container has no fallible reserve, either prove/precharge its
complete allocation envelope from pinned implementation/layout or use a reviewed
fallible index with identical key/duplicate semantics. Do not introduce a smaller
body limit to conceal an uncharged node allocator. Any explicit operation quota
refusal is operational hardening, distinct from source syntax errors and pending
output representability refusals; original captures remain unchanged.

The output protocol must remain a closed typed binary schema with tags/counts/
lengths checked before reserve/copy, no whole-result JSON serialization/reparse.
Opaque science payload values require a recursively budgeted tagged value schema;
calling unrestricted serde before validation is not an allocation bound.
Allocation failure/child abort must be classified operationally and leave no
worker, permit or accounting ownership unverified. Full source operations and
source-error precedence remain required before representability refusal wins.
