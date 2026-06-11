# Shodan

Search internet-connected device data. Requires API key.

- **File:** `engines/shodan.py`
- **Type:** API
- **Auth:** `ENGINE_SHODAN_API_KEY` (required)
- **Categories:** it, security
- **Rate limit:** 1 req/s
- **Base URL:** `https://api.shodan.io`

## Usage

```
?q=<query>&engines=shodan
?q=<query>&categories=security
```

## Response

Returns device results with IP, port, service, product, version, operating system, organization, and location data.

## Notes

- API key required (free tier available at shodan.io).
- The free tier is limited; API subscription unlocks full results.
- Covers IoT devices, servers, routers, cameras, and industrial control systems.
