# SlopSearX overview

SlopSearX is a cloud-native, stateless, AI-agent-first meta search engine. It replaces SearXNG in the GroktoCrawl stack with a horizontally scalable design built for programmatic consumption.

**Key capabilities:**

- **48 engine adapters** — Brave, Wikipedia, DuckDuckGo, Google, GitHub, arXiv, Hacker News, HuggingFace, Semantic Scholar, Stack Exchange, OpenAlex, Internet Archive, Reddit, PyPI, npm, crates.io, RubyGems, Repology, Docker Hub, MusicBrainz, Open Library, TMDB, PubMed, PubChem, ClinicalTrials.gov, Oyez, SEC EDGAR, FRED, Nominatim, openFDA, UniProt, and 17 security/threat-intel engines (Shodan, Censys, VirusTotal, HIBP, AlienVault OTX, AbuseIPDB, VulnCheck, IntelX, DeHashed, CRT.sh, URLhaus, FIRST EPSS, GreyNoise, Exploit-DB, MITRE ATT&CK, CVE Program, NVD)
- **SearXNG-compatible API** — drop-in replacement for existing SearXNG consumers with all 23 response fields preserved
- **Agent-native output** — JSON by default, YAML+Markdown for AI agent contexts
- **Plugin architecture** — one file per engine, `@register_engine` decorator, zero orchestrator changes
- **Intelligent query routing** — topic-based engine dispatch that sends queries only to relevant engines (QueryRouter subsystem)
- **Distributed rate limiting** — Valkey-backed sliding windows correct at 50+ replicas
- **Response caching** — Valkey-backed, category-aware TTL (60s for news, 300s for general)
- **Proxy rotation infrastructure** — configurable proxy pool for scrape-based adapters to evade IP bans
- **Search query suggestions** — background service that fetches suggestions from engine suggest APIs
- **Per-engine quality telemetry** — Valkey-stored success/failure statistics for adaptive engine selection
- **Graceful degradation** — scrape-engine failures never block the response
- **OpenMetrics observability** — per-engine counters, latency histograms, and status gauges

Engines span **9 domains**: general/web, developer/packages, science/research, medical/health, security/threat-intel, finance/economics, media/entertainment, geography/GIS, and legal.

SlopSearX is written in Python 3.12+ using FastAPI and httpx. It runs as a single container image (~200MB, cold start under 2s) behind a load balancer, with Valkey as the only shared state.

## Quick links

- [Architecture](architecture.md) — system design and request flow
- [Getting started](getting-started.md) — install, build, test, run
- [Glossary](glossary.md) — project-specific terms
- [Systems](../systems/index.md) — internal subsystems deep-dive
- [API reference](../api/index.md) — REST endpoints
- [How to contribute](../how-to-contribute/index.md) — contribution workflow
