# DuckDuckGo

General web search engine. HTML scrape-based — no API key required.

- **File:** `engines/duckduckgo.py`
- **Type:** Scrape
- **Auth:** None
- **Categories:** general, news
- **Rate limit:** Conservative (no published limit)
- **Base URL:** `https://html.duckduckgo.com/html/`

## Usage

```
?q=<query>&engines=duckduckgo
?q=<query>&categories=general
```

## Response

Returns web results with titles, URLs, and snippets. No pagination support.

## Notes

- Legal notice: DDG does not provide a public search API. This adapter scrapes the HTML results page.
- CAPTCHA walls and rate limiting may break the adapter at any time.
- Supports proxy rotation via `proxy_pool` config.
