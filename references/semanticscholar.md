# Semantic Scholar

Search academic papers with AI-powered relevance. Optional API key for higher rate limits.

- **File:** `engines/semanticscholar.py`
- **Type:** API
- **Auth:** Optional API key
- **Categories:** general, science, reference
- **Rate limit:** 1 req/s (without key), 10 req/s (with key)
- **Base URL:** `https://api.semanticscholar.org/graph/v1/paper/search`

## Usage

```
?q=<query>&engines=semanticscholar
?q=<query>&categories=science
```

## Response

Returns paper results with title, authors, abstract, year, citation count, and external links (PDF, arXiv, DOI).

## Notes

- Free tier available. API key increases rate limit from 1 to 10 req/s.
- Set `ENGINE_SEMANTICSCHOLAR_API_KEY` env var for higher limits.
- Covers over 200M papers across all scientific disciplines.
