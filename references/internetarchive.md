# Internet Archive

Search archived web pages, books, audio, video, and software. No auth required.

- **File:** `engines/internetarchive.py`
- **Type:** API
- **Auth:** None
- **Categories:** reference, web:archive, historical
- **Rate limit:** 5 req/s (conservative)
- **Base URL:** `https://archive.org`

## Usage

```
?q=<query>&engines=internetarchive
?q=<query>&categories=historical
```

## Response

Returns archived item results with title, description, identifier, media type, and download count.

## Notes

- Public API. The IA can be slow for complex queries.
- Disabled by default — must be explicitly enabled in config.
- Covers Wayback Machine captures, books, audio recordings, videos, and software.
