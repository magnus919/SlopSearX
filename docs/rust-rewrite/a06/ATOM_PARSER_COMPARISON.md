# arXiv Atom parser implementation slice

Status: implemented first slice, **not A06 acceptance**. The public
`ssx_infra::adapters::arxiv::parse_feed(&str, Option<std::time::Instant>)`
accepts already HTTPX-compatible decoded Unicode scalar text and returns a
classified `AdapterResponse`. The original `_parse_feed` query argument is
unused, so this API does not introduce query-dependent extraction. It does not
perform HTTP, register a factory, report observed latency, or implement query or
redirect policy. Existing source records are unchanged.

## Implemented semantics

The already locked `quick-xml` 0.42.0 reader tokenizes the expanded XML. A bounded
iterative tree preserves namespace-qualified direct children, first matching
field, `.text` before the first element child, comment/PI skipping with adjacent
text joining, CDATA/references, XML line endings, and complete-document failure
before exposing any entry. Namespace declarations and attribute values resolve
references, namespace shadowing and default-namespace undeclaration; undefined
prefixes and duplicate expanded attributes are invalid. XML names use the XML1.0
NameStartChar/NameChar ranges rather than Rust identifier classes.

Extraction preserves source authors/order, presence-sensitive empty DOI/journal/
comments/revision fields, ordered payload data and provenance, published-date
trimming, Unicode whitespace collapse and the 300-code-point snippet. It retains
the known source legacy-ID `rsplit('/')` behavior, including lost category prefix.
Payload construction is engine-local and never applies compact/persistence
truncation. Source syntax failures return OK with zero results, including invalid
XML after an otherwise valid entry. The caller remains responsible for HTTP
status and elapsed latency.

Internal general entities use an iterative expansion stack with cycle detection;
external entities are never loaded. Numeric character references in entity values
are resolved at declaration time, preserving their difference from predefined
entity references. General declarations retain first binding. Attribute defaults
(including namespace defaults), explicit-value override, and declared non-CDATA
space normalization are implemented. In the observed default ElementTree parser,
parameter entity definitions are not expanded; encountering a parameter reference
stops processing subsequent declarations. This behavior is implemented and tested,
including declarations that would otherwise introduce a namespace or general
entity. It is not replaced by a general DTD refusal.

## Resources and cancellation

There is no arbitrary depth100, node100000 or entity-depth16 rejection. Native
coverage accepts source-valid depth1000, 100001 child nodes and a64-level single-
reference entity chain. Iterative tree/namespace/entity stacks avoid native
recursive parsing of those inputs.

The explicit provisional allocation ceilings are review obligations, not approved
source-parity claims. F08 permits32MiB decoded response bytes. HTTPX can replace
one invalid byte with a three-byte Unicode replacement scalar; this parser allows
32Mi code points and128MiB UTF8 input, covering that expansion without silently
lowering F08 to4MiB. Tree structural/name accounting and live namespace bindings
have separate128MiB ceilings. Expanded entity text/live buffers and output data
also have128MiB ceilings. These keep duplicate source, prepared text, tree,
namespace and output allocations finite; they are distinct budgets, not a claim
that total process memory is128MiB. Output data accounting includes retained
payload JSON and result text before returning the response. Genuine native
resource excess returns whole ERROR with `arXiv XML exceeds native <kind> limit`;
this is an intentional operational difference still requiring parent review.

The entity implementation uses Expat's default8MiB activation threshold and100
maximum amplification factor, counting indirect entity replacement-source bytes.
Crossing the amplification policy produces a source-style syntax failure and
therefore OK-empty, not a native resource ERROR. Its exact accounting equivalence
for all declaration/reference arrangements remains to be audited before full
acceptance; matching two amplification examples does not prove that equivalence.

Input character scanning, entity expansion, DTD declaration iteration, attribute
processing, element text construction, summary construction, author traversal and
result traversal yield cooperatively. They check the caller's inherited absolute
deadline before/after yields and before success. Expiry returns Timeout. There is
no spawned task, blocking worker or retained background state; dropping the future
stops it. Quick-xml token reads and individual bounded string/map/serialization
operations remain synchronous; no strict per-byte scheduler-latency claim is made.

## Executable evidence for this slice

Run without Python or network:

```sh
cargo test --locked -p ssx-infra --test a06_arxiv_parser
```

Five nonignored tests pass. One replays complete results/payloads/status against
all24 immutable XML observations and27 applicable arXiv parse paths from the
other corpora:16 initial,10 direct-regression and1 shared successful parse.
Transport failures/redirect errors, query construction, OpenAlex and Semantic
Scholar are deliberately not counted as parser coverage. The51 cases are a fixed
inventory for this slice, not completeness evidence for the139 recorded cases or
issue918. Additional native tests cover namespace character references, nested
entities and markup, numeric-reference declaration timing, default namespaces,
parameter entities, whole-document failure, invalid XML names/attributes,
source-valid deep/large structures, amplification, inherited timeout and future
drop during a large input.

Offline pinned Python3.14.7/ElementTree probes invoked the actual
`ArxivAdapter._parse_feed` and recorded the following resource controls (not HTTP
adapter or provider calls):

| Exact recipe | Actual source observation |
| --- | --- |
| `e0='x'; ei='&e(i-1);'`, reference e16/e17/e32/e64 in title | One result, title length1 for all four |
| `e0='x'; ei='&e(i-1);&e(i-1);'`, reference e18 | One result, title length262144 |
| Same doubling, reference e20/e23 | Expat amplification ParseError; source OK-empty |
| Nested x elements at depth100/101/1000 | Accepted XML, source OK-empty |
| Root x with100000/100001 empty y children | Accepted XML, source OK-empty |
| DTD default `feed xmlns CDATA 'http://www.w3.org/2005/Atom'`, ordinary entry/title T | One result with title T |

The source probe script and exact hashes/outcomes are retained for parent review
at `/private/tmp/a06-expat-resource-probes.py` and
`/private/tmp/a06-expat-resource-observations.json`. They are not a newly accepted
mandatory oracle invocation. Durable source-evidence inventory registration and
any additional observations remain A06/F01 work.

## Concrete remaining obligations

- Full XML/DTD well-formedness equivalence: declaration ordering/version/encoding
  grammar, ELEMENT content-model and NOTATION declaration grammar, parameter-
  reference contexts and standalone interactions, unparsed entities and default
  attributes across every supported declaration form. Current ELEMENT/NOTATION
  handling skips content-model validation; this must not be promoted as complete
  Expat parity. Unsupported other DTD constructs return an explicit unfinished-
  compatibility ERROR while no adapter factory is activated.
- Exact Expat amplification accounting and allocation/overflow stress review,
  including attribute/entity combinations, repeated references, direct-input
  padding, large declarations and output amplification. Final operational budgets
  require parent review and source/delta observations, not an arbitrary count cap.
- End-to-end HTTPX-compatible response.text decoding (108 compatibility cases from
  A05), compression/status behavior, query encoding/plain-versus-explicit syntax,
  one-hop redirect host/port policy and physical request/deadline accounting.
- Actual arXiv factory/config/admission/session integration and real-loopback
  cancellation, OpenAlex/Semantic Scholar implementations, complete shared-test
  ownership/successors, portal review, G01 composition and exact-head CI.
- Register the existing three temporary exporter files/four invocations with F01,
  preserve immutable evidence and retire temporary Python under G08. This slice
  adds no mandatory Python execution or runtime Python dependency.
