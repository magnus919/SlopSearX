# Wikipedia

Search Wikipedia articles via the MediaWiki API. Two-stage pipeline: opensearch for title matching, then rich query for extracts and thumbnails.

- **File:** `engines/wikipedia.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference
- **Rate limit:** 200 req/s (generous)
- **Base URL:** `https://en.wikipedia.org/w/api.php`

## Usage

```
?q=<query>&engines=wikipedia
?q=<query>&categories=reference
```

## Response

Returns article results with titles, URLs, content extracts (first ~200 chars), thumbnails, and infoboxes with short descriptions.

## Notes

- Two-stage pipeline: opensearch for title/suggestion matching, then `prop=extracts|pageimages|pageprops` for rich content.
- Supports "Did you mean" corrections from opensearch redirect detection.
- Default 3 results per query (configurable via `max_results`).
