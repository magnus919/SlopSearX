# SEC EDGAR

Search SEC filings from publicly traded companies. No auth required.

- **File:** `engines/edgar.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, finance, reference
- **Rate limit:** 10 req/s (SEC guideline)
- **Base URL:** `https://efts.sec.gov/LATEST/search-index`

## Usage

```
?q=<query>&engines=edgar
?q=<query>&categories=finance
```

## Response

Returns SEC filing results with company name, ticker, filing type (10-K, 8-K, etc.), filing date, and URL to the full filing.

## Notes

- Free, public API from the US Securities and Exchange Commission.
- Rate limit: 10 requests per second as per SEC guidelines.
- Use descriptive `User-Agent` header as required by SEC policy.
- Covers all publicly traded company filings (10-K, 10-Q, 8-K, etc.).
