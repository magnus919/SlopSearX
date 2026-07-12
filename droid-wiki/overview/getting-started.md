# Getting started

## Prerequisites

- Python 3.12+
- Valkey 8 (or Redis-compatible server) for caching and rate limiting
- (Optional) Brave Search API key for primary search

## Quick start with Docker

Pre-built images from GitHub Container Registry:

```bash
# Pull and run with Valkey
docker run -d --name valkey valkey/valkey:8-alpine
docker run -d --name slopsearx -p 8080:8080 \
  -e VALKEY_URL=redis://valkey:6379/0 \
  --link valkey \
  ghcr.io/magnus919/slopsearx:latest

# Try it
curl 'http://localhost:8080/search?q=hello+world&format=json'
```

## Docker Compose

```bash
docker compose up -d
```

This starts SlopSearX + Valkey on the GroktoCrawl network. The service is internal-only by default.

## Development setup

```bash
git clone git@github.com:magnus919/SlopSearX.git
cd SlopSearX
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

## Run the server

```bash
# Without Valkey (caching and rate limiting gracefully degrade)
uvicorn slopsearx.server:app --reload

# With Valkey (full functionality)
VALKEY_URL=redis://localhost:6379/0 uvicorn slopsearx.server:app --reload
```

## Set up pre-commit hooks

```bash
pre-commit install
```

Pre-commit hooks run `ruff` formatting and linting on every commit.

## Run tests

```bash
# Full test suite with coverage
pytest --cov=slopsearx --cov=engines --cov-report=term-missing

# Run specific test files
pytest tests/test_adapters.py -v
pytest tests/test_server.py -v
```

## Before submitting a PR

```bash
# Lint
ruff check .

# Type check
mypy slopsearx/ engines/

# Run tests
pytest -v

# Code quality analysis
radon cc slopsearx/           # cyclomatic complexity
vulture slopsearx/ engines/   # dead code detection
deptry .                      # dependency analysis
import-linter                 # architecture layer checks
```

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `VALKEY_URL` | (none) | Valkey/Redis connection string |
| `ENGINE_BRAVE_API_KEY` | (none) | Brave Search API key |
| `SENTRY_DSN` | (none) | Sentry DSN for error tracking |
| `MAX_CONCURRENT_ENGINES` | 10 | Max simultaneous outbound HTTP connections per search |
| `PER_CLIENT_REQUESTS` | 30 | Allowed requests per client IP per window |
| `PER_CLIENT_WINDOW_SECONDS` | 60 | Sliding window for per-client rate limiting |
| `FAIL_CLOSED` | false | Deny requests when Valkey is unreachable |
| `SEARCH_CACHE_TTL_SECONDS` | 3600 | Cache TTL in seconds |
| `FEATURE_<NAME>` | false | Feature flag overrides |
