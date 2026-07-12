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
```

A mounted YAML config file at `/etc/slopsearx/config.yaml` provides per-engine tuning.

## Troubleshooting Engine Startup

### API Key Not Loading at Startup

If an API-key-protected engine (e.g. Brave) fails to respond at startup, the likely cause is that the API key environment variable is only loaded in `search()` but not in `__init__()`. The startup lifecycle is:

1. Engine constructor (`__init__`) is called with the config dict
2. Startup warmup runs `engine.warmup()` (no-op by default for most engines)
3. The base class `health()` checks `self.config.get("api_key")` — if missing, returns `ERROR`
4. If the circuit breaker trips (5 consecutive errors, 300s timeout), all subsequent searches for that engine are blocked

**Fix:** Load the env var fallback in `__init__()` itself, not only in `search()`:

```python
def __init__(self, config: dict | None = None, **kwargs):
    cfg = config or {}
    if not cfg.get("api_key") and self.env_prefix:
        env_key = os.environ.get(f"{self.env_prefix}_API_KEY", "")
        if env_key:
            cfg["api_key"] = env_key
    super().__init__(cfg, **kwargs)
```

The `search()` method should also retain its own fallback as a belt-and-suspenders measure for code paths that create the adapter without calling `__init__` (e.g., test fixtures, direct instantiation).

### Circuit Breaker Basics

- **Threshold:** 5 consecutive errors (configurable via `ENGINE_CIRCUIT_THRESHOLD` env var)
- **Timeout:** 300 seconds circuit-open duration (configurable via `ENGINE_CIRCUIT_TIMEOUT`)
- **Reset:** A single successful response via `record_success()` resets the error counter and closes the circuit
- **Exposure:** Circuit breaker state is exposed via `/metrics` (Prometheus), **not** via `/health`

## Healthcheck Architecture

SlopSearX has two layers of health monitoring, both of which **do NOT probe external search APIs**:

- **Docker HEALTHCHECK** (Dockerfile, line 30): Runs `python /app/healthcheck.py` every **30s** (timeout 5s, start-period 5s, 3 retries). The script simply does `GET http://127.0.0.1:8080/health` and checks for status 200.
- **`/health` endpoint** (server.py:237-270): Returns server liveness + Valkey connectivity only. Lists every engine as `"ok"` **without** calling the engine's `health()` method. Status degrades to `"degraded"` when Valkey is unreachable and fail-closed mode is active.
- **Kubernetes probes** (k8s/deployment.yaml): `livenessProbe` at `/health` every 30s (initial delay 5s), `readinessProbe` at `/health` every 10s (initial delay 3s). Same `/health` endpoint — no external API calls.
- **Docker Compose** (docker-compose.yml): **No healthcheck block** — relies on the Dockerfile `HEALTHCHECK` instruction.

Actual engine health is tracked **passively** via the circuit breaker during real search requests and exposed via `/metrics` for Prometheus scraping.

### Warmup

Engine warmup occurs **once at startup** (not periodic). The `_startup()` function calls `engine.warmup()` for each active engine via `asyncio.gather()`. The base class `warmup()` is a no-op — individual engines override it if they need pre-warming (e.g., connection pooling, auth token refresh). Warmup failures are non-fatal (caught and logged, never cascade to block startup).

### Suggestion Service (Opt-In)

The `SuggestionService` in `suggest.py` can call the Brave Suggest API (`/res/v1/web/suggest`), but is **disabled by default** (`enable_suggestions: false` in config.yaml). When enabled, it caches results in Valkey for 30 minutes and falls back to DuckDuckGo's suggest API if Brave is unavailable.

## References

### General / Web

| Engine | File | Type | Auth |
|---|---|---|---|
| Brave Search | engines/brave.py | API | `ENGINE_BRAVE_API_KEY` |
| DuckDuckGo | engines/duckduckgo.py | Scrape | None |
| Google | engines/google.py | Scrape | None |
| Hacker News | engines/hackernews.py | API | None |
| Reddit | engines/reddit.py | API | None |
| Wikipedia | engines/wikipedia.py | API | None |

### Developer / Package Registries

| Engine | File | Type | Auth |
|---|---|---|---|
| Crates.io | engines/crates.py | API | None |
| Docker Hub | engines/dockerhub.py | API | None |
| GitHub | engines/github.py | API | `GITHUB_TOKEN` |
| npm | engines/npm.py | API | None |
| PyPI | engines/pypi.py | API | None |
| Repology | engines/repology.py | API | None |
| RubyGems | engines/rubygems.py | API | None |
| Stack Exchange | engines/stackexchange.py | API | Optional app key |

### Science & Research

| Engine | File | Type | Auth |
|---|---|---|---|
| arXiv | engines/arxiv.py | API | None |
| HuggingFace | engines/huggingface.py | API | `HF_TOKEN` (optional) |
| Internet Archive | engines/internetarchive.py | API | None |
| OpenAlex | engines/openalex.py | API | None |
| Open Library | engines/openlibrary.py | API | None |
| Semantic Scholar | engines/semanticscholar.py | API | Optional API key |
| UniProt | engines/uniprot.py | API | None |

### Medical / Health

| Engine | File | Type | Auth |
|---|---|---|---|
| ClinicalTrials.gov | engines/clinicaltrials.py | API | None |
| openFDA | engines/openfda.py | API | None |
| PubChem | engines/pubchem.py | API | None |
| PubMed | engines/pubmed.py | API | None |

### Security / Threat Intelligence

| Engine | File | Type | Auth |
|---|---|---|---|
| AbuseIPDB | engines/abuseipdb.py | API | `ENGINE_ABUSEIPDB_API_KEY` |
| AlienVault OTX | engines/otx.py | API | `ENGINE_OTX_API_KEY` |
| Censys | engines/censys.py | API | `ENGINE_CENSYS_API_KEY` + `_API_SECRET` |
| CRT.sh | engines/crtsh.py | API | None |
| CVE Program (MITRE) | engines/cve.py | API | None |
| DeHashed | engines/dehashed.py | API | `ENGINE_DEHASHED_API_KEY` |
| Exploit-DB | engines/exploitdb.py | Scrape | None |
| FIRST EPSS | engines/epss.py | API | None |
| GreyNoise | engines/greynoise.py | API | `ENGINE_GREYNOISE_API_KEY` (optional) |
| Have I Been Pwned | engines/hibp.py | API | `ENGINE_HIBP_API_KEY` |
| IntelX | engines/intelx.py | API | `ENGINE_INTELX_API_KEY` |
| MITRE ATT&CK | engines/mitreattack.py | API | None |
| NVD | engines/nvd.py | API | `ENGINE_NVD_API_KEY` (optional) |
| Shodan | engines/shodan.py | API | `ENGINE_SHODAN_API_KEY` |
| URLhaus | engines/urlhaus.py | API | None |
| VirusTotal | engines/virustotal.py | API | `ENGINE_VIRUSTOTAL_API_KEY` |
| VulnCheck | engines/vulncheck.py | API | `ENGINE_VULNCHECK_API_KEY` |

### Finance / Economics

| Engine | File | Type | Auth |
|---|---|---|---|
| FRED | engines/fred.py | API | `ENGINE_FRED_API_KEY` |
| SEC EDGAR | engines/edgar.py | API | None |

### Media & Entertainment

| Engine | File | Type | Auth |
|---|---|---|---|
| MusicBrainz | engines/musicbrainz.py | API | None |
| TMDB | engines/tmdb.py | API | `ENGINE_TMDB_API_KEY` |

### Geography / GIS

| Engine | File | Type | Auth |
|---|---|---|---|
| Nominatim | engines/nominatim.py | API | None |

### Legal

| Engine | File | Type | Auth |
|---|---|---|---|
| Oyez | engines/oyez.py | API | None |
