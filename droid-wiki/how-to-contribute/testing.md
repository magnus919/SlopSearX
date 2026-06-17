# Testing

Active contributors: Magnus Hedemark

## Test structure

```
tests/
├── test_adapters.py       # Engine adapter tests (~3,000 lines)
├── test_server.py         # Server and API tests
├── test_cache.py          # Cache integration tests
├── test_ratelimit.py      # Rate limiting tests (including fail-closed)
├── test_concurrency.py    # Concurrency and semaphore tests (686 lines)
├── test_categories.py     # Category routing tests
├── test_feature_flags.py  # Feature flag tests
├── test_integration.py    # End-to-end integration tests
└── ...
```

~6,100 lines across 37 test files.

## Running tests

```bash
# Full suite with coverage
pytest --cov=slopsearx --cov=engines --cov-report=term-missing

# Specific test file
pytest tests/test_adapters.py -v

# Specific test
pytest tests/test_server.py::test_search_json -v

# With coverage threshold
pytest  # --cov-fail-under=70 is set in pyproject.toml
```

## Coverage targets

- Minimum coverage: 70% (enforced by pytest-cov)
- Source paths: `slopsearx/` and `engines/`
- Ignored: `tests/` directory

## Test configuration

In `pyproject.toml`:

```toml
[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
addopts = "--cov=slopsearx --cov=engines --cov-report=term-missing --cov-fail-under=70"
```

## Writing tests

- **Adapter tests:** Verify `search()` returns `AdapterResponse`, never raises. Test error classification (status codes, timeouts, malformed responses)
- **Server tests:** Test the full request lifecycle. Use FastAPI `TestClient` with async support via `pytest-asyncio`
- **Cache tests:** Test cache hit/miss, negative caching, graceful degradation
- **Rate limit tests:** Test local token bucket, Valkey sliding window, fail-closed fallback behavior
- **Concurrency tests:** Test semaphore-bounded dispatch, timeout handling
- **Integration tests:** End-to-end with actual engine dispatch (may skip when Valkey unavailable)

## Key dependencies

| Tool | Purpose |
|---|---|
| pytest | Test framework |
| pytest-asyncio | Async test support (auto mode) |
| pytest-cov | Coverage reporting |
