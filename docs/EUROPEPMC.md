# Europe PMC metadata integration

Decision (2026-10-07): go for an **opt-in Tier 2 metadata adapter**. Europe PMC
adds biomedical preprints and source identifiers alongside PubMed. A small
availability/coverage probe does not establish a production relevance gain.

Enable with `ENGINE_EUROPEPMC_ENABLED=true`. No key, account or fee is required
for the public search endpoint. Defaults: 10 results, 5 second timeout,
0.5 requests/second distributed engine rate limit. This is a conservative
operator limit, not a claimed vendor quota. Shared Valkey caching, policy and
snapshots apply normally. No paid endpoint or fallback is used.

The adapter requests only `resultType=lite`, never abstracts, full text or
annotations. All records retain Europe PMC source/record URLs so attribution survives
merging with PubMed results. PMID overlaps therefore remain separate results;
identifier-based deduplication needs a shared attribution-preserving design. PMID,
PMCID and DOI are preserved without assuming that every record has all three.
Full-text/open-access flags describe upstream availability and do not grant
reuse rights. Preprints carry an explicit not-peer-reviewed notice.

Every content snippet includes Europe PMC / EMBL-EBI attribution and the
provider URL, visible in JSON, YAML, MCP and portal results. Publisher and other
original owners retain rights; operators must review article rights before
any downstream republication. No abstract/full-text mode is exposed.

[REST API](https://europepmc.org/RestfulWebService),
[EMBL-EBI terms](https://www.ebi.ac.uk/about/terms-of-use/), and
[training](https://www.ebi.ac.uk/training/online/courses/embl-ebi-programmatically/europe-pmc-programmatically/)
are the authoritative references. The current platform terms (revised
2024-02-05, checked 2026-10-07) expect attribution, preserve original owner
rights, and prohibit load that obstructs others. No numeric vendor quota was
established. API privacy notice acceptance remains an operator responsibility.

## Bounded evidence

Public GET requests on 2026-10-07, five records each, no credentials:

| Query | Europe PMC hits / seconds | PubMed hits / seconds |
|---|---:|---:|
| CRISPR sickle cell | 4,423 / 0.99 | 497 / 0.28 |
| malaria vaccine | 51,483 / 0.68 | 12,553 / 0.36 |

These counts reflect different query semantics/ranking and are not relevance
scores. Both Europe PMC top fives were MED records; the captured CRISPR top
five and PubMed's top five shared no PMIDs. Manual titles show biomedical fit
but also a CRISPR detection result with weak sickle-cell fit. The deliberately
scoped `SRC:PPR AND COVID-19` returned 93,775 preprints with five PPR identifiers,
a concrete source class absent from PubMed's PMID-only interface. The initial
incorrect `SOURCE:PPR AND long covid` query returned zero; use `SRC`.
All six comparison searches succeeded; this single probe gives no long-term
availability or latency SLO. Captured lite responses are replayed in
`tests/fixtures/europepmc/`; no patient or credential data is included.

No numbered pagination, language, SafeSearch or publication-date enforcement
is claimed. Cursor state cannot be mapped honestly onto stateless numbered
pages without further design. Date and availability metadata are returned
only when upstream provides them; annotations/provenance are out of scope.

## Portal impact

An enabled source appears in shared capability discovery and science scopes;
results show attribution and the preprint notice through the existing HTML
formatter. Defaults remain disabled pending broader relevance evaluation.
The shared portal contract gate and Jev completeness tests are required.

An explicit single-request contract probe is available with
`SLOPSEARX_EUROPEPMC_LIVE=1 pytest --no-cov -q tests/test_europepmc.py -k bounded_live`.
This sends a public biomedical preprint query; routine tests remain offline.
