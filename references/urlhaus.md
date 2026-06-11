# URLhaus

Search known malicious URLs from the Abuse.ch URLhaus project. No auth required.

- **File:** `engines/urlhaus.py`
- **Type:** API
- **Auth:** None
- **Categories:** security, threat-intel
- **Rate limit:** None published
- **Base URL:** `https://urlhaus-api.abuse.ch/v1`

## Usage

```
?q=<query>&engines=urlhaus
?q=<query>&categories=threat-intel
```

## Response

Returns malicious URL results with URL, host, threat type, tags, payload filename, and first/last seen timestamps.

## Notes

- Free, public API from Abuse.ch.
- Covers URLs used for malware distribution.
- Supports lookup by URL, host, or hash.
