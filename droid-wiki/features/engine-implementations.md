# Engine implementations

Active contributors: Magnus Hedemark

## Purpose

SlopSearX ships with 48 pre-built engine adapters spanning 9 domains. Each engine is one file in `engines/`, registered via `@register_engine`, and requires zero changes to the orchestrator.

## Engine types

| Type | Count | Description |
|---|---|---|
| **API** | 45 | Structured JSON API calls via httpx. Reliable, well-documented endpoints |
| **Scrape** | 3 | HTTP GET/POST with stealth headers + HTML parsing via lxml. DuckDuckGo, Google, Exploit-DB |

## Domain breakdown

### General / Web (6 engines — all Tier 1)

| Engine | File | Auth | Categories |
|---|---|---|---|
| Brave Search | `engines/brave.py` | `ENGINE_BRAVE_API_KEY` | general, news, science, images |
| DuckDuckGo | `engines/duckduckgo.py` | None (scrape) | general, news |
| Google | `engines/google.py` | None (scrape) | general, news |
| Hacker News | `engines/hackernews.py` | None | general, news |
| Reddit | `engines/reddit.py` | None | general, social, reddit:subreddit |
| Wikipedia | `engines/wikipedia.py` | None | general, science, reference |

### Developer / Packages (8 engines)

| Engine | File | Auth | Categories |
|---|---|---|---|
| Crates.io | `engines/crates.py` | None | general, it, reference, packages |
| Docker Hub | `engines/dockerhub.py` | None | general, it, reference, packages |
| GitHub | `engines/github.py` | `GITHUB_TOKEN` | general, reference, github:code, github:issues, github:prs |
| npm | `engines/npm.py` | None | general, it, reference, packages |
| PyPI | `engines/pypi.py` | None | general, it, reference, packages |
| Repology | `engines/repology.py` | None | general, it, reference, packages |
| RubyGems | `engines/rubygems.py` | None | general, it, reference, packages |
| Stack Exchange | `engines/stackexchange.py` | Optional | general, reference, science, stackexchange:code, stackexchange:serverfault |

### Science & Research (7 engines)

arXiv, HuggingFace (with `huggingface:datasets`, `huggingface:papers` sub-categories), Internet Archive, OpenAlex, Open Library, Semantic Scholar, UniProt.

### Medical / Health (4 engines)

ClinicalTrials.gov, openFDA, PubChem, PubMed.

### Security / Threat Intelligence (17 engines)

AbuseIPDB, AlienVault OTX, Censys, CRT.sh, CVE Program (MITRE), DeHashed, Exploit-DB (scrape), FIRST EPSS, GreyNoise, Have I Been Pwned, IntelX, MITRE ATT&CK, NVD (NIST), Shodan, URLhaus, VirusTotal, VulnCheck.

### Finance / Economics (2 engines)

FRED, SEC EDGAR.

### Media & Entertainment (2 engines)

MusicBrainz, TMDB.

### Geography / GIS (1 engine)

Nominatim (OpenStreetMap).

### Legal (1 engine)

Oyez (SCOTUS).

## Adapter contract

Every engine follows the adapter contract:

1. One file in `engines/` with a `@register_engine` decorated class
2. `name`, `display_name`, `engine_type`, `categories` set
3. `async def search(query, params)` returns `AdapterResponse` — never raises
4. Import added to `engines/__init__.py`
5. Row in README.md Engines table

See the [Adapter interface system page](../systems/adapter-interface.md) for the full contract and the [Adapter reference](../../docs/ENGINE_ADAPTERS.md) for implementation details.

## Adding a new engine

1. Create `engines/myengine.py` with `@register_engine` class
2. Add `from engines import myengine` to `engines/__init__.py`
3. Add a row to the README.md Engines table
4. Write tests in `tests/test_adapters.py`
5. All new engines default to Tier 2

## Category system

Engines declare SearXNG-compatible categories with optional namespace-prefixed sub-categories. Categories determine:
- Which `?categories=` queries include the engine (OR semantics)
- Topic-based routing via QueryRouter
- Configuration discovery via `/config` endpoint

Operators can override categories without modifying code via config.yaml or env vars:
```bash
ENGINE_MYENG_CATEGORIES=general,news
ENGINE_MYENG_CATEGORIES_ADD=finance
ENGINE_MYENG_CATEGORIES_REMOVE=images
```

## Key source files

| File | Description |
|---|---|
| `engines/*.py` | Individual engine adapters (48 files) |
| `engines/__init__.py` | Imports all engine modules, triggers registration |
| `docs/ENGINE_ADAPTERS.md` | Full adapter reference with contract rules and built-in table |
