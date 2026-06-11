# CRT.sh

Search Certificate Transparency log entries. No auth required.

- **File:** `engines/crtsh.py`
- **Type:** API
- **Auth:** None
- **Categories:** it, security
- **Rate limit:** None published
- **Base URL:** `https://crt.sh`

## Usage

```
?q=<query>&engines=crtsh
?q=<query>&categories=security
```

## Response

Returns certificate results with common name (CN), issuer, SANs, serial number, and validity dates.

## Notes

- Free, public API.
- Useful for discovering subdomains and monitoring certificate issuance for a domain.
