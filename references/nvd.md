# NVD (National Vulnerability Database)

Search NIST CVE metadata with CVSS scores. Optional API key for higher rate limits.

- **File:** `engines/nvd.py`
- **Type:** API
- **Auth:** `ENGINE_NVD_API_KEY` (optional)
- **Categories:** it, security
- **Rate limit:** 5 req/30s (without key), 50 req/30s (with key)
- **Base URL:** `https://services.nvd.nist.gov/rest/json/cves/2.0`

## Usage

```
?q=<query>&engines=nvd
?q=<query>&categories=security
```

## Response

Returns CVE results with CVE ID, description, CVSS v2/v3/v4 scores, severity, CWE weakness IDs, and reference URLs.

## Notes

- Free tier available. API key greatly increases rate limits.
- Set `ENGINE_NVD_API_KEY` env var for higher limits.
- Supports lookup by CVE ID or keyword search.
- CVSS vector strings included for detailed scoring analysis.
