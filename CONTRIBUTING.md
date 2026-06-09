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

- Run tests: `pytest`
- Check lint: `ruff check .`
- Verify conventional commit format on all commits

## Code of Conduct

Be excellent to each other. This is a small project — disagreements happen, but keep them technical and constructive.

## AI-Assisted Contributions

AI-assisted code generation and editing tools are welcome. Disclose the tool used in the PR description and take responsibility for the output — review it, test it, and own it. The contributor is the human, not the tool.
