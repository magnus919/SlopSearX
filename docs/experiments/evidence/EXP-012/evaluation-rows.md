# EXP-012 sanitized held-out routing rows

All 30 original-query and 30 paraphrase Jev calls were valid. Candidate uses
the frozen 0.65 threshold. No search engines were dispatched.

| ID | Family | Labeled useful | Production baseline | Jev candidate | Paraphrase candidate |
| --- | --- | --- | --- | --- | --- |
| e01 | packages/code | crates, github | brave, github, stackexchange | crates, github, google | crates, github, google |
| e02 | packages/code | dockerhub, github | wikipedia, stackexchange, brave | dockerhub, google, brave | dockerhub, stackexchange, google |
| e03 | packages/code | rubygems, github | wikipedia, stackexchange, brave | rubygems, github, google | rubygems, github, stackexchange |
| e04 | packages/code | npm, github, stackexchange | brave, github, stackexchange | npm, google, stackexchange | npm, stackexchange, google |
| e05 | packages/code | pypi, github | brave, github, stackexchange | pypi, google, github | github, pypi |
| e06 | science | arxiv, openalex, semanticscholar | brave, arxiv, semanticscholar | semanticscholar, arxiv, openalex | semanticscholar, arxiv, openalex |
| e07 | science | openalex, semanticscholar | brave, arxiv, semanticscholar | semanticscholar, openalex, arxiv | semanticscholar, openalex, arxiv |
| e08 | science | arxiv, openalex | wikipedia, stackexchange, brave | arxiv, openalex, semanticscholar | openalex, semanticscholar, arxiv |
| e09 | science | huggingface, openalex | brave, arxiv, semanticscholar | huggingface, google, brave | huggingface, google, brave |
| e10 | science | openalex, semanticscholar | wikipedia, stackexchange, brave | openalex, semanticscholar, arxiv | semanticscholar, openalex, google |
| e11 | medical | clinicaltrials, pubmed | wikipedia, stackexchange, brave | clinicaltrials, google, brave | clinicaltrials, pubmed, google |
| e12 | medical | openfda | wikipedia, stackexchange, brave | openfda, google | openfda |
| e13 | medical | pubmed, clinicaltrials | wikipedia, stackexchange, brave | pubmed, semanticscholar, openalex | pubmed, semanticscholar, openalex |
| e14 | medical | pubchem | wikipedia, stackexchange, brave | pubchem, wikipedia, google | pubchem |
| e15 | medical | uniprot, pubmed | wikipedia, stackexchange, brave | uniprot, pubmed, wikipedia | uniprot, wikipedia |
| e16 | security | cve, nvd, epss | wikipedia, stackexchange, brave | nvd, epss, cve | epss, nvd, google |
| e17 | security | mitreattack | wikipedia, stackexchange, brave | mitreattack, google, stackexchange | mitreattack |
| e18 | security | urlhaus | wikipedia, stackexchange, brave | urlhaus | urlhaus |
| e19 | security | exploitdb, cve, nvd | wikipedia, stackexchange, brave | exploitdb, google, github | exploitdb, github, google |
| e20 | security | crtsh | wikipedia, stackexchange, brave | crtsh | crtsh |
| e21 | reference/archive | openlibrary | wikipedia, stackexchange, brave | openlibrary, google, duckduckgo | openlibrary |
| e22 | reference/archive | internetarchive | brave, wikipedia, duckduckgo | google | google |
| e23 | reference/archive | nominatim | wikipedia, stackexchange, brave | nominatim, wikipedia, duckduckgo | nominatim |
| e24 | reference/archive | musicbrainz | wikipedia, stackexchange, brave | musicbrainz, wikipedia, google | musicbrainz, wikipedia |
| e25 | reference/archive | oyez | wikipedia, stackexchange, brave | oyez, wikipedia, google | oyez, google, brave |
| e26 | general/news/social | brave, hackernews, reddit | brave, github, stackexchange | google, brave, duckduckgo | google, brave, github |
| e27 | general/news/social | reddit, brave | wikipedia, stackexchange, brave | reddit, google, duckduckgo | reddit, google, brave |
| e28 | general/news/social | wikipedia, brave, hackernews | wikipedia, stackexchange, brave | google, wikipedia, brave | google, brave, duckduckgo |
| e29 | general/news/social | brave, hackernews, reddit | wikipedia, stackexchange, brave | google, brave, hackernews | google, brave, duckduckgo |
| e30 | general/news/social | wikipedia, brave | brave, wikipedia, duckduckgo | wikipedia, google, brave | google, brave, duckduckgo |

## Acquisition status

Not run. The candidate failed three offline routing gates, so the registered
protocol prohibited Brave and free-engine acquisition calls.
