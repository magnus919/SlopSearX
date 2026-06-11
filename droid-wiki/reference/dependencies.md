# Dependencies

## Core runtime dependencies

Declared in `pyproject.toml` under `[project.dependencies]`.

| Package | Minimum version | Purpose |
|---|---|---|
| `fastapi` | 0.115.0 | Web framework for the REST API |
| `httpx` | 0.28.0 | Async HTTP client for engine API calls |
| `lxml` | 5.3.0 | XML and HTML parsing for scrape-based engines |
| `pyyaml` | 6.0 | YAML output formatting for agent-native responses |
| `uvicorn[standard]` | 0.34.0 | ASGI server for FastAPI |
| `valkey` | 6.0.0 | Valkey client for caching and rate limiting |

## Development dependencies

Declared in `pyproject.toml` under `[project.optional-dependencies]`.

| Package | Minimum version | Purpose |
|---|---|---|
| `pytest` | 8.0 | Test framework |
| `pytest-asyncio` | 0.24.0 | Async test support for pytest |
| `ruff` | 0.9.0 | Python linter and formatter |
| `cssselect` | 1.2 | CSS selector support for lxml (used in tests) |
| `mypy` | 1.13 | Static type checker |
| `pytest-cov` | 6.0 | Code coverage plugin for pytest |
| `pip-tools` | 7.4 | Dependency pinning and lock file management |
