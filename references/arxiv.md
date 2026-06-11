# arXiv

Search academic papers across physics, mathematics, computer science, and more. No auth required.

- **File:** `engines/arxiv.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference
- **Rate limit:** 1 req/3s (arXiv ToS)
- **Base URL:** `http://export.arxiv.org/api/query`

## Usage

```
?q=<query>&engines=arxiv
?q=<query>&categories=science
```

## Response

Returns academic paper results with title, authors, abstract summary (~300 chars), arXiv ID, and published date.

## Notes

- Free, public API. Rate limit of 1 request per 3 seconds is enforced.
- Results are sorted by relevance.
- Content is the abstract, truncated to ~300 characters for snippet display.
