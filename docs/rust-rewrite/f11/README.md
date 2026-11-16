# F11 pure ranking, scholarly grouping and retrieval classification

Baseline: `7cc420f7ffb61127750e77bf6130e225925f9186` (accepted F05).
`manifest.json` pins source blobs, the isolated exporter, Unicode properties,
the corpus digest, and every required case ID/kind. `goldens-*.jsonl` contains
751 deterministic cases from 60 existing source regression tests and additional
numeric/authority, Unicode identifier, 64-member and 64,000-byte boundaries.
All inputs are synthetic/public fixtures under the repository MIT license.

The required native command is:

```sh
cargo xtask check F11 --output /private/tmp/f11-evidence.json
```

Reviewed digest constants outside fixture metadata anchor the corpus, oracle,
Unicode properties and platform probe evidence. The gate rejects source drift,
changed anchored hashes,
missing/duplicate/unknown case coverage and inverted crate dependencies. It
replays ordered complete records and work groups, including member attribution,
checks input immutability, and exercises negative coverage mutations. No Python,
provider, DNS, socket, model or store executes in the gate. Build, format,
strict Clippy and F05 remain separate required delivery checks.

The developer-only oracle can be recaptured from this pinned source tree with
Python 3.14.7 / Unicode 16 and the repository development dependencies:

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/f11/oracle/export.py
```

It imports the Python source only to capture migration evidence; G08 retires it.
It runs the existing deterministic scholarly/rank regression tests, captures their
actual calls, and exports selected literal URL fixtures from the retrieval tests plus
explicit boundary cases. It never calls a provider or shared store. The source
and output locations are repository-local; inspect the resulting diff on refresh.
Python object cycles and lone-surrogate strings cannot enter the accepted typed
Rust JSON/string records and remain outside this corpus.

## APIs and downstream handoff

`ssx_application::ranking::Feeds` is an ordered map from engine names to row vectors;
it preserves insertion order and enforces unique feed names. `rank` implements
presence/default/unknown-strategy fallback and opt-in `reciprocal_rank_fusion`.
Presence counts raw duplicates against engine admission quotas, still merges
already admitted URLs after quota exhaustion, and uses stable encounter ties.
RRF ignores quotas as the source does, sorts engine names, contributes at the
first occurrence of each normalized URL using original raw rank, and ties on
normalized URL. Both retain the original display URL and minimum tier.

`scholarly::group_publications` runs before fusion/quota evaluation. It retains
source order, source positions and complete source payload provenance; declines
oversized/conflicting joins; distinguishes explicit version relationships from
manifestation identity; and supplements bibliographic fields only when complete
identifier maps match. Representative selection follows explicit chronology,
fully versioned same-base arXiv chronology, explicit query revision, then stable
source order. `POLICY_VERSION` remains `scholarly-work-v3`.

`retrieval::classify_retrieval_url` returns exact status/reason/scheme/URL fields.
Only `ok` carries the captured URL, unchanged. Classification is structural and
numeric-only. Hostnames such as `localtest.me` and `127.0.0.1.nip.io` remain
eligible without resolution. Downstream retrieval owns post-resolution checks.
Production Linux numeric eligibility is authoritative. Darwin's numeric-only
`AI_NUMERICHOST` additionally accepts decimal leading-zero quads and wrapping
single-component integers; copying those candidates would turn malformed
Linux inputs such as `018.1.1.1` and `4429711368` into eligible targets. Native
classification preserves the bounded Linux inet_aton/WHATWG grammar and does
not add those Darwin candidates. `numeric-platforms.json` records independent
standard-library numeric-only observations on Linux and Darwin. Linux source
classification there is derived from those observed candidates and the exact
source branch order; whole-source remote execution was denied by automatic
approval review. Native Linux CI replay is required before merge. Seven retrieval
cases retain their Darwin source observations alongside explicit Linux expected
outputs. No DNS is used. WHATWG/canonical candidate precedence stays unchanged.

F14 composes these pure functions into execution. F15 owns later reranking.
F05 HTTP/MCP projections keep their accepted typed validation and receive
precomputed classification from execution rather than implementing a classifier.
The shared DOI normalizer/paper-field codec is extracted unchanged into the pure
`ssx-publication` leaf, with existing `ssx-wire` exports preserved (ADR 003).

Portal impact: this adds pure native algorithms and changes no running Python
portal contract. Future native portal integration must retain these result and
precomputed retrieval projections and run the portal acceptance gate.

## Independent review corrections

The reviewed corpus now includes urllib final-segment parameter separation and
scheme-specific reconstruction. Empty final parameters disappear; nonempty
parameters reinsert after the path; semicolons in earlier segments remain path
text. Scholarly URL identification reads that parsed path, so PubMed
`/123;foo` reports PMID `123`. PMC's complete optional `pmc/` plus `articles/PMC`
pattern uses Python regex `IGNORECASE`, including dotted/dotless I and long S;
uppercase paths match, and near-miss or extra-segment paths remain unrecognized.
Both fusion strategies have source-captured empty/nonempty-parameter cases.
