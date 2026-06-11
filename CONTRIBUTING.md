# Contributing to SlopSearX

Thanks for your interest. This project is in early development — the spec is stable but implementation is fluid.

## How to Contribute

1. **Check existing issues** — look for `good first issue` or `help wanted` labels
2. **Open an issue first** — for bugs, feature requests, or design discussions. Don't jump straight to a PR without discussion
3. **Branch from main** — `git checkout -b feat/my-thing`
4. **Keep PRs focused** — one feature or fix per PR. No mega-PRs
5. **Write tests** — new features come with tests, bug fixes come with regression tests
6. **Conventional Commits** — `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`
7. **DCO sign-off** — every commit must be signed (`git commit -s`). By signing you certify that the contribution is your own work or you have the right to submit it under the MIT license

## Development Setup

```bash
git clone git@github.com:magnus919/SlopSearX.git
cd SlopSearX
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

## Before Submitting a PR

- Run tests with coverage: `pytest --cov=slopsearx --cov=engines --cov-report=term-missing`
- Check lint: `ruff check .`
- Type check: `mypy slopsearx/ engines/`
- Verify conventional commit format on all commits

## Adding a New Engine Adapter

See `docs/ENGINE_ADAPTERS.md` for the full reference — contract rules, data types, lifecycle hooks, and the built-in adapter table. Quick checklist:

- [ ] One Python file in `engines/` with a `@register_engine` class
- [ ] `name`, `display_name`, `engine_type`, `categories` set
- [ ] `async def search()` returns `AdapterResponse` — never raises
- [ ] Import added to `engines/__init__.py`
- [ ] Tests added in `tests/test_adapters.py`
- [ ] Category tags follow SearXNG taxonomy (or use engine-specific namespace prefixes)
- [ ] **README.md Engines table updated** (name, type, auth, categories)
- [ ] Tier classification correct — see "Engine Tier Governance" below

**Removing an engine:** Remove its file from `engines/`, its import from `engines/__init__.py`, and its row from the README.md Engines table.

### Engine Tier Governance

SlopSearX uses a two-tier engine system to keep unscoped search results clean while retaining breadth:

- **Tier 1** — Broad, general-purpose engines that return relevant results on any query. These form the primary result set in unscoped searches. Tier 1 engines are curated aggressively to minimise noise.
- **Tier 2** — All specialised engines (science, packages, security, finance, media, etc.). Their results are surfaced below Tier 1 in unscoped searches, keeping top results focused without losing domain-specific coverage.

**All new engines are Tier 2 by default.** An engine may only be classified as Tier 1 with prior approval from the project maintainers. The bar for Tier 1 is high: the engine must return broadly relevant results across the long tail of everyday queries, not just within its domain.

Current Tier 1 engines are defined in `_TIER1_ENGINES` in `slopsearx/server.py`. This set changes rarely.

## Code of Conduct

Be excellent to each other. This is a small project — disagreements happen, but keep them technical and constructive.

## AI-Assisted Contributions

AI-assisted code generation and editing tools are welcome. Disclose the tool used in the PR description and take responsibility for the output — review it, test it, and own it. The contributor is the human, not the tool.
