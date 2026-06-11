# Lore

SlopSearX was created on June 8-10 2026 by Magnus Hedemark. The project reached 50 commits across three days.

## Eras

### Genesis (Jun 8 2026)

Initial commit `893dcbd` titled "Initial commit" contained the entire project skeleton. Core architecture established:

- Adapter interface, merger, config, server, formatter, caching, rate limiting, and metrics modules.
- First engine adapters: Brave, Wikipedia, DuckDuckGo, Google, GitHub, Hacker News, HuggingFace, arXiv, Semantic Scholar.

### Expansion (Jun 9 2026)

Sixteen more commits adding:

- Stack Exchange, OpenAlex, and Internet Archive adapters.
- Internet Archive adapter fix routing domain queries to Wayback CDX API (PR #33).
- Tests for tier 1 adapters, categories, and server.
- Documentation: README, ENGINE_ADAPTERS.md, AGENTS.md, CONTRIBUTING.md, spec.md, issue templates, PR template.
- CI pipeline.
- Docker and Kubernetes deployment manifests.
- Grafana monitoring dashboard.
- Reddit adapter with social category and subreddit sub-category.

### Professional expansion (Jun 9-10 2026)

The project grew from ~12 engines to 48, adding several major categories of adapters and four new core subsystems:

**17 security / threat-intel engines:** Shodan, Censys, VirusTotal, Have I Been Pwned, AlienVault OTX, AbuseIPDB, VulnCheck, IntelX, DeHashed, CRT.sh, URLhaus, FIRST EPSS, GreyNoise, Exploit-DB, MITRE ATT&CK, CVE Program, NVD.

**19 profession-specific engines:**
- Developer registries: PyPI, npm, crates.io, RubyGems, Repology, Docker Hub.
- Media & entertainment: MusicBrainz, Open Library, TMDB.
- Medical & health: PubMed, PubChem, ClinicalTrials.gov.
- Legal: Oyez.
- Finance: SEC EDGAR, FRED.
- Geography: Nominatim (OpenStreetMap).
- Government: openFDA.
- Biology: UniProt.

**Four new core subsystems:**
- **QueryRouter** (`slopsearx/router.py`) — topic-based query routing that dispatches to relevant engines only, reducing unnecessary API calls and improving latency.
- **ProxyPool** (`slopsearx/proxypool.py`) — configurable proxy rotation infrastructure for scrape-based adapters, helping evade IP bans and CAPTCHA walls.
- **SuggestionService** (`slopsearx/suggest.py`) — background task that fetches search suggestions from engine suggest APIs, populating the SearXNG `suggestions` response field.
- **EngineStatsTracker** (`slopsearx/stats.py`) — per-engine quality telemetry stored in Valkey, supporting adaptive engine selection and observability.

**Replacement of broken upstream adapters:** Three adapters were swapped out — CourtListener (unreliable legal database), USPTO (API changes), and Data.gov (inconsistent availability) — replaced with working alternatives Oyez (SCOTUS rulings), RubyGems (package registry), and openFDA (government health data).

Total commit count reached ~50 across all eras.

## Longest-standing features

All core features (adapter interface, merger, config, server, formatter, caching, rate limiting, metrics) date from the initial commit.

## Deprecated features

None. The project has not yet had any feature become obsolete.

## Major rewrites

None. The architecture has remained stable since the initial commit, with new subsystems and adapters added via the plugin interface.

## Growth trajectory

The project grew from 0 to 15,654 lines of Python in three days. Started with 7 engines and a complete core in the initial commit. Expanded to 12 engines on day 2, then to 48 engines with 4 new core subsystems and full test coverage on day 3.
