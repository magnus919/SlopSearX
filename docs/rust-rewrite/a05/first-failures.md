# A05 retained failures

1. Initial source exporter omitted lite responses for `max_results <= 0`.
   Original `corpus-preflight.json` retains source's third request and swallowed
   `unexpected request` diagnostic. Exporter now supplies a real lite exchange
   and fails if any unplanned request occurs. Acceptance uses fresh source
   capture, never manually rewritten expected outcomes.
2. Whole-output replay found comment-only and doctype-only Google documents
   became OK/empty with HTML5 parsing. Source raises `Document is empty`; native
   now preserves that classification and message. Valid empty actual HTML,
   whitespace, doctype/comments and comment-prefixed HTML remain distinct cases.
3. Cookie request comparison found HTTPX host-only parent-to-subdomain leakage.
   Parent reviewed and approved retaining accepted F08 RFC host-only behavior.
   Source bytes/requests remain unchanged, with one named native expectation.
4. Raw Latin-1/ASCII byte probes found encoding_rs's WHATWG alias mapping changed
   Python-decoded Latin-1 control U+0080 to EURO and accepted non-ASCII bytes.
   Native now explicitly preserves the original codec semantics. No corpus
   normalization was used.
5. Cold native client construction could outlast the source's 5 ms bootstrap.
   Success-path real-loopback test now explicitly accounts for a socket warmup;
   a separate cold-bootstrap expiry test preserves the timeout/swallow/search
   sequence. This is test determinism, not a changed engine timeout.
6. Initial Clippy run rejected nested conditionals, a redundant conversion,
   chunk parsing style and test lock scopes. These were corrected in source.

Later independent review and exact-head CI may expose additional differences;
the delivery task must append their outcomes before acceptance.

Gate mutation checks retained at `/private/tmp/a05-ignored-red.json` and
`/private/tmp/a05-empty-red.json` both failed with
`A05 required native cases did not execute successfully`: one ignored the
corpus test, the other executed an empty loop while all seven test functions
still passed. Exact original test bytes were restored after both probes.
An O06 check was also rejected by the source-stability guard while final
source-mapping metadata was being completed; its semantic checks passed.
A stable-tree rerun is required evidence, not that rejected report.

## Independent full-review corrections

The independent full-source review found four uncaptured incompatibilities:
context-inclusive lxml CSS selection, declaration/processing-instruction/open-
comment empty-document classification, exact Python query/form encoding for
`*`/`~`, and UTF-32 decoding. The original 115-case corpus is retained byte-for-
byte; 43 appended actual-source observations live in `corpus-review.json`.
Corrections compare full outcomes and outbound requests without new exceptions.
The expanded replay also caught a nearby valid `&nbsp;` page being classified
empty; content classification now uses HTML ASCII whitespace, preserving the
source distinction from decoded NBSP.

The first controlled-clock bootstrap assertion expected dispatch at exactly
5 ms; Tokio's timer wheel observed that deadline on the next 6 ms tick. The
final test explicitly drives 4 ms (no dispatch) then 6 ms (dispatch), and also
asserts cold-expiry fallback and exact five-request order across two searches.

One correction build failed writing its disposable compiler query cache with
ENOSPC. Only this task's completed incremental build cache was removed after
parent approval; source/evidence remained intact. Validation resumes with
`CARGO_INCREMENTAL=0`.
