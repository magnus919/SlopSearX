# EXP-013 sanitized acquisition rows

The specialist outcome token is `engine:status:result_count`. Contribution
means at least one result from that specialist survived production merging into
the candidate top ten. General-engine outcomes are summarized separately and
raw result bodies are not retained.

| ID | Family | Labeled specialists | Selected specialists | Specialist outcomes | Top-ten contributors | Target covered |
| --- | --- | --- | --- | --- | --- | --- |
| e01 | packages/code | crates, github | crates, github | github:error:0; crates:ok:10 | crates | yes |
| e02 | packages/code | dockerhub | dockerhub | dockerhub:ok:10 | none | no |
| e03 | packages/code | rubygems, github | rubygems, github | rubygems:ok:1; github:error:0 | none | no |
| e04 | packages/code | npm, github | npm | npm:ok:10 | npm | yes |
| e05 | packages/code | pypi | pypi | pypi:ok:0 | none | no |
| e07 | science/medical | pubmed, openalex | pubmed, semanticscholar, openalex, arxiv | arxiv:ok:5; semanticscholar:rate_limited:0; openalex:ok:10; pubmed:ok:10 | none | no |
| e08 | science/medical | clinicaltrials, pubmed | clinicaltrials, pubmed | clinicaltrials:ok:0; pubmed:rate_limited:0 | none | no |
| e09 | science/medical | arxiv, openalex | arxiv, semanticscholar, openalex, huggingface | arxiv:ok:5; semanticscholar:rate_limited:0; openalex:ok:10; huggingface:ok:0 | none | no |
| e10 | science/medical | openfda | openfda | openfda:ok:10 | none | no |
| e11 | science/medical | pubchem | pubchem | pubchem:ok:0 | none | no |
| e13 | security | nvd, epss | nvd, epss, cve, exploitdb | nvd:ok:1; epss:ok:1; cve:ok:1; exploitdb:ok:0 | cve, nvd | yes |
| e14 | security | mitreattack | mitreattack | mitreattack:ok:0 | none | no |
| e15 | security | urlhaus | urlhaus, crtsh | urlhaus:error:0; crtsh:timeout:0 | none | no |
| e16 | security | exploitdb, nvd | exploitdb, nvd, github, cve | exploitdb:ok:0; nvd:ok:0; github:error:0; cve:ok:0 | none | no |
| e17 | security | crtsh | crtsh | crtsh:timeout:0 | none | no |
| e19 | structured/reference | openlibrary | openlibrary | openlibrary:ok:0 | none | no |
| e20 | structured/reference | pubchem | pubchem | pubchem:ok:0 | none | no |
| e21 | structured/reference | nominatim | nominatim | nominatim:ok:0 | none | no |
| e22 | structured/reference | musicbrainz | musicbrainz | musicbrainz:ok:10 | none | no |
| e23 | structured/reference | oyez | oyez | oyez:ok:10 | none | no |
| e25 | economy/jobs/media | edgar | edgar | edgar:ok:10 | none | no |
| e26 | economy/jobs/media | ashby | ashby | ashby:ok:0 | none | no |
| e27 | economy/jobs/media | lever | lever | lever:ok:0 | none | no |
| e28 | economy/jobs/media | musicbrainz | musicbrainz | musicbrainz:ok:10 | none | no |
| e29 | economy/jobs/media | edgar | edgar | edgar:ok:10 | none | no |
| e31-e35 | general/no-specialist | none | none | none | none | not applicable |

General-result availability passed on all five broad rows. Across all selected
specialist calls: 32 returned `ok`, four `error`, three `rate_limited`, and two
`timeout`; 15 specialist calls returned at least one result, but only four
specialist engines contributed to a final top-ten view.
