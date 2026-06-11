# GitHub

Search GitHub: repositories, code, issues, and pull requests. Requires a GitHub token.

- **File:** `engines/github.py`
- **Type:** API
- **Auth:** `ENGINE_GITHUB_TOKEN` (required)
- **Categories:** general, reference, github:code, github:issues, github:prs
- **Rate limit:** 30 req/min (with token), varies by endpoint
- **Base URL:** `https://api.github.com`

## Usage

```
?q=<query>&engines=github
?q=<query>&categories=github:code
?q=<query>&categories=github:issues
?q=<query>&categories=github:prs
```

## Sub-Categories

- `github:code` — Search code files (requires more specific qualifiers)
- `github:issues` — Search issues and PRs (includes state and labels)
- `github:prs` — PR-only search (alias for issues endpoint with PR filter)
- Default (no sub-category) — Search repositories

## Response

Returns different result types depending on sub-category:
- **Repositories:** name, description, language, stars, topics
- **Code:** file path, repository, snippet fragment
- **Issues/PRs:** title, state, labels, body excerpt, created date

## Notes

- Requires `ENGINE_GITHUB_TOKEN` (personal access token with `public_repo` scope for public repos).
- Code search silently returns empty for queries without repo/org qualifiers (GitHub API limitation).
