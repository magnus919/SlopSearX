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

## Engines (51)

The table below is maintained to match the live adapter registry (51 registered
adapters as of this writing). The MCP server's `slopsearx_list_capabilities`
tool and the `slopsearx://capabilities` resource are generated from that same
registry at runtime — treat them as authoritative.

### General / Web

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Brave Search](https://brave.com/search/api/) | API | `ENGINE_BRAVE_API_KEY` | general, news, science, images |
| [DuckDuckGo](https://duckduckgo.com/) | Scrape | None | general, news, images |
| [Google](https://google.com/) | Scrape | None | general, news |
| [Hacker News](https://news.ycombinator.com/) | API | None | general, news |
| [Reddit](https://reddit.com/) | API | None | general, social, reddit:subreddit |
| [Wikipedia](https://www.wikipedia.org/) | API | None | general, science, reference |

### Developer / Package Registries

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Crates.io](https://crates.io/) | API | None | it, reference, packages |
| [Docker Hub](https://hub.docker.com/) | API | None | it, reference, packages |
| [GitHub](https://github.com/) | API | `GITHUB_TOKEN` | reference, github:code, github:issues, github:prs |
| [npm](https://www.npmjs.com/) | API | None | it, reference, packages |
| [PyPI](https://pypi.org/) | API | None | it, reference, packages |
| [Repology](https://repology.org/) | API | None | it, reference, packages |
| [RubyGems](https://rubygems.org/) | API | None | it, reference, packages |
| [Stack Exchange](https://stackexchange.com/) | API | Optional | general, reference, science, stackexchange:code, stackexchange:serverfault |

### Science & Research

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [arXiv](https://arxiv.org/) | API | None | science, reference |
| [HuggingFace](https://huggingface.co/) | API | `HF_TOKEN` (optional) | science, huggingface:datasets, huggingface:papers |
| [OpenAlex](https://openalex.org/) | API | None | science, reference |
| [Open Library](https://openlibrary.org/) | API | None | books, reference |
| [Semantic Scholar](https://www.semanticscholar.org/) | API | Optional | science, reference |
| [UniProt](https://www.uniprot.org/) | API | None | science, reference, biology, medical |
| [Internet Archive](https://archive.org/) | API | None | reference, web:archive, historical |

### Medical / Health

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [ClinicalTrials.gov](https://clinicaltrials.gov/) | API | None | medical, health, science |
| [openFDA](https://open.fda.gov/) | API | None | medical, health, science, government |
| [PubChem](https://pubchem.ncbi.nlm.nih.gov/) | API | None | science, reference, chemistry, medical |
| [PubMed](https://pubmed.ncbi.nlm.nih.gov/) | API | None | science, reference, medical, health |

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
| [FRED](https://fred.stlouisfed.org/) | API | `ENGINE_FRED_API_KEY` | finance, reference, economics |
| [SEC EDGAR](https://www.sec.gov/edgar/) | API | None | finance, reference |

### Media & Entertainment

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [MusicBrainz](https://musicbrainz.org/) | API | None | music, reference |
| [TMDB](https://www.themoviedb.org/) | API | `ENGINE_TMDB_API_KEY` | movies, entertainment |

### Geography / GIS

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Nominatim (OSM)](https://nominatim.openstreetmap.org/) | API | None | geography, reference |

### Legal

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Oyez (SCOTUS)](https://www.oyez.org/) | API | None | reference, legal |

### Jobs / ATS

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [Ashby](https://www.ashbyhq.com/) | API | None | jobs |
| [Greenhouse](https://www.greenhouse.io/) | API | None | jobs |
| [Lever](https://www.lever.co/) | API | None | jobs |

**Adding a new engine:** See [`docs/ENGINE_ADAPTERS.md`](docs/ENGINE_ADAPTERS.md) for the full adapter reference — contract rules, data types, lifecycle hooks, and the category system.

## MCP Server (agents)

SlopSearX ships a Model Context Protocol server for AI agents. It exposes
intent-level search (no URL strings), capability discovery, scope
explanation, snapshot-based pagination, and asynchronous research jobs —
built on the same pipeline as the HTTP API.

```bash
# Install (already included in the package dependencies)
pip install -e ".[dev]"

# Run over stdio (default transport)
slopsearx-mcp            # or: python -m slopsearx.mcp

# Run over HTTP for remote clients (e.g. Hermes Agent on another host)
MCP_TRANSPORT=http MCP_HOST=0.0.0.0 MCP_AUTH_TOKEN=change-me slopsearx-mcp

# Remote gateway: stdio MCP server that proxies to a remote SlopSearX server
slopsearx-mcp --remote http://<slopsearx-host>:8000/mcp   # token: MCP_REMOTE_TOKEN=…

# Gateway against an OAuth-mode remote: --oauth runs the standard MCP OAuth
# flow (loopback callback; tokens persisted for reuse)
slopsearx-mcp --remote http://<slopsearx-host>:8000/mcp --oauth

# OAuth 2.1 mode (required by OAuth-only clients, e.g. Claude Web connectors)
MCP_TRANSPORT=http MCP_OAUTH_ENABLED=1 MCP_OAUTH_ISSUER_URL=https://mcp.example.com slopsearx-mcp
```

- 15 tools: `slopsearx_search`, `slopsearx_search_targeted`,
  `slopsearx_search_jobs`, `slopsearx_search_security`,
  `slopsearx_search_science`, `slopsearx_list_capabilities`,
  `slopsearx_explain_search_scope`, `slopsearx_get_service_status`,
  `slopsearx_read_results`, `slopsearx_read_result`,
  `slopsearx_start_research`, `slopsearx_get_job`, `slopsearx_cancel_job`,
  `slopsearx_retry_research`, `slopsearx_extend_research`
- Resources: `slopsearx://capabilities`, `slopsearx://capabilities/{engine}`,
  `slopsearx://routing-profiles`, `slopsearx://health/summary`
- Specialist tools (jobs, security, science, research) are disabled until
  the operator grants them (`MCP_GRANT_JOBS=1`, `MCP_GRANT_SECURITY=1`,
  `MCP_GRANT_SCIENCE=1`, `MCP_GRANT_RESEARCH=1`).
- Sensitive engines (`hibp`, `dehashed`) are unreachable from generic
  routing, categories, and intent profiles, and are rejected by **every**
  explicit-engine search path (generic explicit engines, targeted, jobs,
  security, science) unless the operator sets `MCP_TARGETED_SENSITIVE_ALLOWED=1`
  (deliberate, uniform policy boundary enforced by one shared gate).
- There is no separate "advanced search" tool. Richer `include`/detail
  (card vs. full record) semantics on the existing search/read tools and the
  per-engine capability matrix (`slopsearx_list_capabilities`) cover
  `requires_*` needs — see `docs/MCP_SERVER.md` §6.14.

Full installation, configuration, client setup (Claude Desktop, Cursor,
**Hermes Agent** — see "Hermes Agent (Nous Research)" in the guide — and
generic MCP clients), and agent usage guidance:
**[docs/MCP_SERVER.md](docs/MCP_SERVER.md)**.

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
