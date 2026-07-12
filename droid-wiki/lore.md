# Lore

SlopSearX was created on June 8-15, 2026 by Magnus Hedemark from a multi-agent council debate that established the architectural foundation.

## Eras

### Genesis (Jun 8, 2026)

Initial commit established the complete project skeleton: adapter interface, merger, config, server, formatter, caching, rate limiting, and metrics modules. First engine adapters: Brave, Wikipedia, DuckDuckGo, Google, GitHub, Hacker News, HuggingFace, arXiv, Semantic Scholar. The architecture was debated by a multi-agent council — the single-replica-type topology, hybrid config model, presence-weighted ranking, and SearXNG-as-wire-format decisions were all captured in the `spec.md` council convergence summary.

### Expansion (Jun 9, 2026)

Sixteen more commits adding Stack Exchange, OpenAlex, Internet Archive, and Reddit adapters. Documentation explosion: README, ENGINE_ADAPTERS.md, AGENTS.md, CONTRIBUTING.md, spec.md, issue templates, PR template. CI pipeline established. Docker and Kubernetes manifests created. Grafana monitoring dashboard built.

### Professional expansion (Jun 9-10, 2026)

The project grew from ~12 engines to 48, adding 17 security/threat-intel engines and 19 profession-specific engines across developer registries, medicine, law, finance, geography, government, and biology. Four new core subsystems: QueryRouter, ProxyPool, SuggestionService, EngineStatsTracker.

### Hardening (Jun 10-17, 2026)

After rapid expansion, development shifted to reliability and operational maturity:

**Reliability:** Circuit breaker on every engine adapter. Fail-closed rate limiting with local fallback. Per-client rate limiting. Engine dispatch semaphore.

**Observability:** Query audit trail. Two-level caching (search + answer). Negative caching. Structlog-based JSON logging. Sentry error tracking integration. X-Request-ID middleware for distributed tracing.

**Operational:** Alertmanager rules (6 alerts). Grafana dashboard with per-engine panels. Runbooks for incident response. Feature flags (safe-by-default). Engine circuit breaker (5-error/300s timeout).

**Developer experience:** SSX CLI agent-friendly wrapper. Pre-commit hooks. Expanded dev tooling: deptry, radon, vulture, import-linter, jscpd. Release-please for automated releases. Dependabot configuration.

## Longest-standing features

All core features (adapter interface, merger, config, server, formatter, caching, rate limiting, metrics) date from the initial commit on June 8.

## Deprecated features

None. The project has not yet had any feature become obsolete.

## Major rewrites

None. The architecture has remained stable since the initial commit, with new subsystems and adapters added via the plugin interface.

## Council convergence

The multi-agent council debate (captured in spec.md Appendix) resolved seven architectural tensions:

| Tension | Converged Position |
|---|---|
| Single service vs two-process | Single replica type — all engines in one process |
| Env-var-only config vs hybrid | Hybrid: env vars for secrets, mounted config for engine tuning |
| Distributed rate limiting vs per-replica | Valkey-backed sliding window required from day one |
| SearXNG contract as internal schema vs wire-only | Internal Result dataclass, SearXNG JSON is one output formatter |
| Weighted fusion in V1 vs V2 | Presence-weighted in V1; extract an interface when a second strategy exists |
| Wikipedia pre-dispatch vs concurrent | All engines concurrent |
| Brave as primary vs equal-weight engines | Brave API backbone, scrape engines as optional multipliers |

## Growth trajectory

The project grew from 0 to ~19,000 lines of Python in nine days. Started with 7 engines and a complete core. Expanded to 48 engines with 4 new core subsystems, then spent a week on reliability hardening, observability, and developer tooling.
