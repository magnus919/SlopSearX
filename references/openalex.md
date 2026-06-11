# OpenAlex

Search scholarly works, authors, and institutions via the OpenAlex open catalog. No auth required.

- **File:** `engines/openalex.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference
- **Rate limit:** 10 req/s (100K/day polite usage)
- **Base URL:** `https://api.openalex.org`

## Usage

```
?q=<query>&engines=openalex
?q=<query>&categories=science
```

## Response

Returns academic paper results with title, authors, publication year, journal, cited by count, and DOI/URL.

## Notes

- Free, public API with generous polite pool (100K requests/day).
- Use `mailto` in `User-Agent` header for better rate limits.
- Covers over 250M scholarly works from journals, repositories, and conferences.
