# Getting started with SlopSearX

## Prerequisites

- Python 3.12+
- Valkey 7+ (or Redis 7+) — for caching, rate limiting, stats, and audit trail
- Brave Search API key (optional but recommended, set `ENGINE_BRAVE_API_KEY`)

## Local development setup

```bash
git clone git@github.com:magnus919/SlopSearX.git
cd SlopSearX
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

## Running tests

```bash
# Run all tests
pytest

# Run with verbose output
pytest -v

# Run specific test file
pytest tests/test_merger.py

# Run tests matching a pattern
pytest -k "cache"
```

## Linting

```bash
ruff check .
ruff format .
```

## Running the server

Start Valkey (if not already running):

```bash
docker run -d --name valkey -p 6379:6379 valkey/valkey:8-alpine
```

Start the SlopSearX server:

```bash
uvicorn slopsearx.server:app --host 0.0.0.0 --port 8080
```

The server starts on `http://localhost:8080`. It loads all registered engines but only the ones with API keys configured will return results.

## Using the SSX CLI

The `ssx` CLI provides an agent-friendly wrapper around the API:

```bash
# Search across all engines (YAML+Markdown output by default)
python ssx search "quantum computing breakthroughs"

# Search with category filter
python ssx search "transformers" --categories science

# Search specific engines
python ssx search "python web scraping" --engines brave,wikipedia,stackexchange

# List all engines with status
python ssx engines

# Health check
python ssx health

# Show engine-to-categories mapping
python ssx config

# JSON output for programmatic use
python ssx search "hello" --json
```

The CLI reads the server URL from `SSX_URL` (default: `http://localhost:8080`).

## Making direct API requests

```bash
# SearXNG-compatible JSON
curl 'http://localhost:8080/search?q=python+web+scraping&format=json'

# Agent-native YAML+Markdown
curl 'http://localhost:8080/search?q=python+web+scraping&format=yaml'

# Filter by category
curl 'http://localhost:8080/search?q=transformers&categories=science'

# Select specific engines
curl 'http://localhost:8080/search?q=hello&engines=brave,wikipedia'

# Health check
curl 'http://localhost:8080/health'

# OpenMetrics
curl 'http://localhost:8080/metrics'

# Engine config
curl 'http://localhost:8080/config'
```

## Docker

```bash
# Build and run with docker-compose
docker compose up -d

# Build standalone image
docker build -t slopsearx:0.1.0 .
docker run -p 8080:8080 -e VALKEY_URL=redis://host.docker.internal:6379/0 slopsearx:0.1.0
```

## Kubernetes

Apply the Kustomize manifests to deploy with Valkey:

```bash
kubectl apply -k k8s/
```

This creates a deployment with 3 replicas, a ClusterIP service on port 8080, and an HPA that scales from 3 to 100 replicas at 70% CPU utilization.

## Configuration

SlopSearX uses a three-layer config system:

1. **Built-in defaults** — hardcoded engine URLs, timeouts, and cache TTLs
2. **Config file** — optional YAML file at `/etc/slopsearx/config.yaml`
3. **Environment variables** — `ENGINE_*` and `SEARCH_*` variables override everything

Key environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `VALKEY_URL` | (empty) | Valkey connection string |
| `MAX_CONCURRENT_ENGINES` | 10 | Max simultaneous outbound HTTP connections per search |
| `PER_CLIENT_REQUESTS` | 30 | Allowed search requests per client IP per window |
| `PER_CLIENT_WINDOW_SECONDS` | 60 | Sliding window duration for per-client rate limiting |
| `FAIL_CLOSED` | false | When Valkey is unreachable, deny rate-limit checks |
| `FAIL_CLOSED_GRACE_SECONDS` | 30 | Seconds before falling back to in-process rate limiter |
| `SEARCH_CACHE_TTL_SECONDS` | 3600 | Cache TTL for non-news queries |
| `SEARCH_CACHE_NEGATIVE_TTL_SECONDS` | 60 | Cache TTL for negative (error) entries |

See [Configuration](../systems/configuration.md) for details.

## CI pipeline

The project uses GitHub Actions with four workflows:

1. **ci.yml** — Lint (`ruff check .`) and test (`pytest -v` on Python 3.12 and 3.13) on every push/PR
2. **docker.yml** — Build and push Docker image to ghcr.io on main/tag pushes
3. **droid.yml** — Factory Droid tag tracking on main pushes
4. **droid-review.yml** — Factory Droid auto-review on PR open/sync

See [Tooling](../how-to-contribute/tooling.md) for details.
