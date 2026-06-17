# Lore

SlopSearX was created on June 8-15 2026 by Magnus Hedemark. The project reached 76 commits across eight days.

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

**19 profession-specific engines:** Developer registries (PyPI, npm, crates.io, RubyGems, Repology, Docker Hub), media (MusicBrainz, Open Library, TMDB), medical (PubMed, PubChem, ClinicalTrials.gov), legal (Oyez), finance (SEC EDGAR, FRED), geography (Nominatim), government (openFDA), biology (UniProt).

**Four new core subsystems:** QueryRouter, ProxyPool, SuggestionService, EngineStatsTracker.

### Hardening (Jun 10-14 2026)

After the rapid expansion to 48 engines, development shifted to reliability and operational hardening. Key additions:

**Reliability features:**

- **Circuit breaker** on every engine adapter (`slopsearx/adapter.py`). After 5 consecutive errors, the circuit opens for 5 minutes, preventing wasted dispatch to failing engines. Half-open probes allow automatic recovery.
- **Fail-closed rate limiting** (`slopsearx/ratelimit.py`). When Valkey is unreachable and `FAIL_CLOSED=true`, rate-limit checks deny requests during a configurable grace period, then fall back to an in-process token bucket. Default is fail-open for availability.
- **Per-client rate limiting.** Client IP-based rate limits using the same Valkey sliding window strategy, preventing noisy tenants from starving others. Configurable via `PER_CLIENT_REQUESTS` and `PER_CLIENT_WINDOW_SECONDS`.
- **Engine dispatch semaphore.** Capped concurrent outbound HTTP connections per search request (`MAX_CONCURRENT_ENGINES`, default 10), preventing resource exhaustion under load.

**Observability:**

- **Query audit trail** (`slopsearx/audit.py`). Every search query recorded in a daily Valkey stream with dispatch statistics, client IP, and latency. Capped at ~10K entries per day, 90-day retention.
- **Two-level caching** (`slopsearx/cache.py`). Added answer-level cache (broad key, query only) alongside the existing precise search cache (query + language + safesearch).
- **Negative caching.** Failed queries get a short-lived (60s) cache entry that returns HTTP 503 on subsequent requests, preventing thundering-herd retries.

**Developer experience:**

- **SSX CLI** (`ssx`). Agent-friendly CLI wrapping all API endpoints with YAML+Markdown default output and JSON mode for programmatic use.
- **URL sanitization** (`slopsearx/adapter.py`). Strips sensitive query parameters from error messages to prevent credential leakage in logs.
- **SKILL.md.** Comprehensive usage documentation covering all 48 engines, their categories, API configuration, and the CLI.
- **48 engine reference files.** Individual reference documents for every engine in `references/`, covering authentication, endpoints, rate limits, and data formats.
- **Expanded test suite.** Concurrency tests (686 lines), fail-closed tests (686 lines), cache integration tests, and per-adapter tests for Google, Stack Exchange, TMDB, OpenAlex, Shodan, FRED, and Internet Archive.

## Longest-standing features

All core features (adapter interface, merger, config, server, formatter, caching, rate limiting, metrics) date from the initial commit on June 8.

## Deprecated features

None. The project has not yet had any feature become obsolete.

## Major rewrites

None. The architecture has remained stable since the initial commit, with new subsystems and adapters added via the plugin interface.

## Growth trajectory

The project grew from 0 to 19,073 lines of Python in eight days (Jun 8-15). Started with 7 engines and a complete core in the initial commit. Expanded to 12 engines on day 2, 48 engines with 4 new core subsystems on day 3, and then spent days 4-8 on reliability hardening, observability, and developer tooling.
