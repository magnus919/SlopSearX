# Patterns and conventions

Active contributors: Magnus Hedemark

## Code style

- **Python 3.12+** — use modern Python features (match/case, type hints, `str | None`)
- **Type hints** — all public APIs are fully typed (`mypy --strict`)
- **No exceptions** — adapters never raise; all errors classified in `AdapterResponse.status`
- **Async/await** — all I/O is async (httpx, Valkey)

## Architecture rules

### Adapter contract (primary invariant)

1. One file per engine, `@register_engine` decorator
2. Adapters never raise exceptions — errors classified as `EngineStatus`
3. No adapter cross-talk — no shared state, no calling other adapters
4. Rate limiting owned by the adapter class — `self.rate_limiter.acquire()`
5. Internal schema decoupled from wire format — `SearchResult` is canonical

### System boundaries

- **Valkey is the only shared state** — no local volumes, no persistent DB, no per-replica state
- **Graceful degradation** — failing sub-components never block the response
- **Stateless at application layer** — every replica is interchangeable
- **Plugin architecture** — adding engines requires zero orchestrator changes

## Feature flags

All new behavior-adjacent code is gated behind a feature flag (default `false`):

```python
if config.feature_flags.is_enabled("ai_dispatch"):
    # new behavior
```

Flags are snake_case, set via `config.yaml` → `features:` block or `FEATURE_<NAME>` env vars.

## Conventional commits

```
feat:    new feature
fix:     bug fix
docs:    documentation
chore:   maintenance (deps, config)
ci:      CI/CD changes
refactor: restructuring without behavior change
```

DCO sign-off required: `git commit -s`.

## Import layering

Enforced by `import-linter`:

```
slopsearx.server         (top)
slopsearx.router
slopsearx.merger
slopsearx.formatter
slopsearx.suggest
slopsearx.stats
slopsearx.audit
slopsearx.cache
slopsearx.config
slopsearx.ratelimit
slopsearx.proxypool
slopsearx.adapter         (bottom)
```

Exception: `slopsearx.adapter → slopsearx.proxypool` (allowed for ScrapeAdapter's ProxyPool integration).

## Naming conventions

- **Engine env vars:** `ENGINE_{NAME}_{SETTING}` (e.g., `ENGINE_BRAVE_API_KEY`)
- **Feature flags:** `FEATURE_{NAME}` env vars, `snake_case` in config
- **Metric names:** `slopsearx_{subsystem}_{metric}_unit` (e.g., `slopsearx_engine_queries_total`)
- **Valkey keys:** `{namespace}:{identifier}:{scope}` (e.g., `ratelimit:brave:12345`, `search:sha256hex`)
