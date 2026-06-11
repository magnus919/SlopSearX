# AbuseIPDB

Check IP address reputation and report abusive IPs. Requires API key.

- **File:** `engines/abuseipdb.py`
- **Type:** API
- **Auth:** `ENGINE_ABUSEIPDB_API_KEY` (required)
- **Categories:** security, threat-intel
- **Rate limit:** 1 req/s
- **Base URL:** `https://api.abuseipdb.com/api/v2/check`

## Usage

```
?q=<query>&engines=abuseipdb
?q=<query>&categories=security
```

## Response

Returns IP reputation results with abuse confidence score, total reports, last reported date, ISP, domain, and country.

## Notes

- API key required (free tier available at abuseipdb.com).
- Best for checking specific IP addresses vs general keyword search.
