# Query router

Active contributors: Magnus Hedemark

## Purpose

Lightweight query classifier that routes search queries to the most relevant engine subset based on topic keyword matching. Reduces latency, conserves API quota, and produces less noisy results for agent consumers by dispatching only to engines likely to return relevant results.

No ML, no LLM calls, no remote API — purely keyword-based classification with a first-match-wins strategy.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `QueryRouter` | `slopsearx/router.py` | Lightweight query classifier. Initialized from config or defaults, exposes `route()` method. |
| `_DEFAULT_TOPICS` | `slopsearx/router.py` | Six default topic signatures: `code`, `science`, `news`, `social`, `reference`, `historical`. Each has a list of trigger keywords and a list of target engine names. |
| `_DEFAULT_FALLBACK` | `slopsearx/router.py` | Default fallback engine set (`["brave", "wikipedia"]`) used when no topic matches. |
| `RoutingConfig` | `slopsearx/config.py` | Configuration dataclass with `enabled` (bool), `topics` (optional dict), and `fallback` (optional list of engine names). |
| `route()` | `slopsearx/router.py` | Core method — takes query string and optional category list, returns engine name list or `None`. |

## How it works

### Classification strategy

The router uses a first-match-wins keyword matching strategy. It does not use ML, LLMs, or any remote API:

1. **Check enabled.** If routing is disabled (`routing.enabled: false`), `route()` immediately returns `None`.
2. **Check categories.** If the caller already provided an explicit category filter (non-empty `categories` list), routing is skipped — category-based engine selection takes precedence.
3. **Normalize query.** The query is lowercased and stripped.
4. **Scan topics.** Topics are scanned in order (`code` → `science` → `news` → `social` → `reference` → `historical`). For each topic, its keyword list is scanned. The first keyword that appears as a substring of the normalized query selects that topic's engine set as the return value.
5. **Fallback.** If no keyword matches any topic, `route()` returns a caller-defined fallback engine set (defaults to `["brave", "wikipedia"]` via the `fallback` parameter).

Example: a query containing "python" matches the `code` topic's keyword list, returning `["brave", "github", "stackexchange", "duckduckgo", "wikipedia"]`. A query containing "news" matches the `news` topic, returning `["brave", "hackernews", "duckduckgo"]`.

### Default topic signatures

| Topic | Example keywords | Target engines |
|---|---|---|
| `code` | python, javascript, rust, typescript, golang, react, api, code, github, docker, kubernetes, sql, npm, pip, cargo | brave, github, stackexchange, duckduckgo, wikipedia |
| `science` | quantum, physics, biology, chemistry, neural network, machine learning, deep learning, paper, doi, theorem, algorithm, mathematics, statistics | brave, arxiv, semanticscholar, openalex, duckduckgo, wikipedia |
| `news` | news, today, breaking, latest, update, announced, released, headline | brave, hackernews, duckduckgo |
| `social` | reddit, hacker news, show hn, ask hn, discussion, forum | brave, hackernews, reddit, duckduckgo |
| `reference` | documentation, docs, tutorial, guide, how to, reference, wiki, manual, definition | brave, wikipedia, stackexchange, duckduckgo |
| `historical` | archive, wayback, historical, history, old, vintage, retro | brave, wikipedia, internetarchive, duckduckgo |

### Configuration format

All routing parameters are configurable via the `routing` section in `config.yaml`:

```yaml
routing:
  enabled: true
  topics:
    code:
      keywords: [python, javascript, rust, api, code, github]
      engines: [brave, github, stackexchange, wikipedia]
    science:
      keywords: [quantum, physics, biology, chemistry]
      engines: [brave, arxiv, semanticscholar, openalex, wikipedia]
  fallback:
    - brave
    - wikipedia
```

Operators can override topic definitions, add new topics, or change the fallback set. Topics defined in config replace defaults entirely.

## Integration points

- **Server startup:** `startup()` in `server.py` creates a `QueryRouter` from `cfg.routing` (a `RoutingConfig` dataclass converted to dict via `dataclasses.asdict()`)
- **Search handler:** The `search()` endpoint in `server.py` calls `_router.route(q)` when there is no category filter. If the router returns a non-None engine list, those engines replace the default engine set for the request
- **Config system:** `RoutingConfig` is a dataclass in `config.py` with `enabled`, `topics`, and `fallback` fields. Loaded from YAML config, overridable via env vars
- **Global state:** The `_router` module-level variable in `server.py` holds the singleton instance

## Entry points for modification

- Adding or removing topic signatures: edit `_DEFAULT_TOPICS` in `router.py`, or override via `routing.topics` in `config.yaml`
- Changing the fallback engine set: edit `_DEFAULT_FALLBACK` in `router.py`, or set via `routing.fallback` in `config.yaml`
- Changing routing logic: modify `route()` method — keyword match strategy, case normalization, or category interaction
- Adding new config fields: update `RoutingConfig` dataclass in `config.py` and `_apply_config()` in `router.py`

## Key source files

| File | Description |
|---|---|
| `slopsearx/router.py` | QueryRouter class, _DEFAULT_TOPICS, _DEFAULT_FALLBACK, all routing logic |
| `slopsearx/config.py` | RoutingConfig dataclass, YAML + env var loading for routing section |
| `slopsearx/server.py` | Router initialization at startup, invocation in search handler |
