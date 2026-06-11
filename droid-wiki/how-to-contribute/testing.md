# Testing

## Test framework

SlopSearX uses **pytest** with `asyncio_mode = "auto"` for async test support.

## Running tests

```bash
# Run all tests
pytest

# Verbose output
pytest -v

# Run specific test file
pytest tests/test_merger.py

# Run specific test class
pytest tests/test_merger.py::TestPresenceRanker

# Run with coverage
pytest --cov=slopsearx --cov=engines
```

## Test structure

Tests mirror the source structure. Each core module has a corresponding test file:

| Test file | What it covers |
|---|---|
| `tests/test_adapter.py` | `EngineAdapter` base class, `@register_engine`, `discover_engines()`, `SearchResult` |
| `tests/test_adapters.py` | Individual engine adapter behavior (12 engine files) |
| `tests/test_merger.py` | `PresenceRanker`, deduplication, metadata helpers |
| `tests/test_server.py` | FastAPI endpoints (`/search`, `/health`) with mock engine |
| `tests/test_formatter.py` | JSON and YAML+Markdown output formatters |
| `tests/test_config.py` | Three-layer config loading, env var overrides |
| `tests/test_cache.py` | Valkey cache key generation, TTL logic |
| `tests/test_ratelimit.py` | Rate limiter strategies (local token bucket, Valkey sliding window) |
| `tests/test_metrics.py` | OpenMetrics rendering |
| `tests/test_categories.py` | Category routing and filtering |
| `tests/test_tier1_adapters.py` | Tier 1 engine adapter behavior |

## CI pipeline

The CI runs on every push/PR to `main` (`.github/workflows/ci.yml`):

1. Install dependencies with `pip install -e ".[dev]"`
2. Lint with `ruff check .`
3. Test with `pytest -v` on Python 3.12 and 3.13

## Writing tests

### Adapter testing pattern

Mock engine adapters use `AdapterResponse` directly to simulate engine behavior without real HTTP calls:

```python
from slopsearx.adapter import AdapterResponse, EngineStatus, SearchResult

response = AdapterResponse(
    results=[SearchResult(url="https://example.com", title="Test", content="...", engine="mock")],
    status=EngineStatus.OK,
    latency_ms=42.0,
)
```

### Server testing pattern

Server tests use FastAPI's `TestClient` with a registered `_MockEngine` that returns controlled responses for different query strings (`"error"`, `"timeout_sim"`, `"blocked"`, `"rate_limited"`, or normal results).

### Config testing pattern

Config tests use `monkeypatch.setenv()` to set `ENGINE_*` env vars before calling `load_config()`:

```python
def test_env_var_api_key_flows_to_adapter(monkeypatch):
    monkeypatch.setenv("ENGINE_BRAVE_API_KEY", "test-key-12345")
    cfg = load_config()
    assert cfg.engines["brave"].api_key == "test-key-12345"
```
