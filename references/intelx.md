# IntelX

Search intelligence data from the deep and dark web. Requires API key.

- **File:** `engines/intelx.py`
- **Type:** API
- **Auth:** `ENGINE_INTELX_API_KEY` (required)
- **Categories:** security, threat-intel
- **Rate limit:** 1 req/s
- **Base URL:** `https://2.intelx.io`

## Usage

```
?q=<query>&engines=intelx
?q=<query>&categories=threat-intel
```

## Response

Returns intelligence results with selector, type (email, domain, IP, phone, etc.), bucket, and timestamps.

## Notes

- API key required (free tier available at intelx.io).
- Covers leaked databases, dark web content, and public intelligence sources.
- Longer timeout (15s default) due to potentially slower deep-web sources.
