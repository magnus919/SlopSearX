# Patterns and conventions

## Architecture rules

These rules are enforced by convention and reviewed in PRs:

1. **The adapter interface is the primary invariant.** Every engine is one file, registered via `@register_engine`. Adding an engine requires zero changes to the orchestrator.
2. **Adapters never raise exceptions.** All errors are classified and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
3. **Internal schema is decoupled from wire format.** The `SearchResult` dataclass is the internal model. SearXNG JSON is one output formatter among many.
4. **Valkey is the only shared state.** No local volumes, no persistent DB, no per-replica state beyond what Valkey provides.
5. **Scrape engines use HTTP + HTML parsing.** No headless browsers. DDG and Google adapters use httpx + lxml for HTML parsing.

## Coding style

- Python 3.12+ with `from __future__ import annotations` in all files
- Line length: 120 characters
- Quote style: double quotes (`"`)
- 100% type annotated (no `Any` except for config passthrough)
- Ruff lint rules: `E`, `F`, `I`, `N`, `W`

Conventions are enforced by ruff and configured in `pyproject.toml`.

## Error handling pattern

All adapters follow the same error classification pattern:

```python
async def search(self, query: str, params=None) -> AdapterResponse:
    start_time = time.monotonic()
    try:
        async with httpx.AsyncClient(...) as client:
            resp = await client.get(...)
            latency = (time.monotonic() - start_time) * 1000

            if resp.status_code == 429:
                return AdapterResponse(results=[], status=EngineStatus.RATE_LIMITED, latency_ms=latency)
            if resp.status_code in (403, 503):
                return AdapterResponse(results=[], status=EngineStatus.BLOCKED, latency_ms=latency)
            resp.raise_for_status()
            # ... parse results
            return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=latency)

    except httpx.TimeoutException:
        return AdapterResponse(results=[], status=EngineStatus.TIMEOUT, latency_ms=latency)
    except Exception as exc:
        return AdapterResponse(results=[], status=EngineStatus.ERROR, error_message=str(exc), latency_ms=latency)
```

Key details:
- Always measure latency from request start, not from try/except entry
- Classify HTTP 429 as `RATE_LIMITED`, 403/503 as `BLOCKED`
- Wrap `httpx.TimeoutException` separately from generic `Exception`
- `latency_ms` is set on every return path (including error paths)
- Rate-limiting calls go through `self.rate_limiter.acquire(engine_name)` before each request

## Category system

Each engine declares its categories as a class attribute:

```python
class MyEngine(EngineAdapter):
    categories = ["general", "science", "github:code"]
```

Categories use SearXNG taxonomy. Sub-categories use namespace prefixes (`github:code`, `huggingface:datasets`). Operators can override categories via config or env vars without modifying code.

## Testing patterns

- Use `asyncio_mode = "auto"` - no manual async fixture setup needed
- Mock adapters for server tests via `@register_engine` + `_MockEngine` class
- Config tests use `monkeypatch.setenv()` for env var testing
- Metric tests verify the rendered OpenMetrics string format

## Import conventions

- `slopsearx.adapter` is the primary shared module
- Engine files import from `slopsearx.adapter` only
- Core modules (`merger.py`, `formatter.py`, etc.) import from `slopsearx.adapter`
- Application code uses relative imports within the package
