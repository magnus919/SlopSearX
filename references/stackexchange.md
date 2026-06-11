# Stack Exchange

Search Stack Exchange network (Stack Overflow, Server Fault, etc.). Optional app key for higher rate limits.

- **File:** `engines/stackexchange.py`
- **Type:** API
- **Auth:** Optional app key
- **Categories:** general, reference, science, stackexchange:code, stackexchange:serverfault
- **Rate limit:** 30 req/s (with app key), 300 req/day (without)
- **Base URL:** `https://api.stackexchange.com/2.3`

## Usage

```
?q=<query>&engines=stackexchange
?q=<query>&categories=stackexchange:code
?q=<query>&categories=stackexchange:serverfault
```

## Sub-Categories

- `stackexchange:code` — Search Stack Overflow (programming)
- `stackexchange:serverfault` — Search Server Fault (sysadmin)
- Default — Search across all Stack Exchange sites

## Response

Returns Q&A results with title, URL, tags, answer count, score, accepted answer status, and creation date.

## Notes

- Without an app key, limited to 300 requests per day.
- Set app key via `ENGINE_STACKEXCHANGE_API_KEY` env var.
