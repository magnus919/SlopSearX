# Concrete remaining parser allocation closure

Status: design for review, not permission to add unsafe code or a resource
qualification claim. Actual rows IPC now preserves all587 original complete
outcomes, but the following allocations still escape the child root ledger.

## Inspected source and exact gaps

The pinned libxml2 2.15.4 archive remains SHA256
98087fd181d9070724f3fbc65c7377db03038eb92bd882374daff44940138821.
Its xmlmemory.c is SHA256
c1a46db028e2a12e70b083e1a9b19bb5ca0fa0101dc3f1494e2547cd1a6db4d9.
`xmlMemSetup` assigns xmlFree/xmlMalloc/xmlMallocAtomic/xmlRealloc/xmlMemStrdup;
its source explicitly requires installation before ANY other libxml routine.
There is no per-context tree-allocation quota in the currently used parser API.
NONET and parser depth/text limits are not allocation accounting.

Current patched Rust source identities:

- parser.rs4365c1b0714c33d25f08e45df017778ab87286da5fa0950d054850093a9dec5d:
  parse_untrusted_html_codepoints creates an uncharged PEP393-width byteVec;
  parse_with_resource_policy creates parser/context/encoding CString and collects
  all structured errors into an uncharged Vec before wrapping a Document.
- tree/node.rs2df990816eb98050cfb28b73250ff574747acd4ee4f3dac068f52a8f2887d9e6:
  get_name copies a C string; get_content and attr_node_value call allocating
  xmlNodeGetContent then copy to Rust; get_properties builds HashMap<String,
  String>; get_child_nodes builds a Vec of reference-counted Node wrappers.
- tree/document.rsaff7974532914f1f692d1cc93b1b54da0e44f787ab4f294ec3315fad5f56c970:
  Document maintains a pointer-keyed NodeMap and Rc/RefCell wrapper graph.

The current Html flatten pass then creates another Vec<Node>, BTreeMap attrs,
String fields, traversal stack and per-node childrenVec. CSS/select/text extraction
and image sanitization create additional uncharged Rust collections. Removing
JSON tree IPC removes its clone/frame buffers, not these allocations.

## Smallest safe dependency direction

Add a private immutable HTML parse facade in the already-patched dependency,
separate from general Document/Node. Proposed types:

- `BudgetedHtmlDocument`: owns the C document and root-ledger association;
  nonClone, !Send/!Sync, no conversion to mutable Document or raw pointer.
- `HtmlNode<'doc>`: copyable immutable view whose lifetime borrows that document;
  exposes parent/first_child/next_sibling, node kind, name bytes and text bytes.
- `HtmlAttribute<'doc>`: linked immutable attribute view exposing name bytes and
  an iterator of its text/entity content fragments.

Existing Node cannot safely return borrowed strings: it is Clone and aliases can
mutate the same C tree through another wrapper. The new opaque immutable owner
must therefore never expose those existing wrappers or mutation APIs. Borrowed
views eliminate NodeMap, Rc wrappers, get_child_nodes Vec and HashMap getters.
C strings remain byte views; bounded caller sinks reproduce existing lossyUTF8
conversion exactly instead of assuming every internal string is validUTF8.

For attribute content, first prove the fragment traversal equals the current
xmlNodeGetContent result over entities/CDATA/malformed attributes and all587
cases. If direct fragments cannot preserve a supported node kind, retain the
C getter under the SAME charged C allocation hooks and stream/copy its returned
bytes before freeing it. Do not silently return an empty value on allocation
failure or broaden the parser options.

## C allocation hooks and lifecycle proposal

Install fixed private callbacks through xmlMemSetup at dedicated child startup,
BEFORE init_parser, parse context creation, catalog/table initialization or any
other libxml call. The ordinary application process and legacy parser APIs must
not install these hooks. Add a process-wide one-time initialization guard inside
the dependency; the helper has exactly one root Budget and one parser thread.
This requires a reviewed dependency edge to neutral ssx-owned-budget (no runtime
or new external version) or an equally concrete reviewed reservation interface.
No application-owned unsafe callback is proposed.

Each C allocation uses an aligned private header storing checked requestedsize,
full layout and a nonClone root Charge. Reserve payload+header+alignment BEFORE
calling the underlying allocator. NULL/overflow/budget refusal returns NULL and
sets a sticky resource-refused flag. free releases storage before its Charge.
realloc reserves/allocates the complete new block while the old allocation and
Charge remain live, copies min(old,new), then frees old; failure retains old
pointer/charge exactly. strdup uses checked strlen+1 and the same allocator.
No callback formats diagnostics, allocates a side-map, panics or unwinds through
C. Exact ABI/alignment/zero-size/null/realloc failure behavior requires review.

All C parser globals and outstanding document blocks remain charged until child
exit or verified explicit cleanup. Never restore hooks while any hooked pointer
can be freed. Keep root ownership alive for the entire child, including parser
context/document/global teardown. A sticky refusal makes the WHOLE operation a
Resource outcome even when libxml returns a partial tree. It must not become
Documentempty and trigger another DDG request. Existing loader denial, fixed
encoding, structured-error callback restoration and kill/reap ownership remain.

This hook covers libxml-managed allocations, not arbitrary SDK/glibc iconv,
allocator metadata, Rust/Tokio/stdio or OS RSS. Enumerate direct allocation calls
in the pinned compiled C feature closure and qualify known fixed-encoding iconv
open/close overhead separately. GNU amd64/arm64 and macOS symbol/closure receipts,
process memory limits where enforceable, measured RSS/FD stress and aggregate
container/concurrency budgets remain required. No NONET-only or hook-only RSS
claim is acceptable.

## Rust flatten, CSS and sanitizer closure

Build a charged Html owner directly from immutable views. Precount nodes/attrs
with a stack-free parent/sibling walk, reserve exact Vec capacities before copy,
and retain all string/attribute/children backing charges beside the Html owner.
Replace BTreeMap attrs with a counted ordered attributeVec (parser uniqueness
must be tested) and preserve exact attr lookup semantics. Traversal uses sibling
links or precharged storage; no hidden get_child_nodes/HashMap temporary remains.
Keep codepoint input and PEP393 encoded bytes simultaneously charged while live.

Move extraction to a fallible row sink: precharge row capacity/growth overlap,
text concatenation and redirect-unwrapping buffers. Google dedup keys must be
charged; a borrowed set over immutable URL views avoids copies where exact source
comparison permits it. CSS and text helpers receive checkpoints, but hard child
supervision remains necessary for non-emitting scans.

The child currently calls accepted storage::build_media and sends its sanitized
fields to the parent. To close sanitizer heap, generalize the existing sanitizer
algorithm to a fallible caller-owned sink/collection policy, retaining the legacy
API as a wrapper over the SAME algorithm. Charge cleanedURL, lowercase scheme,
query key/value groups (including Vec growth overlap), serialized query and each
reconstruction allocation before allocation; preserve duplicate grouping,
sensitive-key comparison, invalid-netloc fallback and originalURL distinctions.
Do not fork a second media algorithm. The current lazy python_text_tables load
also needs an explicit charge/lifetime or a reviewed static URL-only table from
its already-pinned Unicode data. This is a narrow shared-domain proposal needing
owner coordination; it is not yet implemented or adopted.

## Required evidence before claiming closure

1. C ABI/header/alignment review plus malloc/zero/null/free/strdup/realloc growth,
   shrink and injected-failure tests; old/new overlap and sticky refusal counts.
2. Prove hook installation precedes initialization; callback counters actually
   fire on parser/document/global paths; every allocation header frees under the
   same hooks, including source parse failure and partial-tree OOM.
3. Negative resource-loader and fixed-encoding controls remain green with hooks;
   all587 complete source outcomes,44surrogate and allNUL controls unchanged.
4. Per-node/attribute/name/text lossy conversion and entity-fragment parity,
   deep/wide/many-attribute CSS/sanitizer stress, exact quota refusal before
   additional HTTP requests, slow/no-output deadline and cancellation/reaping.
5. Parent frame/projection/query fallback/result owner overlap ledger plus
   complete child peak, native allocator/RSS/FD and all-target closure receipts.

No implementation of the new unsafe facade/hooks is authorized by this proposal
alone. The rows/selector work can progress while the exact dependency patch and
shared sanitizer sink are reviewed; these requirements remain A05 scope.
