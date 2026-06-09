# SlopSearX

**Cloud-native, stateless, AI-agent-first meta search engine.** Drop-in SearXNG replacement for the GroktoCrawl stack.

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

SlopSearX is a horizontally scalable, stateless meta search engine designed for AI agent consumption. It replaces SearXNG in the GroktoCrawl stack with:

- **JSON output by default** — structured responses designed for programmatic consumption
- **YAML+Markdown native output** — structured + readable for AI agent contexts via `format=yaml`
- **SearXNG-compatible API** — drop-in replacement for existing consumers
- **Plugin engine adapters** — one file per engine, `@register_engine`, zero orchestrator changes
- **Distributed rate limiting** — Valkey-backed sliding windows, correct at 50+ replicas
- **Stateless, cloud-native** — no local volumes, no persistent DB, all replicas interchangeable
- **Hybrid config** — env vars for secrets, optional mounted file for engine tuning

## Status

Pre-alpha. Spec complete. Active development.

## Engines

| Engine | Type | Status |
|---|---|---|
| [Brave Search API](https://brave.com/search/api/) | API | Planned |
| [DuckDuckGo](https://duckduckgo.com/) | Scrape (HTTP + HTML) | Planned |
| [Google](https://google.com/) | Scrape (HTTP + HTML) | Planned |
| [Wikipedia](https://www.wikipedia.org/) | Structured API | Planned |

## Quick Start

```bash
# Coming soon
docker run -p 8080:8080 slopsearx/slopsearx:latest
curl 'http://localhost:8080/search?q=hello+world&format=json'
```

## License

MIT — see [LICENSE](LICENSE).
