# Google

General web search engine. HTML scrape-based — no API key required.

- **File:** `engines/google.py`
- **Type:** Scrape
- **Auth:** None
- **Categories:** general, news
- **Rate limit:** Conservative
- **Base URL:** `https://www.google.com/search`

## Usage

```
?q=<query>&engines=google
?q=<query>&categories=news
```

## Response

Returns web results with titles, URLs, snippets, and optional published dates.

## Notes

- Legal notice: scraping Google Search may violate Google's ToS. Use responsibly.
- CAPTCHA walls and rate limiting may affect reliability.
- Supports proxy rotation via `proxy_pool` config.
