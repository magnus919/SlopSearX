# Censys

Search internet asset data: certificates, hosts, and services. Requires API credentials.

- **File:** `engines/censys.py`
- **Type:** API
- **Auth:** `ENGINE_CENSYS_API_KEY` + `ENGINE_CENSYS_API_SECRET` (required)
- **Categories:** it, security
- **Rate limit:** 0.5 req/s
- **Base URL:** `https://search.censys.io/api/v2`

## Usage

```
?q=<query>&engines=censys
?q=<query>&categories=security
```

## Response

Returns host/certificate results with IP, open ports, services, protocols, TLS certificate details, and location data.

## Notes

- Requires both API key and secret (free tier available at censys.io).
- Useful for attack surface discovery and certificate transparency monitoring.
