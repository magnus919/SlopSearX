---
name: slopsearx
description: Cloud-native, stateless, AI-agent-first meta search engine with 48 plugin engines. SearXNG-compatible API, category routing, distributed rate limiting, and agent-native YAML+Markdown output.
---

# SlopSearX

SlopSearX is a horizontally scalable meta search engine designed for AI agent consumption. It fans out queries to 48 engines in parallel, deduplicates and ranks results, and returns structured output in SearXNG-compatible JSON or agent-native YAML+Markdown.

## How to Use

### Starting the Server

```bash
# From the project root
uvicorn slopsearx.server:app --host 0.0.0.0 --port 8080
```

### Using the CLI (ssx)

The `ssx` CLI wraps all API endpoints in an agent-friendly way:

```bash
# Search across all engines (YAML output by default)
python ssx search "your query"

# Search with category filter
python ssx search "your query" --categories science

# Search specific engines
python ssx search "your query" --engines arxiv,wikipedia

# List all engines with status
python ssx engines

# Health check
python ssx health

# Show engine config/categories
python ssx config

# Output as JSON (for programmatic use)
python ssx search "your query" --json
```

### Direct API Usage

| Endpoint | Description |
|---|---|
| `GET /search?q=<query>&format=json` | SearXNG-compatible JSON |
| `GET /search?q=<query>&format=yaml` | Agent-native YAML+Markdown |
| `GET /search?q=<query>&categories=science,news` | Category filter (OR) |
| `GET /search?q=<query>&engines=brave,wikipedia` | Explicit engine selection |
| `GET /search?q=<query>&language=fr` | Language filter |
| `GET /search?q=<query>&time_range=year` | Time range filter |
| `GET /search?q=<query>&pageno=2` | Pagination |
| `GET /health` | Per-engine health check |
| `GET /metrics` | OpenMetrics for Prometheus |
| `GET /config` | Categories to engines mapping |

### Example: Agent-Native Search

```bash
curl 'http://localhost:8080/search?q=quantum+computing+breakthroughs+2025&format=yaml'
```

Returns YAML-frontmatter with structured results followed by a Markdown summary - ideal for AI agent processing.

### Engine Categories

Engines are organized by categories. Use `?categories=` to narrow scope:

- `general` — Brave, DuckDuckGo, Google, Wikipedia, Reddit, Hacker News
- `science` — arXiv, Semantic Scholar, OpenAlex, PubMed, PubChem, HuggingFace
- `security` — NVD, CVE, Shodan, Censys, VirusTotal, AbuseIPDB, OTX, URLhaus
- `reference` — Wikipedia, Stack Exchange, GitHub, Open Library, MITRE ATT&CK
- `it` — NVD, CVE, Shodan, Censys, CRT.sh, npm, PyPI, Crates.io
- `news` — Brave, DuckDuckGo, Google, Hacker News
- `finance` — FRED, SEC EDGAR
- `medical` — PubMed, PubChem, ClinicalTrials.gov, openFDA
- `music` — MusicBrainz
- `movies` — TMDB
- `geography` — Nominatim
- `social` — Reddit
- `books` — Open Library
- `legal` — Oyez
- `packages` — npm, PyPI, Crates.io, RubyGems, Docker Hub, Repology

### Engine-Specific Sub-Categories

Some engines support sub-categories for fine-grained routing:

- `github:code` — GitHub code search
- `github:issues` — GitHub issues/PRs search
- `huggingface:datasets` — HuggingFace dataset search
- `huggingface:papers` — HuggingFace paper search
- `reddit:subreddit` — Reddit subreddit-scoped search
- `stackexchange:code` — Stack Overflow code search
- `stackexchange:serverfault` — Server Fault search

### Configuration

Engines are configured via three-layer priority (env vars > config file > defaults):

```bash
# Required API keys as env vars
export ENGINE_BRAVE_API_KEY="your_key"
export ENGINE_GITHUB_TOKEN="ghp_..."
export ENGINE_SHODAN_API_KEY="your_key"

# Category overrides
export ENGINE_BRAVE_CATEGORIES="general,news,science"
export ENGINE_WIKIPEDIA_CATEGORIES_ADD="education"
```

A mounted YAML config file at `/etc/slopsearx/config.yaml` provides per-engine tuning.

## References

### General / Web

| Engine | File | Type | Auth |
|---|---|---|---|
| [Brave Search](references/brave.md) | engines/brave.py | API | `ENGINE_BRAVE_API_KEY` |
| [DuckDuckGo](references/duckduckgo.md) | engines/duckduckgo.py | Scrape | None |
| [Google](references/google.md) | engines/google.py | Scrape | None |
| [Hacker News](references/hackernews.md) | engines/hackernews.py | API | None |
| [Reddit](references/reddit.md) | engines/reddit.py | API | None |
| [Wikipedia](references/wikipedia.md) | engines/wikipedia.py | API | None |

### Developer / Package Registries

| Engine | File | Type | Auth |
|---|---|---|---|
| [Crates.io](references/crates.md) | engines/crates.py | API | None |
| [Docker Hub](references/dockerhub.md) | engines/dockerhub.py | API | None |
| [GitHub](references/github.md) | engines/github.py | API | `GITHUB_TOKEN` |
| [npm](references/npm.md) | engines/npm.py | API | None |
| [PyPI](references/pypi.md) | engines/pypi.py | API | None |
| [Repology](references/repology.md) | engines/repology.py | API | None |
| [RubyGems](references/rubygems.md) | engines/rubygems.py | API | None |
| [Stack Exchange](references/stackexchange.md) | engines/stackexchange.py | API | Optional app key |

### Science & Research

| Engine | File | Type | Auth |
|---|---|---|---|
| [arXiv](references/arxiv.md) | engines/arxiv.py | API | None |
| [HuggingFace](references/huggingface.md) | engines/huggingface.py | API | `HF_TOKEN` (optional) |
| [Internet Archive](references/internetarchive.md) | engines/internetarchive.py | API | None |
| [OpenAlex](references/openalex.md) | engines/openalex.py | API | None |
| [Open Library](references/openlibrary.md) | engines/openlibrary.py | API | None |
| [Semantic Scholar](references/semanticscholar.md) | engines/semanticscholar.py | API | Optional API key |
| [UniProt](references/uniprot.md) | engines/uniprot.py | API | None |

### Medical / Health

| Engine | File | Type | Auth |
|---|---|---|---|
| [ClinicalTrials.gov](references/clinicaltrials.md) | engines/clinicaltrials.py | API | None |
| [openFDA](references/openfda.md) | engines/openfda.py | API | None |
| [PubChem](references/pubchem.md) | engines/pubchem.py | API | None |
| [PubMed](references/pubmed.md) | engines/pubmed.py | API | None |

### Security / Threat Intelligence

| Engine | File | Type | Auth |
|---|---|---|---|
| [AbuseIPDB](references/abuseipdb.md) | engines/abuseipdb.py | API | `ENGINE_ABUSEIPDB_API_KEY` |
| [AlienVault OTX](references/otx.md) | engines/otx.py | API | `ENGINE_OTX_API_KEY` |
| [Censys](references/censys.md) | engines/censys.py | API | `ENGINE_CENSYS_API_KEY` + `_API_SECRET` |
| [CRT.sh](references/crtsh.md) | engines/crtsh.py | API | None |
| [CVE Program (MITRE)](references/cve.md) | engines/cve.py | API | None |
| [DeHashed](references/dehashed.md) | engines/dehashed.py | API | `ENGINE_DEHASHED_API_KEY` |
| [Exploit-DB](references/exploitdb.md) | engines/exploitdb.py | Scrape | None |
| [FIRST EPSS](references/epss.md) | engines/epss.py | API | None |
| [GreyNoise](references/greynoise.md) | engines/greynoise.py | API | `ENGINE_GREYNOISE_API_KEY` (optional) |
| [Have I Been Pwned](references/hibp.md) | engines/hibp.py | API | `ENGINE_HIBP_API_KEY` |
| [IntelX](references/intelx.md) | engines/intelx.py | API | `ENGINE_INTELX_API_KEY` |
| [MITRE ATT&CK](references/mitreattack.md) | engines/mitreattack.py | API | None |
| [NVD](references/nvd.md) | engines/nvd.py | API | `ENGINE_NVD_API_KEY` (optional) |
| [Shodan](references/shodan.md) | engines/shodan.py | API | `ENGINE_SHODAN_API_KEY` |
| [URLhaus](references/urlhaus.md) | engines/urlhaus.py | API | None |
| [VirusTotal](references/virustotal.md) | engines/virustotal.py | API | `ENGINE_VIRUSTOTAL_API_KEY` |
| [VulnCheck](references/vulncheck.md) | engines/vulncheck.py | API | `ENGINE_VULNCHECK_API_KEY` |

### Finance / Economics

| Engine | File | Type | Auth |
|---|---|---|---|
| [FRED](references/fred.md) | engines/fred.py | API | `ENGINE_FRED_API_KEY` |
| [SEC EDGAR](references/edgar.md) | engines/edgar.py | API | None |

### Media & Entertainment

| Engine | File | Type | Auth |
|---|---|---|---|
| [MusicBrainz](references/musicbrainz.md) | engines/musicbrainz.py | API | None |
| [TMDB](references/tmdb.md) | engines/tmdb.py | API | `ENGINE_TMDB_API_KEY` |

### Geography / GIS

| Engine | File | Type | Auth |
|---|---|---|---|
| [Nominatim](references/nominatim.md) | engines/nominatim.py | API | None |

### Legal

| Engine | File | Type | Auth |
|---|---|---|---|
| [Oyez](references/oyez.md) | engines/oyez.py | API | None |
