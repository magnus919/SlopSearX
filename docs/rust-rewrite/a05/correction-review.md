# A05 independent-review correction handoff

The independent full-source report at
`/private/tmp/a05-independent-full-review.md` demonstrated four source/native
mismatches. Every finding now has complete actual-source whole-search captures
and native full-response/result/request assertions in `corpus-review.json`.
The original `corpus.json` remains byte-for-byte unchanged at SHA256
`4a83fb3db44894679213ba96941915dc12ad0bf00ad5cbfc691fb0410a86fc2e`.

| Finding | Correction | Appended cases |
| --- | --- | --- |
| Context-scoped CSS excluded the context node | One helper includes a matching context once, then descendants in document order; every Element.cssselect translation uses it | `ddg_context_*`, `google_context_snippet` |
| Declaration/PI/open-comment bodies became false OK/empty | Pinned native HTML tokenizer detects actual start tags/text; comments, doctypes, declarations and PI are not content | `*_review_declaration*`, `*_review_processing_instruction*`, `*_review_unclosed_comment`, nearby valid entity/text/element cases |
| `*`/`~` URL/form bytes differed | Native Python quote_plus encoder preserves exact UTF-8 bytes and safe characters | `*_query_review_*`, `ddg_image_query_encoding` |
| UTF-32 bodies silently lost results | Native BOM/endian-aware UTF-32 decoder preserves source text and replacement behavior | `*_review_utf-32`, `*_review_utf-32-le`, `*_review_utf-32-be` |

The native gate pins both corpora and requires every one of 158 completed case
IDs. Seven native test functions remain mandatory, with no ignored/filtered
cases. Full normalized AdapterResponse/SearchResult assertions and exact
adapter-owned outbound method/URL/header/body/order/count assertions remain.
Only the previously approved typed network diagnostic and host-only cookie
exceptions remain; no result/status/error normalization was added.

Cold bootstrap timing now uses a controlled Tokio clock: no dispatch at 4 ms,
dispatch at the 6 ms timer-wheel tick for its 5 ms deadline. The same test proves
first-search success, second-search lite fallback, fresh cookie jars, and exact
`GET /home`, `POST /html`, `GET /home`, `POST /html`, `POST /lite` order.

Dependency graph changes are narrow: workspace declares the already-pinned
html5ever `=0.39.0`, ssx-infra adds its direct edge, and the existing Tokio dev
dependency enables `test-util`. Cargo.lock adds only the ssx-infra→html5ever
edge. Existing package versions, checksums and transitive edges remain intact.
Cargo deny and the complete parser/request corpus must pass on this graph.

Parent owns source-pin approval, independent correction recheck, integration,
commits, PR and exact-head CI. This handoff is not acceptance or a live-provider,
portal-composition, production/performance or deployment claim.
