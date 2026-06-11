# FRED (Federal Reserve Economic Data)

Search economic data series from the St. Louis Fed. Requires API key.

- **File:** `engines/fred.py`
- **Type:** API
- **Auth:** `ENGINE_FRED_API_KEY` (required)
- **Categories:** general, finance, reference, economics
- **Rate limit:** None published
- **Base URL:** `https://api.stlouisfed.org/fred/series/search`

## Usage

```
?q=<query>&engines=fred
?q=<query>&categories=finance
```

## Response

Returns economic data series results with series ID, title, frequency, units, seasonal adjustment, observation range, and popularity score.

## Notes

- API key required (free at fred.stlouisfed.org).
- Covers 800K+ US and international economic time series.
- Results sorted by popularity by default.
