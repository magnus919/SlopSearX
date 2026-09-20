# EXP-014 sanitized acquisition rows

Selections at the lowest and recommended thresholds are shown. Outcome tokens
are `engine:status:result_count`; rank is the first essential result in the
specialist-aware top ten.

| ID | Family | Essential | Selected @0.45 | Selected @0.65 | @0.65 outcomes | Essential rank |
| --- | --- | --- | --- | --- | --- | ---: |
| q01 | packages | crates | crates, github | crates | crates:ok:10 | 1 |
| q02 | packages | dockerhub | dockerhub | dockerhub | dockerhub:ok:10 | 1 |
| q03 | packages | rubygems | rubygems | rubygems | rubygems:ok:0 | - |
| q04 | packages | npm | github, npm | npm | npm:ok:10 | 1 |
| q05 | packages | repology | repology | repology | repology:error:0 | - |
| q06 | science/medical | clinicaltrials | clinicaltrials, openalex, openfda, pubmed, semanticscholar | clinicaltrials, pubmed | clinicaltrials:ok:0; pubmed:ok:4 | - |
| q07 | science/medical | arxiv | arxiv, huggingface, openalex, semanticscholar | arxiv, huggingface, openalex, semanticscholar | arxiv:ok:5; huggingface:ok:0; openalex:ok:10; semanticscholar:rate_limited:0 | 1 |
| q08 | science/medical | openfda | openfda | openfda | openfda:ok:10 | 1 |
| q09 | science/medical | pubchem | pubchem | pubchem | pubchem:ok:0 | - |
| q10 | science/medical | uniprot | pubmed, semanticscholar, uniprot | pubmed, uniprot | pubmed:ok:10; uniprot:ok:0 | - |
| q11 | security | nvd, epss | cve, epss, nvd | cve, epss, nvd | cve:ok:1; epss:ok:1; nvd:ok:1 | 1 |
| q12 | security | mitreattack | mitreattack | mitreattack | mitreattack:ok:0 | - |
| q13 | security | crtsh | crtsh | crtsh | crtsh:error:0 | - |
| q14 | security | exploitdb | cve, exploitdb, nvd | cve, exploitdb, nvd | cve:ok:0; exploitdb:ok:0; nvd:ok:0 | - |
| q15 | security | urlhaus | urlhaus | urlhaus | urlhaus:error:0 | - |
| q16 | structured | openlibrary | openlibrary | openlibrary | openlibrary:ok:0 | - |
| q17 | structured | nominatim | none | none | none | - |
| q18 | structured | musicbrainz | musicbrainz | musicbrainz | musicbrainz:ok:10 | 1 |
| q19 | structured | oyez | oyez | oyez | oyez:ok:10 | 1 |
| q20 | structured | edgar | edgar | edgar | edgar:ok:10 | 1 |
| q21 | jobs/ml | ashby | ashby | ashby | ashby:ok:0 | - |
| q22 | jobs/ml | greenhouse | greenhouse | greenhouse | greenhouse:ok:0 | - |
| q23 | jobs/ml | lever | lever | lever | lever:ok:0 | - |
| q24 | jobs/ml | huggingface | huggingface | huggingface | huggingface:ok:0 | - |
| q25 | jobs/ml | github | github | none | none | - |
| q26-q30 | general | none | none | none | none | not applicable |
