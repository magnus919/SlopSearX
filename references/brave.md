# Brave Search API

General web search engine with news, science, and image search capabilities. Requires an API key.

- **File:** `engines/brave.py`
- **Type:** API
- **Auth:** `ENGINE_BRAVE_API_KEY` (required)
- **Categories:** general, news, science, images
- **Rate limit:** 15 req/s (varies by plan)
- **Base URL:** `https://api.search.brave.com/res/v1/web/search`

## Usage

```
?q=<query>&engines=brave
?q=<query>&categories=news
```

## Response

Returns web results with titles, URLs, descriptions, and optional thumbnails. Also returns answer-box content (infobox, mixed sections) mapped to the SearXNG `answers` field.

## Notes

- API key is free via the Brave Search API plan (up to 2,000 queries/month).
- Set `ENGINE_BRAVE_API_KEY` env var or configure in `config.yaml`.
