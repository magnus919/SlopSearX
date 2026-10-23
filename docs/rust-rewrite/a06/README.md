# A06 native scholarly adapters

Current actual factories, F06 wiring and required native gate are documented in
[NATIVE_QUALIFICATION.md](NATIVE_QUALIFICATION.md). See
[PORTAL_IMPACT.md](PORTAL_IMPACT.md) for the separate issue984 integrated browser
and shared-service obligations. Local component qualification does not establish
final merged-source/platform acceptance or issue closure.

## Historical initial source packet

The following notes preserve the original source-packet stage and its then-open
work; their pending-implementation statements are historical.

Issue [#918](https://github.com/magnus919/SlopSearX/issues/918) covers OpenAlex,
Semantic Scholar, and arXiv. This work starts from freshly fetched upstream
`730c37a5f48b9830088594582cefb4c2f10a35e8`; A01's accepted factories and shared
F08 HTTP/F10 admission interfaces are already present. The prior A03 branch is
preserved; this work uses `codex/rust-a06-scholarly-adapters` in its idle worktree.

This packet contains **67 actual whole-adapter source observations**, not native
implementations or acceptance evidence: 21 OpenAlex, 23 Semantic Scholar and
23 arXiv. The exporter uses HTTPX MockTransport, retains ordered requests and
client options, and captures complete adapter outcomes. It removes latency only,
serializes enum values and sorts sets. Source-file SHA256 values accompany the
observations. Only fake fixture credentials are present. No provider was called.

Reproduce from this repository root with the pinned migration oracle:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_source.py > /tmp/a06-source.json
```

The final formatted exporter reproduces `source-initial.json` byte-for-byte.
It remains temporary developer-only Python requiring an F01 disposition and
invocation entry, and retirement under G08. It must never be executed by native
runtime or mandatory native qualification gates. That inventory update is still
pending, as are all native implementations and full regression mappings.

## Contracts to finish before acceptance

- OpenAlex: validate inclusive date bounds before admission/dispatch, preserve
  upstream date parameters and local proof checks, DOI fallback, abstract
  position ordering, upstream relevance order and payload provenance. Relative
  date-window enforcement must compose with the accepted shared service.
- Semantic Scholar: preserve optional API-key placement, complete publication
  metadata, citation coercion and author ordering, and source-bounded Retry-After
  cooldown publication without counting a local synthetic failure as upstream.
- arXiv: preserve plain-phrase versus explicit syntax, exact request encoding,
  one-hop HTTPS same-host/port redirect policy, Atom namespace/mixed-content
  behavior, authors/identifiers/revision metadata and invalid-feed outcomes.
  `response.text` decoding is part of the unresolved shared parser/decoder
  compatibility work identified by A05; do not silently accept UTF-8-only parity.
- All engines: exact missing/null/type/error behavior or reviewed typed boundary,
  F08 body/time/cancellation bounds, real-loopback disposal and registry factories,
  engine-local metadata only, portal impact review and G01 aggregate handoff.

Map all named-engine tests, including shared tests outside their named files, to
real native cases. Extend this initial corpus for every uncovered regression;
never treat its current count as completeness. Keep original observations
immutable, record source/native differences explicitly, and require independent
full review plus exact-head CI before acceptance or issue closure.

## Direct-test supplement

`source-regressions.json` adds 38 actual whole-adapter observations from the
inspected tests listed in `REGRESSION_MAP.md`. Run the same exporter with
`--regressions` to reproduce it. The default exporter output remains byte-for-byte
identical to the original 67 records. The supplement includes the two-work
OpenAlex fixture, all original DOI forms and five malformed payloads, all three
leap-day bounds and seven invalid-bound cases, all ten arXiv query variants, and
seven Retry-After values. Every invalid-bound capture records zero HTTP requests.

This closes source-input gaps, not native runtime coverage. Admission failure,
elapsed-time behavior, shared cooldown/expiry, cancellation, cross-surface
assertions and the complete malformed-input matrix still require implementation
and executable native evidence. Both exporter modes require their own temporary
F01 invocation entries. Source adapters are unchanged from the baseline; newer
upstream publication-date service/API proof work belongs in F14 and T02/T06,
with A06 preserving the OpenAlex filtering assertions used by those layers.

## Native parser candidate

The arXiv decoded-text parser candidate now uses pinned Expat2.9.0 behind an
owned native helper, with a shared lifecycle pool, checked binary reply and
explicit operational quotas. The prior bespoke parser is retained as comparison
only, never a runtime fallback. See [candidate contracts and remaining review](ATOM_PARSER.md).
Original source observations stay immutable; two malformed-version upgrade
corrections and resource refusals are recorded explicitly. Full A06/HTTP/codec/
factory acceptance and GNU qualification remain pending.
