# Hacker News

Search Hacker News stories and comments via Algolia API. No auth required.

- **File:** `engines/hackernews.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, news
- **Rate limit:** 10 req/s
- **Base URL:** `https://hn.algolia.com/api/v1/search`

## Usage

```
?q=<query>&engines=hackernews
?q=<query>&categories=news
```

## Response

Returns HN story results with titles, URLs, points, author, comment count, and timestamps.

## Notes

- Free, public API with generous rate limits.
- Results include `created_at` timestamps for time-aware filtering.
