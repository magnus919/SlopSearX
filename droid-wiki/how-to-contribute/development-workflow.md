# Development workflow

Active contributors: Magnus Hedemark

## Setup

```bash
git clone git@github.com:magnus919/SlopSearX.git
cd SlopSearX
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
pre-commit install
```

## Contribution workflow

1. **Find or create an issue** — bugs, features, or design discussions
2. **Branch from `main`** — `git checkout -b feat/my-thing`
3. **Implement** — keep PRs focused: one feature or fix per PR
4. **Write tests** — new features need tests, bug fixes need regression tests
5. **Conventional Commits** — `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`
6. **DCO sign-off** — every commit: `git commit -s`
7. **Open PR** — fill the PR template, link the issue

## Pre-commit checklist

Before pushing:

```bash
ruff check .          # Lint
ruff format .         # Format
mypy slopsearx/ engines/  # Type check
pytest -v             # Run tests
```

## Commit conventions

```
feat: add NVD engine adapter
fix: handle DuckDuckGo CAPTCHA detection
docs: update engine table in README
chore: bump httpx to 0.28
ci: add Python 3.13 to test matrix
refactor: extract URL normalization to merger
```

## PR template

Each PR should describe:
- What changed and why
- Link to related issues
- Testing performed
- Any operational considerations (new env vars, config changes)

## Engine contribution

Adding a new engine:

1. Create `engines/myengine.py` with `@register_engine` class
2. Implement `async def search(query, params)` — never raise, classify errors
3. Set `name`, `display_name`, `engine_type`, `categories`
4. Add `from engines import myengine` to `engines/__init__.py`
5. Add row to README.md Engines table
6. Add tests in `tests/test_adapters.py`
7. All new engines default to Tier 2 — Tier 1 requires maintainer approval

Removing an engine: remove the file, its import, its README row.

## AI-assisted contributions

AI-assisted code generation and editing tools are welcome. Disclose the tool used in the PR description and take responsibility for the output — review it, test it, and own it.

## Key source files

| File | Description |
|---|---|
| `CONTRIBUTING.md` | Full contribution guide |
| `.github/PULL_REQUEST_TEMPLATE.md` | PR template |
| `.pre-commit-config.yaml` | Pre-commit hook configuration |
| `pyproject.toml` | ruff, mypy, pytest, coverage settings |
