# SlopSearX

**Cloud-native, stateless, AI-agent-first meta search engine.** Drop-in SearXNG replacement for the GroktoCrawl stack.

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Docker](https://github.com/magnus919/SlopSearX/actions/workflows/docker.yml/badge.svg)](https://github.com/magnus919/SlopSearX/actions/workflows/docker.yml)

---

SlopSearX is a horizontally scalable, stateless meta search engine designed for AI agent consumption. It replaces SearXNG in the GroktoCrawl stack with:

- **JSON output by default** — structured responses designed for programmatic consumption
- **YAML+Markdown native output** — structured + readable for AI agent contexts via `format=yaml`
- **SearXNG-compatible API** — drop-in replacement for existing consumers
- **Plugin engine adapters** — one file per engine, `@register_engine`, zero orchestrator changes
- **Category routing** — SearXNG-compatible taxonomy with sub-categories and env-var overrides
- **Distributed rate limiting** — Valkey-backed sliding windows, correct at 50+ replicas
- **Response caching** — Valkey-backed, category-aware TTL, 150x speedup on cache hits
- **OpenMetrics observability** — `/metrics` endpoint, per-engine counters + latency + status
- **Stateless, cloud-native** — no local volumes, no persistent DB, all replicas interchangeable
- **Hybrid config** — env vars for secrets, optional mounted file for engine tuning

## API

| Endpoint | Description |
|---|---|
| `GET /search?q=...&format=json` | SearXNG-compatible JSON (default) |
| `GET /search?q=...&format=yaml` | YAML+Markdown agent-native output |
| `GET /search?q=...&categories=science,news` | Filter by category (OR semantics) |
| `GET /search?q=...&engines=brave,wikipedia` | Explicit engine selection |
| `GET /health` | Per-engine health check with status |
| `GET /metrics` | OpenMetrics for Prometheus scraping |
| `GET /config` | Categories→engines mapping for runtime discovery |

## Engines (48)

### General / Web

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Brave Search](https://brave.com/search/api/) | API | `ENGINE_BRAVE_API_KEY` | general, news, science, images |
| [DuckDuckGo](https://duckduckgo.com/) | Scrape | None | general, news |
| [Google](https://google.com/) | Scrape | None | general, news |
| [Hacker News](https://news.ycombinator.com/) | API | None | general, news |
| [Reddit](https://reddit.com/) | API | None | general, social, reddit:subreddit |
| [Wikipedia](https://www.wikipedia.org/) | API | None | general, science, reference |

### Developer / Package Registries

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Crates.io](https://crates.io/) | API | None | general, it, reference, packages |
| [Docker Hub](https://hub.docker.com/) | API | None | general, it, reference, packages |
| [GitHub](https://github.com/) | API | `GITHUB_TOKEN` | general, reference, github:code, github:issues, github:prs |
| [npm](https://www.npmjs.com/) | API | None | general, it, reference, packages |
| [PyPI](https://pypi.org/) | API | None | general, it, reference, packages |
| [Repology](https://repology.org/) | API | None | general, it, reference, packages |
| [RubyGems](https://rubygems.org/) | API | None | general, it, reference, packages |
| [Stack Exchange](https://stackexchange.com/) | API | Optional | general, reference, science, stackexchange:code, stackexchange:serverfault |

### Science & Research

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [arXiv](https://arxiv.org/) | API | None | general, science, reference |
| [HuggingFace](https://huggingface.co/) | API | `HF_TOKEN` (optional) | general, science, huggingface:datasets, huggingface:papers |
| [OpenAlex](https://openalex.org/) | API | None | general, science, reference |
| [Open Library](https://openlibrary.org/) | API | None | general, books, reference |
| [Semantic Scholar](https://www.semanticscholar.org/) | API | Optional | general, science, reference |
| [UniProt](https://www.uniprot.org/) | API | None | general, science, reference, biology, medical |
| [Internet Archive](https://archive.org/) | API | None | reference, web:archive, historical |

### Medical / Health

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [ClinicalTrials.gov](https://clinicaltrials.gov/) | API | None | general, medical, health, science |
| [openFDA](https://open.fda.gov/) | API | None | general, medical, health, science, government |
| [PubChem](https://pubchem.ncbi.nlm.nih.gov/) | API | None | general, science, reference, chemistry, medical |
| [PubMed](https://pubmed.ncbi.nlm.nih.gov/) | API | None | general, science, reference, medical, health |

### Security / Threat Intelligence

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [AbuseIPDB](https://www.abuseipdb.com/) | API | `ENGINE_ABUSEIPDB_API_KEY` | security, threat-intel |
| [AlienVault OTX](https://otx.alienvault.com/) | API | `ENGINE_OTX_API_KEY` | security, threat-intel |
| [Censys](https://censys.io/) | API | `ENGINE_CENSYS_API_KEY` + `_API_SECRET` | it, security |
| [CRT.sh](https://crt.sh/) | API | None | it, security |
| [CVE Program (MITRE)](https://cve.mitre.org/) | API | None | it, security |
| [DeHashed](https://dehashed.com/) | API | `ENGINE_DEHASHED_API_KEY` | security, threat-intel |
| [Exploit-DB](https://www.exploit-db.com/) | Scrape | None | security, exploit |
| [FIRST EPSS](https://www.first.org/epss/) | API | None | security, threat-intel |
| [GreyNoise](https://www.greynoise.io/) | API | `ENGINE_GREYNOISE_API_KEY` (optional) | security, threat-intel |
| [Have I Been Pwned](https://haveibeenpwned.com/) | API | `ENGINE_HIBP_API_KEY` | security, reference |
| [IntelX](https://intelx.io/) | API | `ENGINE_INTELX_API_KEY` | security, threat-intel |
| [MITRE ATT&CK](https://attack.mitre.org/) | API | None | security, reference |
| [NVD (NIST)](https://nvd.nist.gov/) | API | `ENGINE_NVD_API_KEY` (optional) | it, security |
| [Shodan](https://www.shodan.io/) | API | `ENGINE_SHODAN_API_KEY` | it, security |
| [URLhaus](https://urlhaus.abuse.ch/) | API | None | security, threat-intel |
| [VirusTotal](https://www.virustotal.com/) | API | `ENGINE_VIRUSTOTAL_API_KEY` | security, malware |
| [VulnCheck](https://vulncheck.com/) | API | `ENGINE_VULNCHECK_API_KEY` | security, threat-intel |

### Finance / Economics

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [FRED](https://fred.stlouisfed.org/) | API | `ENGINE_FRED_API_KEY` | general, finance, reference, economics |
| [SEC EDGAR](https://www.sec.gov/edgar/) | API | None | general, finance, reference |

### Media & Entertainment

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [MusicBrainz](https://musicbrainz.org/) | API | None | general, music, reference |
| [TMDB](https://www.themoviedb.org/) | API | `ENGINE_TMDB_API_KEY` | general, movies, entertainment |

### Geography / GIS

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Nominatim (OSM)](https://nominatim.openstreetmap.org/) | API | None | general, geography, reference |

### Legal

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Oyez (SCOTUS)](https://www.oyez.org/) | API | None | general, reference, legal |

**Adding a new engine:** See [`docs/ENGINE_ADAPTERS.md`](docs/ENGINE_ADAPTERS.md) for the full adapter reference — contract rules, data types, lifecycle hooks, and the category system.

## Quick Start

### VPN and proxy deployments

Google and DuckDuckGo are best-effort HTML-scrape adapters. VPN, proxy, and
datacenter IPs can receive consent, challenge, or block pages; configure a
Brave API key (`ENGINE_BRAVE_API_KEY`) for a reliable API-backed web-search
source. A Brave key supplements the other active Tier-1 engines; it does not
disable them.

To surface successful scrape responses that parse to zero results, enable the
opt-in diagnostic flag. These entries appear in `meta.empty_engines`; they are
warnings rather than failures because a search can legitimately have no matches.

```bash
FEATURE_EMPTY_SCRAPE_DIAGNOSTICS=true
```

Pre-built Docker images are available from GitHub Container Registry. Builds run automatically on every push to `main` (`latest`, `unstable`) and on version tags (`stable`, `X`, `X.Y`, `X.Y.Z`).

```bash
# Pull and run with Valkey for caching and rate limiting
docker run -d --name valkey valkey/valkey:8-alpine
docker run -d --name slopsearx -p 8080:8080 \
  -e VALKEY_URL=redis://valkey:6379/0 \
  --link valkey \
  ghcr.io/magnus919/slopsearx:latest

# Try it
curl 'http://localhost:8080/search?q=hello+world&format=json'
```

## License

MIT — see [LICENSE](LICENSE).
