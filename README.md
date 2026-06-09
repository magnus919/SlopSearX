# SlopSearX

**Cloud-native, stateless, AI-agent-first meta search engine.** Drop-in SearXNG replacement for the GroktoCrawl stack.

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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

## Engines

| Engine | Type | Auth | Categories |
|---|---|---|---|
| [arXiv](https://arxiv.org/) | API | None | general, science, reference |
| [Brave Search](https://brave.com/search/api/) | API | API key | general, news, science, images |
| [DuckDuckGo](https://duckduckgo.com/) | Scrape | None | general, news |
| [GitHub](https://github.com/) | API | Token | general, reference, github:code/issues/prs |
| [Google](https://google.com/) | Scrape | None | general, news |
| [Hacker News](https://news.ycombinator.com/) | API | None | general, news |
| [HuggingFace](https://huggingface.co/) | API | Optional | general, science, hf:datasets/papers |
| [Internet Archive](https://archive.org/) | API | None | reference, web:archive, historical |
| [OpenAlex](https://openalex.org/) | API | None | general, science, reference |
| [Semantic Scholar](https://www.semanticscholar.org/) | API | Optional | general, science, reference |
| [Stack Exchange](https://stackexchange.com/) | API | Optional | general, reference, science, stackexchange:code/serverfault |
| [Wikipedia](https://www.wikipedia.org/) | API | None | general, science, reference |

**Adding a new engine:** See [`docs/ENGINE_ADAPTERS.md`](docs/ENGINE_ADAPTERS.md) for the full adapter reference — contract rules, data types, lifecycle hooks, and the category system.

## Quick Start

```bash
# Coming soon
docker run -p 8080:8080 slopsearx/slopsearx:latest
curl 'http://localhost:8080/search?q=hello+world&format=json'
```

## License

MIT — see [LICENSE](LICENSE).
