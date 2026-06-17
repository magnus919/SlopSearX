# Dependencies

## Runtime dependencies

| Package | Version | Purpose |
|---|---|---|
| **fastapi** | >=0.115.0 | HTTP framework for REST API |
| **httpx** | >=0.28.0 | Async HTTP client for engine API calls and HTML scraping |
| **lxml** | >=5.3.0 | HTML parsing for scrape-based engines (DuckDuckGo, Google, Exploit-DB) |
| **pyyaml** | >=6.0 | YAML config parsing and YAML+Markdown output formatting |
| **uvicorn[standard]** | >=0.34.0 | ASGI server |
| **valkey** | >=6.0.0 | Valkey client for caching, rate limiting, stats, audit trail |
| **structlog** | >=24.0 | Structured JSON logging |
| **sentry-sdk** | >=2.0 | Optional error tracking (activated by `SENTRY_DSN`) |

## Development dependencies

| Package | Version | Purpose |
|---|---|---|
| **pytest** | >=8.0 | Test framework |
| **pytest-asyncio** | >=0.24.0 | Async test support |
| **ruff** | >=0.9.0 | Linting and formatting |
| **cssselect** | >=1.2 | CSS selector support for lxml HTML parsing |
| **mypy** | >=1.13 | Static type checking |
| **pytest-cov** | >=6.0 | Coverage reporting |
| **pip-tools** | >=7.4 | Dependency management |
| **pre-commit** | >=4.0 | Git pre-commit hooks |
| **radon** | >=6.0 | Cyclomatic complexity analysis |
| **vulture** | >=2.11 | Dead code detection |
| **import-linter** | >=2.3 | Architecture layer enforcement |
| **deptry** | >=0.21 | Dependency analysis (unused/missing) |

## Install commands

```bash
# Runtime only
pip install -e .

# With dev dependencies
pip install -e ".[dev]"
```

## Notes

- **No prometheus-client dependency** — metrics use stdlib-only implementation in `slopsearx/metrics.py`
- **No browser automation** — scrape engines use HTTP + HTML parsing, not Playwright/Selenium
- **No database** — Valkey is the only shared state
- **No local volumes** — all persistent state lives in Valkey with TTL-based expiry
