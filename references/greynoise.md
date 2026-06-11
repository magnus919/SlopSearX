# GreyNoise

Search internet noise data — IPs scanning the internet. Optional API key.

- **File:** `engines/greynoise.py`
- **Type:** API
- **Auth:** `ENGINE_GREYNOISE_API_KEY` (optional)
- **Categories:** security, threat-intel
- **Rate limit:** 1 req/s (with key)
- **Base URL:** `https://api.greynoise.io/v3/community`

## Usage

```
?q=<query>&engines=greynoise
?q=<query>&categories=threat-intel
```

## Response

Returns IP classification results with noise status (benign vs. malicious), name, classification, last seen timestamp, and tags.

## Notes

- Free community tier available. API key recommended.
- Classifies IPs as "noise" (benign scanners) or potentially malicious.
- Useful for filtering out background internet noise in security investigations.
