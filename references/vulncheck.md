# VulnCheck

Search vulnerability intelligence with exploitation data. Requires API key.

- **File:** `engines/vulncheck.py`
- **Type:** API
- **Auth:** `ENGINE_VULNCHECK_API_KEY` (required)
- **Categories:** security, threat-intel
- **Rate limit:** 5 req/s
- **Base URL:** `https://api.vulncheck.com/v3`

## Usage

```
?q=<query>&engines=vulncheck
?q=<query>&categories=threat-intel
```

## Response

Returns vulnerability results with CVE ID, CVSS score, exploitation status, affected products, and proof-of-concept references.

## Notes

- API key required (free tier available at vulncheck.com).
- Focuses on vulnerabilities with confirmed exploitation activity.
- Includes vendor advisory cross-references and known exploit timelines.
