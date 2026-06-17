# By the numbers

## Codebase size

| Metric | Count |
|---|---|
| Core library modules (`slopsearx/`) | 15 |
| Engine adapters (`engines/`) | 48 |
| Test files (`tests/`) | 37 |
| Lines of Python | ~19,000 |
| Lines of Markdown documentation | ~3,600 |
| Lines of YAML config | ~230 |

## Engine breakdown

| Domain | Engines |
|---|---|
| General / Web | 6 (Brave, DuckDuckGo, Google, Hacker News, Reddit, Wikipedia) |
| Developer / Packages | 8 (Crates.io, Docker Hub, GitHub, npm, PyPI, Repology, RubyGems, Stack Exchange) |
| Science & Research | 7 (arXiv, HuggingFace, Internet Archive, OpenAlex, Open Library, Semantic Scholar, UniProt) |
| Medical / Health | 4 (ClinicalTrials.gov, openFDA, PubChem, PubMed) |
| Security / Threat Intel | 17 (AbuseIPDB, AlienVault OTX, Censys, CRT.sh, CVE, DeHashed, EPSS, Exploit-DB, GreyNoise, HIBP, IntelX, MITRE ATT&CK, NVD, Shodan, URLhaus, VirusTotal, VulnCheck) |
| Finance / Economics | 2 (FRED, SEC EDGAR) |
| Media & Entertainment | 2 (MusicBrainz, TMDB) |
| Geography / GIS | 1 (Nominatim) |
| Legal | 1 (Oyez) |
| **Total** | **48** |

## Engine types

| Type | Count | Examples |
|---|---|---|
| API | 45 | Brave, Wikipedia, GitHub, arXiv, Shodan |
| Scrape | 3 | DuckDuckGo, Google, Exploit-DB |

## API endpoints

| Endpoint | Purpose |
|---|---|
| `GET /search` | Execute search (JSON or YAML+Markdown) |
| `GET /health` | Server liveness and Valkey connectivity |
| `GET /metrics` | OpenMetrics for Prometheus scraping |
| `GET /config` | Categories-to-engines mapping |

## Observability

| Metric | Type | Labels |
|---|---|---|
| `slopsearx_engine_queries_total` | Counter | `engine` |
| `slopsearx_engine_latency_seconds` | Histogram | `engine`, `quantile` (0.5, 0.9, 0.99) |
| `slopsearx_engine_status` | Gauge | `engine` (0=ok, 1=degraded, 2=down) |
| `slopsearx_cache_hit_total` | Counter | `type` (hit/miss) |
| `slopsearx_server_requests_total` | Counter | (no labels) |
| `slopsearx_server_requests_by_category_total` | Counter | `category` |
| `slopsearx_server_requests_by_format_total` | Counter | `format` |
| `slopsearx_server_errors_total` | Counter | `type` (timeout, circuit_open, rate_limited, internal) |

## Alertmanager rules

| Alert | Severity | Condition |
|---|---|---|
| SlopSearxDown | critical | `/health` unreachable for 1m |
| EngineDegraded | warning | Engine status > 0 for 5m |
| HighErrorRatio | warning | Query growth > 25% in 5m |
| HighLatency | warning | P95 latency > 5s for 5m |
| RateLimitSaturation | info | Request rate > 100/s for 5m |
| ServerErrorSpike | warning | Error rate > 0.1/s for 5m |

## CI/CD

| Workflow | Trigger | Jobs |
|---|---|---|
| `ci.yml` | Push / PR | Lint (ruff), Test (pytest on 3.12 + 3.13) |
| `docker.yml` | Push to main / tag | Build Docker image, push to ghcr.io |
| `droid.yml` | Push to main | Factory Droid tagging |
| `droid-review.yml` | PR open / sync | Factory Droid code review |
| `droid-wiki-refresh.yml` | Push to main | Wiki refresh via Factory Droid |
| `release-please.yml` | Push to main | Automated release PR management |

## Dev tooling

| Tool | Purpose |
|---|---|
| ruff | Linting and formatting |
| mypy | Static type checking |
| pytest + pytest-asyncio + pytest-cov | Testing with coverage |
| pre-commit | Git hooks for ruff formatting/linting |
| deptry | Dependency analysis |
| radon | Cyclomatic complexity |
| vulture | Dead code detection |
| import-linter | Architecture layer enforcement |
| jscpd | Copy-paste detection (config in `.jscpd.json`) |

## Valkey keyspace

| Key pattern | Purpose | TTL |
|---|---|---|
| `search:{sha256}` | Precise search cache | 3600s (general) / 300s (news) |
| `answer:{sha256}` | Broad answer cache | 3600s |
| `suggest:{sha256}` | Suggestion cache | 1800s |
| `ratelimit:{engine}:{window}` | Per-engine rate limit counter | 2× window |
| `ratelimit:client:{ip}:{window}` | Per-client rate limit counter | 2× window |
| `engine_stats:{engine}:{date}` | Daily quality telemetry | 90 days |
| `query_audit:{date}` | Daily audit stream | 90 days, max 10K entries |

## Deployment

| Metric | Value |
|---|---|
| Docker image size | ~200MB |
| Cold start time | <2s |
| Default replicas | 3 (K8s) |
| Max replicas | 100 (HPA) |
| Port | 8080 |
