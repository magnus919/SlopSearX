# A03 parity and intentional differences

Source observations are immutable, including source hazards. These decisions
are separate from observations; none changes a golden expected result.

| Boundary | Source observation | Native decision/evidence |
| --- | --- | --- |
| Stack Exchange parsing outside catch | Wrong collections/rows, invalid score/date/body can raise instead of returning AdapterResponse | Classify the whole parse as error with no partial results. Real source exceptions stay in the corpus; replay proves refusal. This follows the adapter invariant that orchestration never receives parser exceptions. |
| Typed result/finite score | Python dataclasses can carry null/nontext strings or nonfinite float score while status is OK | Refuse the entire response at the native typed/finite boundary. Mixed valid/invalid rows yield no partial response. Original raw values remain captured. |
| Operator numeric settings | Python dynamically forwards negative/noninteger/container max_results; SE may slice or throw. Negative/huge timeouts also vary by transport. | Pure settings resolver rejects noninteger or negative max_results and values above platform usize, preserving zero. Timeout must be finite, nonnegative and Duration/Instant representable; fraction and zero remain valid. This is explicit native config validation, not wire parity for malformed operator scalars. |
| Zero deadline | MockTransport does not enforce real socket timing; source can return a fixture result even at zero timeout | Zero operation budget is exhausted before session construction; GH/HF/HN return timeout, SE error. This removes a race where immediate connection failure could win the elapsed deadline. No provider request is sent. |
| Invalid base URL | HTTPX MockTransport can record relative requests before source produces error | Accepted F08 absolute URL construction refuses empty/relative malformed bases before transport. Native request count is explicitly zero for these cases, while source request records are preserved. Valid absolute custom base paths and query oddities compare exactly. |
| Invalid header bytes | MockTransport accepts a synthetic newline-containing header without real HTTP framing | F08/header construction rejects invalid credential header bytes before transport, generic error and zero requests. This is not claimed as real source-wire acceptance. |
| Provider errors | Python may expose exception text, including URLs and lower-level errors | Fixed provider-safe diagnostic text avoids credential/URL disclosure; known source code-auth, 401, 422 and SE400 messages are retained. Status, zero/partial results and request counts are compared separately. |
| Secret configuration | Python has no adapter key length cap | Reuse the accepted redacted Secret with a 4096-byte native config cap. Optional empty credentials map to None. HF whitespace is preserved, GH/SE Python strip is retained. |
| Health/probe | Source base health can report public credential readiness without HTTP | SearchEngine has no provider health hook. Accepted F10 owns configured and observed health; public search/no-key and code-only refusal are covered without inventing an upstream health request. |

HTTPX's default headers, host and compression advertising are retained in the
source request provenance, while comparisons assert engine-owned method, exact
raw target, request count, UA (GH/HF/HN), Accept (GH) and credential placement.
F08 owns transport framing/defaults. No generic response-shape normalization is
applied: falsey empty collections retain source behavior, and malformed selected
rows fail according to the documented typed/outside-catch boundary.

The timestamp helper exposure is visibility-only: accepted application civil
arithmetic, range/rounding and algorithm are unchanged. Adapters translate its
internal error to a fixed provider diagnostic rather than exposing its legacy
health-specific wording. Numeric timestamps and Python float score conversion
remain distinct: bool is a numeric timestamp, numeric text is not.

These adapters deliberately preserve limitations: GH page1, other paging
ignored, provider-specific category precedence, three engines' lack of local
max-results slice, SE400 classification and header key convention. F09's live
policy/eligibility/enforcement remains authoritative; adapter metadata is not
permission to bypass it. No implicit retries, background health calls, hidden
credential discovery or Python runtime fallback is introduced.
