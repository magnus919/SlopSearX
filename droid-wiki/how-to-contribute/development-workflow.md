# Development workflow

## Branching

Branch from `main`:

```bash
git checkout -b feat/my-thing
```

## Commit conventions

Use Conventional Commits format. Every commit must be signed (DCO):

```bash
git commit -s -m "feat: add MyEngine adapter"
git commit -s -m "fix: handle CAPTCHA detection in DuckDuckGo adapter"
```

Prefixes: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`

## PR process

1. Check existing issues for `good first issue` or `help wanted` labels
2. Open an issue before creating a PR for bugs, features, or design discussions
3. Keep PRs focused - one feature or fix per PR
4. Write tests: new features come with tests, bug fixes come with regression tests
5. Run the full test suite and lint check before submitting

## Pre-submit checklist

```bash
pytest                     # Run all tests
ruff check .               # Check lint
git log --oneline          # Verify conventional commit format
```

## AI-assisted contributions

AI-assisted code generation is welcome. Disclose the tool used in the PR description and take responsibility for the output (review it, test it, own it). The contributor is the human, not the tool.
