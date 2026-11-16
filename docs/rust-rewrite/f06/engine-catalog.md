# Native engine catalog (F06)

Generated from `catalog.json`, checked against 51 engine-local fragments.

| Name | Type | Categories | Auth | Cost | Jev role | Implementation |
| --- | --- | --- | --- | --- | --- | --- |
| `abuseipdb` | api | security, threat-intel | required | freemium | specialist | unavailable |
| `arxiv` | api | science, reference | none | free | specialist | unavailable |
| `ashby` | api | jobs | none | free | specialist | unavailable |
| `brave` | api | general, news, science, images | required | freemium | general | unavailable |
| `censys` | api | it, security | required | freemium | specialist | unavailable |
| `clinicaltrials` | api | medical, health, science | none | free | specialist | unavailable |
| `crates` | api | it, reference, packages | none | free | specialist | unavailable |
| `crtsh` | api | it, security | none | free | specialist | unavailable |
| `cve` | api | it, security | none | free | specialist | unavailable |
| `dehashed` | api | security, threat-intel | required | paid | specialist | unavailable |
| `dockerhub` | api | it, reference, packages | none | free | specialist | unavailable |
| `duckduckgo` | scrape | general, news, images | none | free | general | unavailable |
| `edgar` | api | finance, reference | none | free | specialist | unavailable |
| `epss` | api | security, threat-intel | none | free | specialist | unavailable |
| `europepmc` | api | science, reference, medical, health | none | free | specialist | unavailable |
| `exploitdb` | scrape | security, exploit | none | free | specialist | unavailable |
| `fred` | api | finance, reference, economics | required | freemium | specialist | unavailable |
| `github` | api | reference, github:code, github:issues, github:prs | none | free | specialist | unavailable |
| `google` | scrape | general, news | none | free | general | unavailable |
| `greenhouse` | api | jobs | none | free | specialist | unavailable |
| `greynoise` | api | security, threat-intel | none | free | specialist | unavailable |
| `hackernews` | api | general, news | none | free | broad_support | unavailable |
| `hibp` | api | security, reference | required | freemium | specialist | unavailable |
| `huggingface` | api | science, huggingface:datasets, huggingface:papers | none | free | specialist | unavailable |
| `intelx` | api | security, threat-intel | required | freemium | specialist | unavailable |
| `internetarchive` | api | reference, web:archive, historical | none | free | specialist | unavailable |
| `lever` | api | jobs | none | free | specialist | unavailable |
| `mitreattack` | api | security, reference | none | free | specialist | unavailable |
| `musicbrainz` | api | music, reference | none | free | specialist | unavailable |
| `nominatim` | api | geography, reference | none | free | specialist | unavailable |
| `npm` | api | it, reference, packages | none | free | specialist | unavailable |
| `nvd` | api | it, security | none | free | specialist | unavailable |
| `openalex` | api | science, reference | none | free | specialist | unavailable |
| `openfda` | api | medical, health, science, government | none | free | specialist | unavailable |
| `openlibrary` | api | books, reference | none | free | specialist | unavailable |
| `otx` | api | security, threat-intel | required | freemium | specialist | unavailable |
| `oyez` | api | reference, legal | none | free | specialist | unavailable |
| `pubchem` | api | science, reference, chemistry, medical | none | free | specialist | unavailable |
| `pubmed` | api | science, reference, medical, health | none | free | specialist | unavailable |
| `pypi` | api | it, reference, packages | none | free | specialist | unavailable |
| `reddit` | api | general, social, reddit:subreddit | none | free | broad_support | unavailable |
| `rubygems` | api | it, reference, packages | none | free | specialist | unavailable |
| `semanticscholar` | api | science, reference | none | free | specialist | unavailable |
| `shodan` | api | it, security | required | freemium | specialist | unavailable |
| `stackexchange` | api | general, reference, science, stackexchange:code, stackexchange:serverfault | none | free | broad_support | unavailable |
| `tmdb` | api | movies, entertainment | required | freemium | specialist | unavailable |
| `uniprot` | api | science, reference, biology, medical | none | free | specialist | unavailable |
| `urlhaus` | api | security, threat-intel | required | free | specialist | unavailable |
| `virustotal` | api | security, malware | required | freemium | specialist | unavailable |
| `vulncheck` | api | security, threat-intel | required | freemium | specialist | unavailable |
| `wikipedia` | api | general, science, reference | none | free | broad_support | unavailable |

`unavailable` means the descriptor is registered while the native provider port is not yet implemented. Secret values are never part of this catalog.
