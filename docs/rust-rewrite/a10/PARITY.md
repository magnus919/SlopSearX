# A10 source comparison and explicit differences

The historical reference captures are `source-regressions-v1.json` (133 observations from 74 passing original test calls), `source-boundaries-v2.json` (308 observations), and `source-supplement-v1.json` (30 observations). Their bytes are immutable. The historical failed boundary capture is retained separately and is not counted as completed evidence. `oracle-inventory.json` and `evidence/original-run-archive-v1.json` distinguish the actual executed producer bytes from the current formatted producer. Neither producer nor historical text is invoked by the native gate.

The comparison retains full source output, method, full URL, body, custom headers, dispatch count and unused-exchange count. It does not normalize arbitrary fields or accept status-only equality. The exact 452 difference records bind complete source/native values to case identities. The original reference used HTTPX MockTransport; the native replay uses injected exchange readers. Actual private HTTP fixtures separately prove the native transport bytes.

## HTTP identity and diagnostics

The native JSON API User-Agent identifies the native SlopSearX version. ExploitDB and MITRE retain their explicit source browser User-Agent. MockTransport-added Host, Accept-Encoding and Connection headers are recorded separately from adapter-authored headers. Actual native loopback requests establish the native transport-added headers; no claim is made that these two transport implementations add identical defaults.

Forty-two recorded diagnostic-only differences bind the exact source and native messages, with every other output field unchanged. These are finite entries, not permission to ignore future error text. The source request comparisons likewise require exact full URLs, bodies and custom headers.

## Typed public representation refusals

Ten exact source observations cannot be represented by the accepted F05 public result types. They retain the full raw source expectation in the immutable capture and the full classified native refusal in the ledger:

- One CVE description contains a lone surrogate that cannot inhabit a Rust string.
- Five CVE publication-date observations use booleans, numbers, lists or objects where the native public field is an optional string.
- One NVD publication date is a list. The native adapter classifies the entire response as Error, including any otherwise valid sibling rows; it does not stringify the date or return a partial success.
- Three EPSS scores are nonfinite. The native public score requires a finite number.

The exact refusal identities are:

| Source identity | Native boundary |
| --- | --- |
| `a10-boundary/cve/description-132` | Lone surrogate cannot inhabit the public string. |
| `a10-boundary/cve/date-135` | Non-string optional publication date. |
| `a10-boundary/cve/date-136` | Non-string optional publication date. |
| `a10-boundary/cve/date-137` | Non-string optional publication date. |
| `a10-boundary/cve/date-138` | Non-string optional publication date. |
| `a10-boundary/cve/date-142` | Non-string optional publication date. |
| `a10-boundary/nvd/date-185` | List publication date; whole response Error. |
| `a10-boundary/epss/score-194` | Nonfinite public score. |
| `a10-boundary/epss/score-195` | Nonfinite public score. |
| `a10-boundary/epss/score-196` | Nonfinite public score. |

These refusals are deliberate boundary differences, not claims of literal output parity for those ten cases.

## Settings and deadlines

Two raw original NVD settings throw TypeError before the original try block and before HTTP. The native adapter returns a classified Error, preserving the adapter-never-raises invariant. The text case is exercised through real F06 YAML and text-seeded environment settings with zero HTTP; integer-seeded invalid text is rejected by F06. Raw None and unbounded integer settings are explicitly separate from public typed configuration reachability. Raw max values remain exact on outbound requests; only source slicing uses safe signed/clamped bounds.

Fifteen zero or negative timeout controls require exact native Timeout with zero dispatch. The original immediate MockTransport outcomes do not prove enforcement by a real Python network transport.

Only `exploitdb/timeout-0.25`, `exploitdb/timeout-True`, `mitreattack/timeout-True` and `mitreattack/timeout-0.25` allow scheduling alternatives. Each must produce one complete correlated tuple: exact Timeout with zero dispatch and one unused source exchange; exact Timeout with one exact full request and no unused exchange; or exact full source Success with one exact request and no unused exchange plus exactly one actual finite operation deadline and one terminal projection handoff stamp, with identical deadlines and the timestamp used by the real terminal check strictly before that deadline. The observer is scoped to the individual future; it does not create another clock sample or change the real checks. No arbitrary errors, partial results, extra requests or artificial delays in corpus replay are allowed. This witnesses cooperative handoff acceptance; scheduler delay after that check is not evidence of a hard wall-clock return deadline.

Private pending-socket tests separately observe an actual request before cancellation, timeout or shutdown, then verify socket closure and joined ownership. Native helper tests observe extraction entry and actual work before cancellation/deadline/shutdown and verify reap and released storage. JSON API calls without a SearchContext retain their source network-timeout scope; this lane does not broaden their timeout semantics.

## Bounded ownership

HTML uses the existing A05 input/tree/descriptor/row-buffer charging and retained worker lease. The old A05 layout and reply bytes remain covered. A10 validates its separate row schema, rejects malformed/cross-layout frames, yields during empty-field row loops and long UTF-8 fields, and retains owned storage through public handoff. NVD structural CWE membership uses charged cooperative comparison rather than stringification. These are scoped ownership proofs; G03 still owns a complete process RSS claim.
