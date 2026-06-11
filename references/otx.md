# AlienVault OTX

Search Open Threat Exchange for threat intelligence indicators. Requires API key.

- **File:** `engines/otx.py`
- **Type:** API
- **Auth:** `ENGINE_OTX_API_KEY` (required)
- **Categories:** security, threat-intel
- **Rate limit:** 5 req/s
- **Base URL:** `https://otx.alienvault.com/api/v1`

## Usage

```
?q=<query>&engines=otx
?q=<query>&categories=threat-intel
```

## Response

Returns threat intelligence results with pulse name, description, indicators (IPs, domains, hashes), tags, and adversary information.

## Notes

- API key required (free at otx.alienvault.com).
- Covers community-contributed threat pulses and indicators of compromise (IOCs).
