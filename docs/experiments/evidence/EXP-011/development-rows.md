# EXP-011 sanitized development rows

All 36 Jev responses were HTTP/schema-valid. “Card selection” is the frozen
`noul-card@0.65` output. The table omits response bodies and credentials.

| ID | Family | Labeled useful engines | Deterministic harness selection | Card selection | Top probability | Calls valid |
| --- | --- | --- | --- | --- | ---: | --- |
| d01 | packages/code | pypi, github, stackexchange | brave, github, stackexchange | duckduckgo, abuseipdb, cve | 0.74 | yes |
| d02 | packages/code | repology, npm | none | huggingface, greynoise, abuseipdb | 0.77 | yes |
| d03 | science | arxiv, openalex, semanticscholar | none | clinicaltrials, lever, cve | 0.79 | yes |
| d04 | science | huggingface, openalex, uniprot | none | internetarchive, arxiv, ashby | 0.69 | yes |
| d05 | medical | clinicaltrials, pubmed | none | greynoise, censys, cve | 0.80 | yes |
| d06 | medical | openfda, pubmed | none | nominatim, brave, dockerhub | 0.88 | yes |
| d07 | security | cve, nvd, epss | none | github, virustotal, crates | 0.71 | yes |
| d08 | security | mitreattack | none | ashby, mitreattack, oyez | 0.88 | yes |
| d09 | reference/archive | internetarchive | brave, wikipedia, internetarchive | crates, musicbrainz, otx | 0.84 | yes |
| d10 | reference/archive | oyez | none | abuseipdb, cve, fred | 0.86 | yes |
| d11 | general/news/social | hackernews, reddit, brave | none | clinicaltrials | 0.50 | yes |
| d12 | general/news/social | brave, wikipedia, hackernews | none | exploitdb | 0.48 | yes |

## Per-engine acquisition status

Not collected. The experiment stopped before Phase 4, so no engine was
dispatched and no contribution or result-quality claim is available.
