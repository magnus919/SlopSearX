# Query router

Active contributors: Magnus Hedemark

## Purpose

Examines search queries and routes them to the most relevant engine subset. This improves latency, reduces API quota waste, and produces less noisy results for agent consumers. First-match-wins keyword matching — no ML, no LLM calls, no remote API.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `QueryRouter` | `slopsearx/router.py` | Lightweight query classifier with configurable topic signatures |
| `_DEFAULT_TOPICS` | `slopsearx/router.py` | Built-in topic-to-engines mappings for code, science, news, social, reference, historical |

## How it works

### Topic structure

Each topic has:
- `name` — human-readable label (e.g., `"code"`, `"science"`)
- `keywords` — list of trigger words checked against the query
- `engines` — list of engine names to dispatch to when matched

### Matching algorithm

1. Query is lowercased and trimmed
2. Topics are checked in order (first-match-wins)
3. For each topic, keywords are checked against the query
4. First topic with a matching keyword determines the engine set
5. Queries matching no topic return `None` — server falls back to Tier 1 engines

### Default topics

```python
_DEFAULT_TOPICS = [
    {"name": "code", "keywords": ["python", "javascript", "typescript", "rust", ...],
     "engines": ["brave", "github", "stackexchange", "duckduckgo", "wikipedia"]},
    {"name": "science", "keywords": ["quantum", "physics", "biology", ...],
     "engines": ["brave", "arxiv", "semanticscholar", "openalex", "duckduckgo", "wikipedia"]},
    {"name": "news", "keywords": ["news", "today", "breaking", "latest", ...],
     "engines": ["brave", "hackernews", "duckduckgo"]},
    {"name": "social", "keywords": ["reddit", "hacker news", "show hn", ...],
     "engines": ["brave", "hackernews", "reddit", "duckduckgo"]},
    {"name": "reference", "keywords": ["documentation", "docs", "tutorial", ...],
     "engines": ["brave", "wikipedia", "stackexchange", "duckduckgo"]},
    {"name": "historical", "keywords": ["archive", "wayback", "historical", ...],
     "engines": ["brave", "wikipedia", "internetarchive", "duckduckgo"]},
]
```

### Skipping routing

Routing is skipped when:
- `routing.enabled = false` in config
- `categories` param is explicitly provided
- `engines` param is explicitly provided

## Integration points

- **Server search handler:** `_router.route(q)` called when no `categories` or `engines` param is provided
- **Config:** Topics and fallback engines overrideable via `config.yaml` routing section

## Entry points

- Add a topic: add entry to `_DEFAULT_TOPICS` or override via `config.yaml`
- Change matching: modify `route()` method
- Tune keyword sensitivity: adjust keyword lists per topic

## Key source files

| File | Description |
|---|---|
| `slopsearx/router.py` | QueryRouter class and default topics |
| `slopsearx/server.py` | Caller: `_router.route(q)` in the search handler |
