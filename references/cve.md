# CVE Program (MITRE)

Search Common Vulnerabilities and Exposures via the CVE API. No auth required.

- **File:** `engines/cve.py`
- **Type:** API
- **Auth:** None
- **Categories:** it, security
- **Rate limit:** 5 req/s
- **Base URL:** `https://cveawg.mitre.org/api`

## Usage

```
?q=<query>&engines=cve
?q=<query>&categories=security
```

## Response

Returns CVE results with CVE ID, description, CVSS score, affected products, and references.

## Notes

- Free, public API from MITRE.
- Covers all published CVE records.
- Supports lookup by CVE ID or keyword.
