# A12 bound and type application review

This is an explicit review list. Existing native primitives are accepted source;
applying their refusal to a particular A12 source case still needs parent review.
It is not permission to declare another subset of Python inputs legal.

| Boundary | Current bytes/values | Existing source reference | A12 status |
| --- | --- | --- | --- |
| F08 wire, decode, request-body and session exchanges | 8MiB wire,32MiB decoded,2MiB request body,16 exchanges;10 redirects if enabled | `crates/ssx-infra/src/http.rs::Bounds::default` | Reuses unchanged Options defaults; no A12 override. Redirects remain disabled, matching original client defaults. |
| Source JSON bytes/depth/text/integer syntax | 4MiB bytes,100 depth,4Mi code points,4300 integer digits | `adapters/source_json.rs::MAX_*`, accepted A01/A07/A11 consumers | Calls the existing parser; no codec change or new JSON-shape cap. Integer4300 behavior is the existing source parser contract, not a64-bit A12 integer cap. |
| Public score and typed text | finite f64 score, String title/URL/content, Option<String> date | F05 `ssx-domain/src/records.rs`, `source_payload::ProducedBatch` and A11 PARITY.md typed-refusal cases | Reuses the same typed API. A12 tests explicitly exercise six nonfinite score inputs, numeric titles and a lone surrogate; unused malformed values are not rejected. Exact source-operation/refusal precedence cross-products remain review work. |
| Source representation | shared Python repr, Unicode16 decimal tables, source float spelling | `source_repr.rs`, `ssx-json`, `ssx-domain::storage` | Uses accepted implementations; no ASCII-only result alphabet, numeric magnitude filter or string truncation added. |
| Owned projection scratch/output |32MiB per operation | `clinicaltrials.rs::ROW_BYTES` and A07/A08/A09/A11 operation quotas | Same numerical quota and owned allocator, but its A12 application is explicitly **proposed for review**, not automatically accepted by another batch. No global RSS or post-handoff response lifetime claim. Source DOM remains separately bounded/owned by the existing SourceJson parser. |
| Finite absolute lifecycle | min(configured deadline, caller SearchContext); context also bounds admission | application SearchContext and `clinicaltrials.rs::network_deadline`, A11 request owner | Uses the same native rule; actual in-flight tests exercise each finite bound/drop/shutdown. Direct nonfinite config without finite caller context refuses, as existing native lifecycle code does. No timer renewal or source tiny-deadline success exemption. |
| CRT.sh HTML sniff/scan |first256 raw bytes,first65536 decoded code points;9 ordered markers | `engines/crtsh.py::_looks_like_html`, `.search`, `_CHALLENGE_MARKERS` | Original behavior, not a new native safety limit. Tests distinguish scan cutoff in code points from bytes and malformed rows beyond local slice. |
| Provider result slices | Shodan/CRT.sh configured slice; other three retain source behavior | five original engine files | Boolean/negative limits preserve Python slicing. Censys/VT do not acquire a local limit; VulnCheck ignores max_results. No hardcoded A12 row cap. |
| F06 configuration | existing ConfigScalar integer/f64/bool/text and Secret | `ssx-config/src/lib.rs`, A11 PARITY.md configuration entry contract | New A12 settings accept every existing scalar variant. Resolved blank base defaults match service normalization; direct empty base remains explicit/invalid. No new raw settings API. |
| Header/URL construction | existing F08 Request/reqwest URL and HeaderMap | F08 plus A11 Unicode credential controls | No custom A12 host/query/body alphabet. Unicode Basic auth encodes UTF-8. Non-ASCII direct string headers retain original pre-dispatch rejection; exact diagnostic spelling is a reviewable F08 abstraction difference. |
| Test-only child watchdog/log bound |20s monotonic,1MiB combined logs,64KiB private test request headers | private A12 harness only | Owns/reaps its child, removes private directory and fail-closes without truncation. These constants do not restrict adapter/provider data or inflate adapter timers. |

Generic F08 HTTP/network failure messages deliberately expose no request URL,
credential or response body. The current native constants and original assertion
scope are recorded in the source map; no absent original output has been invented.
Any new source refusal beyond these existing boundaries must be proposed rather
than normalized away. No shared runtime or SourceJSON edit is part of A12.
