# Tooling

Active contributors: Magnus Hedemark

## Purpose

Build system, linters, CI workflows, code quality tools, and the SSX CLI that support development and deployment of SlopSearX.

## SSX CLI

Agent-friendly CLI wrapper at the repository root:

```bash
python ssx search "quantum computing" --categories science
python ssx engines
python ssx health
python ssx config
```

All commands support `--json` for programmatic output. Default output is YAML+Markdown. Server URL from `SSX_URL` env var (default: `http://localhost:8080`).

## Linting and formatting

**ruff** handles both linting and formatting (no separate Black/isort/flake8 config):

```bash
ruff check .        # Lint
ruff format .       # Format
```

Configuration in `pyproject.toml` under `[tool.ruff]`:
- Target: Python 3.12
- Line length: 120
- Rules: E (pycodestyle), F (pyflakes), I (isort), N (pep8-naming), W (pycodestyle warnings)
- Quote style: double

## Type checking

**mypy** with strict mode:

```bash
mypy slopsearx/ engines/
```

Configuration in `pyproject.toml` under `[tool.mypy]`:
- `strict = true`
- Python 3.12 target
- `valkey`, `yaml`, `lxml` ignored (no type stubs)

## Code quality tools

| Tool | Purpose | Command |
|---|---|---|
| **deptry** | Dependency analysis (unused/missing deps) | `deptry .` |
| **radon** | Cyclomatic complexity | `radon cc slopsearx/` |
| **vulture** | Dead code detection | `vulture slopsearx/ engines/` |
| **import-linter** | Architecture layer enforcement | `import-linter` |
| **jscpd** | Copy-paste detection | `jscpd` (config in `.jscpd.json`) |

### deptry config

In `pyproject.toml`:
```toml
[tool.deptry]
extend_exclude = ["tests", "engines"]
```

### vulture config

In `pyproject.toml`:
```toml
[tool.vulture]
paths = ["slopsearx", "engines"]
min_confidence = 65
ignore_decorators = ["@register_engine"]
ignore_names = ["engine_type", "categories", "fallback"]
```

### import-linter config

In `pyproject.toml`:
```toml
[tool.importlinter]
root_package = "slopsearx"

[[tool.importlinter.contracts]]
id = "architecture-layers"
type = "layers"
layers = ["slopsearx.server", "slopsearx.router", "slopsearx.merger",
          "slopsearx.formatter", "slopsearx.suggest", "slopsearx.stats",
          "slopsearx.audit", "slopsearx.cache", "slopsearx.config",
          "slopsearx.ratelimit", "slopsearx.proxypool", "slopsearx.adapter"]
```

## Pre-commit hooks

```bash
pre-commit install
```

Runs `ruff` formatting and linting on every commit. Configuration in `.pre-commit-config.yaml`.

## CI/CD

### Workflows in `.github/workflows/`

| Workflow | Trigger | Jobs |
|---|---|---|
| **ci.yml** | Push / PR | Lint (ruff check), Test (pytest on Python 3.12 + 3.13) |
| **docker.yml** | Push to main / tag | Build Docker image, push to `ghcr.io/magnus919/slopsearx` |
| **droid.yml** | Push to main | Factory Droid tagging for session tracking |
| **droid-review.yml** | PR open / sync | Factory Droid automated code review |
| **droid-wiki-refresh.yml** | Push to main | Wiki refresh via Factory Droid |
| **release-please.yml** | Push to main | Automated release PR management |

### Release Please

Uses Google's `release-please-action` to automate releases:
- Detects conventional commits since last release
- Creates/updates a release PR with changelog
- On merge: creates GitHub release + version tag

## Build and packaging

`pyproject.toml` with setuptools:

- **Runtime deps:** fastapi, httpx, lxml, pyyaml, uvicorn, valkey, structlog, sentry-sdk
- **Dev deps:** pytest, pytest-asyncio, ruff, cssselect, mypy, pytest-cov, pip-tools, pre-commit, radon, vulture, import-linter, deptry
- Package includes: `slopsearx*` and `engines*`

## Docker

- **Dockerfile** — Multi-stage build, Python 3.12-slim, ~200MB image, cold start <2s
- **docker-compose.yml** — SlopSearX + Valkey on GroktoCrawl network
- **HEALTHCHECK** — 30s interval, 5s timeout via `docker/healthcheck.py`
- **`.dockerignore`** — excludes venv, git, pycache, test artifacts

## Kubernetes

Kustomize manifests in `k8s/`:
- Deployment: 3 replicas, resource limits, Valkey env vars
- ClusterIP service: port 8080
- HPA: 3-100 replicas at 70% CPU utilization

Apply: `kubectl apply -k k8s/`

## Key source files

| File | Description |
|---|---|
| `ssx` | Agent-friendly CLI |
| `pyproject.toml` | Build config, dependencies, tool settings |
| `.pre-commit-config.yaml` | Pre-commit hooks |
| `.github/workflows/ci.yml` | Main CI: lint + test |
| `.github/workflows/docker.yml` | Docker build + push |
| `.github/workflows/release-please.yml` | Automated releases |
| `.github/dependabot.yml` | Dependency update automation |
| `Dockerfile` | Production build |
| `docker-compose.yml` | Dev orchestration |
| `k8s/` | Kubernetes manifests |
| `.jscpd.json` | Copy-paste detection config |
