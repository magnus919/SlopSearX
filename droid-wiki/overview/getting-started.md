# Getting started with SlopSearX

## Prerequisites

- Python 3.12+
- Valkey 7+ (or Redis 7+) — for caching and rate limiting
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
```

## Linting

```bash
ruff check .
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

The server starts on `http://localhost:8080`. It loads 12 engines by default but only the ones with API keys configured will return results.

## Making requests

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

See [Configuration](../systems/configuration.md) for details.

## CI pipeline

The project uses GitHub Actions with two jobs per push/PR:

1. `ruff check .` — linting
2. `pytest -v` — tests on Python 3.12 and 3.13

See `.github/workflows/ci.yml` for the full workflow.
